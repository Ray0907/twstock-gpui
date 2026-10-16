use super::{SharedHistoryWork, run_history_worker};
use crate::market::*;
use futures::StreamExt;
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

#[derive(Clone, Default)]
struct Clock(Arc<Mutex<Duration>>);
impl HistoryClock for Clock {
    fn elapsed(&self) -> Duration {
        *self.0.lock().unwrap()
    }
    fn sleep(&mut self, duration: Duration) {
        *self.0.lock().unwrap() += duration;
    }
}
struct Http {
    fail_first: bool,
    clock: Clock,
    requests: Arc<Mutex<Vec<Duration>>>,
}
impl HttpTransport for Http {
    fn get(&mut self, url: &str) -> Result<String, String> {
        self.requests.lock().unwrap().push(self.clock.elapsed());
        // Simulated network time also makes worker readiness advance without real sleeps.
        self.clock.sleep(Duration::from_secs(2));
        if std::mem::take(&mut self.fail_first) {
            return Err("temporary HTTP 503".into());
        }
        let date = url
            .split("date=")
            .nth(1)
            .unwrap()
            .split('&')
            .next()
            .unwrap();
        let year = date[..4].parse::<i32>().unwrap() - 1911;
        let month = &date[4..6];
        let rows = (1..=28)
            .map(|day| {
                serde_json::json!([
                    format!("{year}/{month}/{day:02}"),
                    "1000",
                    "0",
                    "10",
                    "12",
                    "9",
                    "11"
                ])
            })
            .collect::<Vec<_>>();
        Ok(serde_json::json!({"stat":"OK", "data": rows}).to_string())
    }
}

#[test]
fn real_worker_retries_transport_and_provider_initialization_failures() {
    for initialization_failure in [false, true] {
        let cache = tempfile::tempdir().unwrap();
        let directory = cache.path().to_owned();
        let symbol = Symbol::new(Market::Listed, "AAAA").unwrap();
        let work = SharedHistoryWork::default();
        work.0
            .lock()
            .unwrap()
            .queue
            .enqueue(std::slice::from_ref(&symbol), Some(&symbol));
        let requests = Arc::new(Mutex::new(Vec::new()));
        let request_log = requests.clone();
        let attempts = Arc::new(AtomicUsize::new(0));
        let initialization_attempts = attempts.clone();
        let worker_state = work.clone();
        let (tx, mut rx) = futures::channel::mpsc::unbounded();
        let (received, replies) = std::sync::mpsc::channel();
        let forwarder = std::thread::spawn(move || {
            futures::executor::block_on(async move {
                while let Some(result) = rx.next().await {
                    if received.send(result).is_err() {
                        break;
                    }
                }
            })
        });
        let worker = std::thread::spawn(move || {
            run_history_worker(worker_state, tx, || {
                let attempt = initialization_attempts.fetch_add(1, Ordering::SeqCst);
                if initialization_failure && attempt == 0 {
                    return Err("provider initialization failed".into());
                }
                let clock = Clock::default();
                Ok(HistoryProvider::with_clock(
                    Http {
                        fail_first: !initialization_failure,
                        clock: clock.clone(),
                        requests: request_log.clone(),
                    },
                    directory.clone(),
                    clock,
                ))
            })
        });
        let mut store = MarketStore::new(Capabilities::mis());
        store.add_instrument(Instrument::new(symbol.clone()));
        let (failed_symbol, result) = replies.recv_timeout(Duration::from_secs(5)).unwrap();
        let reason = result.as_ref().unwrap_err().clone();
        store.set_history(failed_symbol, result);
        {
            let mut state = work.0.lock().unwrap();
            state
                .queue
                .enqueue(std::slice::from_ref(&symbol), Some(&symbol));
            assert_eq!(
                state.queue.next(),
                None,
                "no automatic/enqueue retry while cooling down"
            );
            state.queue.select(&symbol);
            assert_eq!(
                state.queue.next(),
                Some(symbol.clone()),
                "failure must not be finished"
            );
        }
        assert_eq!(store.history_errors[&symbol], reason);
        assert_eq!(store.order, vec![symbol.clone()]);
        work.1.notify_one();
        let retried = replies.recv_timeout(Duration::from_secs(5));
        {
            let mut state = work.0.lock().unwrap();
            state.stopped = true;
            work.1.notify_one();
        }
        worker.join().unwrap();
        forwarder.join().unwrap();
        let (retried_symbol, rows) =
            retried.expect("selected failure must be retried by the actual worker");
        assert_eq!(rows.as_ref().unwrap().len(), 100);
        store.set_history(retried_symbol, rows);
        assert!(!store.history_errors.contains_key(&symbol));
        assert_eq!(store.order, vec![symbol.clone()]);
        let mut state = work.0.lock().unwrap();
        state.queue.select(&symbol);
        assert_eq!(
            state.queue.next(),
            None,
            "only successful loads are finished"
        );
        assert_eq!(
            attempts.load(Ordering::SeqCst),
            if initialization_failure { 2 } else { 1 }
        );
        let log = requests.lock().unwrap();
        assert!(
            log.windows(2)
                .all(|p| p[1] - p[0] >= Duration::from_secs(2))
        );
    }
}
