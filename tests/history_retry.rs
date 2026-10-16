use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use twstock_gpui::market::*;

fn symbol(code: &str) -> Symbol {
    Symbol::new(Market::Listed, code).unwrap()
}

#[test]
fn persistent_failure_requires_enqueue_after_cooldown_and_backoff_is_capped() {
    let a = symbol("AAAA");
    let b = symbol("BBBB");
    let mut queue = HistoryQueue::default();
    let mut at = Instant::now();
    queue.enqueue_at(&[a.clone(), b.clone()], Some(&a), at);
    for seconds in [60, 120, 240, 480, 600, 600] {
        queue.fail(&a, at);
        if queue.next().is_some() {
            assert_eq!(
                queue.next(),
                Some(b.clone()),
                "a failed job must not block others"
            );
            queue.finish(&b);
        }
        assert_eq!(queue.next(), None, "no automatic retry");
        queue.enqueue_at(
            std::slice::from_ref(&a),
            Some(&a),
            at + Duration::from_secs(seconds - 1),
        );
        assert_eq!(
            queue.next(),
            None,
            "enqueue must not treat selected as an explicit retry"
        );
        at += Duration::from_secs(seconds);
        assert_eq!(queue.next(), None, "expiry alone must not schedule work");
        queue.enqueue_at(std::slice::from_ref(&a), None, at);
        assert_eq!(queue.next(), Some(a.clone()));
    }
    queue.finish(&a);
    queue.select(&a);
    queue.enqueue_at(
        std::slice::from_ref(&a),
        Some(&a),
        at + Duration::from_secs(3600),
    );
    assert_eq!(queue.next(), None, "successful history must not reload");
}

#[test]
fn explicit_selection_bypasses_cooldown_and_deduplicates_retries() {
    let a = symbol("AAAA");
    let b = symbol("BBBB");
    let at = Instant::now();
    let mut queue = HistoryQueue::default();
    queue.enqueue_at(&[a.clone(), b.clone()], Some(&a), at);
    queue.fail(&a, at);
    assert_eq!(queue.next(), Some(b.clone()));
    queue.select(&a);
    queue.select(&a);
    assert_eq!(queue.next(), Some(a.clone()));
    queue.fail(&a, at);
    assert_eq!(queue.next(), Some(b.clone()));
    queue.finish(&b);
    queue.enqueue_at(
        std::slice::from_ref(&a),
        None,
        at + Duration::from_secs(119),
    );
    assert_eq!(
        queue.next(),
        None,
        "explicit failed retries still grow backoff"
    );
    queue.enqueue_at(
        std::slice::from_ref(&a),
        None,
        at + Duration::from_secs(120),
    );
    assert_eq!(queue.next(), Some(a));
}

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
struct Fake {
    clock: Clock,
    requests: Arc<Mutex<Vec<Duration>>>,
    responses: VecDeque<Result<String, String>>,
}
impl HttpTransport for Fake {
    fn get(&mut self, _: &str) -> Result<String, String> {
        self.requests.lock().unwrap().push(self.clock.elapsed());
        self.responses.pop_front().unwrap()
    }
}
#[test]
fn transient_failure_then_retry_preserves_reason_and_watchlist_until_success() {
    for explicit_selection in [false, true] {
        let cache = tempfile::tempdir().unwrap();
        let mut clock = Clock::default();
        let requests = Arc::new(Mutex::new(Vec::new()));
        let fake = Fake {
            clock: clock.clone(),
            requests: requests.clone(),
            responses: VecDeque::from([
                Err("temporary HTTP 503".into()),
                Ok(include_str!("fixtures/history.json").into()),
            ]),
        };
        let mut provider =
            HistoryProvider::with_clock(fake, cache.path().to_owned(), clock.clone());
        let a = symbol("AAAA");
        let mut store = MarketStore::new(Capabilities::mis());
        store.add_instrument(Instrument::new(a.clone()));
        let mut queue = HistoryQueue::default();
        let at = Instant::now();
        queue.enqueue_at(std::slice::from_ref(&a), Some(&a), at);
        let result = provider.recent(&a, 1, 1_768_457_700);
        assert!(result.is_err());
        queue.fail(&a, at);
        store.set_history(a.clone(), result);
        assert_eq!(store.order, vec![a.clone()]);
        assert_eq!(store.history_errors[&a], "temporary HTTP 503");
        assert!(!store.histories.contains_key(&a));
        assert_eq!(queue.next(), None);
        if explicit_selection {
            queue.select(&a);
        } else {
            clock.sleep(Duration::from_secs(60));
            queue.enqueue_at(std::slice::from_ref(&a), Some(&a), at + clock.elapsed());
        }
        assert_eq!(queue.next(), Some(a.clone()));
        assert_eq!(
            store.history_errors[&a], "temporary HTTP 503",
            "retry must keep failure UI until data arrives"
        );
        let result = provider.recent(&a, 1, 1_768_457_700);
        assert!(result.is_ok());
        queue.finish(&a);
        store.set_history(a.clone(), result);
        assert_eq!(store.order, vec![a.clone()]);
        assert!(!store.history_errors.contains_key(&a));
        assert_eq!(store.histories[&a].len(), 1);
        let requests = requests.lock().unwrap();
        assert_eq!(requests.len(), 2);
        assert!(
            requests[1] - requests[0] >= Duration::from_secs(2),
            "explicit selection must retain the request throttle"
        );
        assert_eq!(queue.next(), None);
    }
}
