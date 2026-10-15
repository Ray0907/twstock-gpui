use super::*;
use chrono::{Datelike, FixedOffset, NaiveDate, NaiveTime, TimeZone, Timelike};
use futures::{Stream, StreamExt};
use serde_json::Value;
use std::{
    process::Command,
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};

const INDEX: &str = "https://mis.twse.com.tw/stock/index.jsp";
pub trait HttpTransport: Send + 'static {
    fn get(&mut self, url: &str) -> Result<String, String>;
}
pub struct CurlSession {
    cookie: tempfile::NamedTempFile,
    referer: String,
}
impl CurlSession {
    pub fn new(referer: &str) -> Result<Self, String> {
        Ok(Self {
            cookie: tempfile::NamedTempFile::new().map_err(|e| e.to_string())?,
            referer: referer.into(),
        })
    }
}
impl HttpTransport for CurlSession {
    fn get(&mut self, url: &str) -> Result<String, String> {
        let output = Command::new("curl")
            .args([
                "--silent",
                "--show-error",
                "--fail",
                "--compressed",
                "--connect-timeout",
                "10",
                "--max-time",
                "25",
                "-A",
                "Mozilla/5.0",
                "-e",
                &self.referer,
                "-b",
            ])
            .arg(self.cookie.path())
            .arg("-c")
            .arg(self.cookie.path())
            .arg(url)
            .output()
            .map_err(|e| format!("HTTP transport unavailable: {e}"))?;
        if !output.status.success() {
            return Err(String::from_utf8_lossy(&output.stderr).trim().into());
        }
        String::from_utf8(output.stdout).map_err(|_| "response is not UTF-8".into())
    }
}
fn optional_text<'a>(row: &'a Value, key: &str) -> Result<Option<&'a str>, String> {
    match row.get(key) {
        None => Ok(None),
        Some(Value::String(s)) if s == "-" => Ok(None),
        Some(Value::String(s)) if !s.is_empty() => Ok(Some(s)),
        _ => Err(format!("invalid MIS string field {key}")),
    }
}
fn price(row: &Value, key: &str) -> Result<Option<Price>, String> {
    match row.get(key) {
        None => Ok(None),
        Some(Value::String(s)) if s == "-" => Ok(None),
        Some(Value::String(s)) => {
            let p = s.parse::<Price>()?;
            if p.0 < 0 {
                return Err("negative asset price".into());
            }
            Ok(Some(p))
        }
        _ => Err(format!("invalid price field {key}")),
    }
}
fn number(input: &str) -> Result<u64, String> {
    input
        .replace(',', "")
        .parse()
        .map_err(|_| "invalid quantity".into())
}
fn depth(
    row: &Value,
    prices: &str,
    quantities: &str,
    as_of: i64,
) -> Result<Option<Vec<Depth>>, String> {
    let p = optional_text(row, prices)?;
    let q = optional_text(row, quantities)?;
    let Some(p) = p else {
        return Ok(None);
    };
    let Some(q) = q else { return Ok(None) };
    let p = p.split('_').filter(|s| !s.is_empty()).collect::<Vec<_>>();
    let q = q.split('_').filter(|s| !s.is_empty()).collect::<Vec<_>>();
    if p.len() != q.len() {
        return Err("depth price/quantity length mismatch".into());
    }
    let mut book = Vec::new();
    for (p, q) in p.into_iter().zip(q).take(5) {
        let price = if p == "-" {
            None
        } else {
            let price = p.parse::<Price>()?;
            if price.0 < 0 {
                return Err("negative depth price".into());
            }
            Some(price)
        };
        let quantity = if q == "-" {
            None
        } else {
            Some(number(q)?.checked_mul(1000).ok_or("depth overflow")?)
        };
        if let (Some(price), Some(quantity)) = (price, quantity)
            && price.0 > 0
        {
            book.push(Depth {
                price,
                quantity,
                as_of,
            });
        }
    }
    Ok(Some(book))
}
pub fn local_time(timestamp: i64) -> chrono::DateTime<FixedOffset> {
    FixedOffset::east_opt(8 * 3600)
        .unwrap()
        .timestamp_opt(timestamp, 0)
        .single()
        .unwrap()
}
fn session(date: NaiveDate, now: i64) -> SessionState {
    let current = local_time(now);
    if current.weekday().number_from_monday() > 5 || date != current.date_naive() {
        return SessionState::Closed;
    }
    let minutes = current.hour() * 60 + current.minute();
    if minutes < 9 * 60 {
        SessionState::PreOpen
    } else if minutes < 13 * 60 + 30 {
        SessionState::Open
    } else {
        SessionState::PostClose
    }
}
pub fn parse_mis(body: &str, now: i64) -> Result<Vec<QuoteUpdate>, String> {
    let value: Value = serde_json::from_str(body).map_err(|_| "invalid MIS JSON")?;
    if value["rtcode"].as_str() != Some("0000") {
        return Err(format!(
            "MIS rtcode {}",
            value["rtcode"].as_str().unwrap_or("missing")
        ));
    }
    let rows = value["msgArray"].as_array().ok_or("missing MIS rows")?;
    if rows.is_empty() {
        return Err("MIS returned no instruments".into());
    }
    rows.iter()
        .map(|row| {
            let market = match row["ex"].as_str() {
                Some("tse") => Market::Listed,
                Some("otc") => Market::Otc,
                _ => return Err("unsupported exchange".into()),
            };
            let symbol = Symbol::new(market, row["c"].as_str().ok_or("missing code")?)?;
            let date =
                NaiveDate::parse_from_str(row["d"].as_str().ok_or("missing date")?, "%Y%m%d")
                    .map_err(|_| "invalid MIS date")?;
            let time = NaiveTime::parse_from_str(
                row["t"].as_str().ok_or("missing per-symbol time")?,
                "%H:%M:%S%.f",
            )
            .map_err(|_| "invalid per-symbol MIS time")?;
            let as_of = FixedOffset::east_opt(8 * 3600)
                .unwrap()
                .from_local_datetime(&date.and_time(time))
                .single()
                .ok_or("invalid MIS timestamp")?
                .timestamp();
            let mut instrument = Instrument::new(symbol.clone());
            instrument.name = optional_text(row, "n")?.unwrap_or(&symbol.code).into();
            let volume = match optional_text(row, "v")? {
                None => None,
                Some(v) => Some(
                    number(v)?
                        .checked_mul(instrument.lot_size)
                        .ok_or("volume overflow")?,
                ),
            };
            Ok(QuoteUpdate {
                symbol,
                instrument: Some(Box::new(instrument)),
                last: price(row, "z")?,
                volume,
                open: price(row, "o")?,
                high: price(row, "h")?,
                low: price(row, "l")?,
                prev_close: price(row, "y")?,
                limit_up: price(row, "u")?,
                limit_down: price(row, "w")?,
                bids: depth(row, "b", "g", as_of)?,
                asks: depth(row, "a", "f", as_of)?,
                as_of: Some(as_of),
                source: Some("MIS".into()),
                session: Some(session(date, now)),
            })
        })
        .collect()
}
pub struct MisProvider<T: HttpTransport = CurlSession> {
    transport: Arc<Mutex<T>>,
    ready: bool,
    retry: u64,
    pub bootstrap_error: Option<String>,
    subscriptions: Arc<Mutex<Vec<Symbol>>>,
}
impl MisProvider<CurlSession> {
    pub fn new() -> Result<Self, String> {
        Ok(Self::with_transport(CurlSession::new(INDEX)?))
    }
}
impl<T: HttpTransport> MisProvider<T> {
    pub fn with_transport(transport: T) -> Self {
        Self {
            transport: Arc::new(Mutex::new(transport)),
            subscriptions: Arc::new(Mutex::new(Vec::new())),
            ready: false,
            retry: 5,
            bootstrap_error: None,
        }
    }
    pub fn resubscribe(&self, symbols: Vec<Symbol>) {
        *self.subscriptions.lock().unwrap() = symbols;
    }
    pub fn capabilities(&self) -> Capabilities {
        Capabilities::mis()
    }
    pub fn poll_once(&mut self, symbols: &[Symbol], now: i64) -> Vec<MarketEvent> {
        let started = Instant::now();
        let mut events = Vec::new();
        if !self.ready {
            events.push(MarketEvent::Link(LinkState::Connecting));
        }
        let response = (|| {
            let mut http = self
                .transport
                .lock()
                .map_err(|_| "HTTP session lock failed")?;
            if !self.ready {
                self.bootstrap_error = http.get(INDEX).err();
                if let Some(error) = &self.bootstrap_error {
                    eprintln!("MIS bootstrap unavailable ({error}); requesting quotes directly");
                }
                self.ready = true;
            }
            let channels = symbols
                .iter()
                .take(self.capabilities().max_subscriptions)
                .map(|s| format!("{}_{}.tw", s.market.exchange(), s.code))
                .collect::<Vec<_>>()
                .join("%7C");
            if channels.is_empty() {
                return Err("empty subscription".into());
            }
            let body=http.get(&format!("https://mis.twse.com.tw/stock/api/getStockInfo.jsp?ex_ch={channels}&json=1&delay=0&_={}",now*1000))?;
            let mut rows = parse_mis(&body, now)?;
            rows.retain(|row| {
                symbols
                    .iter()
                    .take(self.capabilities().max_subscriptions)
                    .any(|s| *s == row.symbol)
            });
            if symbols
                .iter()
                .take(self.capabilities().max_subscriptions)
                .any(|s| !rows.iter().any(|r| r.symbol == *s))
            {
                return Err("MIS omitted requested instrument".into());
            }
            Ok::<_, String>(rows)
        })();
        match response {
            Ok(rows) => {
                events.extend(rows.into_iter().map(MarketEvent::Quote));
                events.push(MarketEvent::Polled {
                    at: now.saturating_add(started.elapsed().as_secs() as i64),
                });
                events.push(MarketEvent::Link(LinkState::Live));
                self.retry = 5;
            }
            Err(reason) => {
                self.ready = false;
                events.push(MarketEvent::Link(LinkState::Failed {
                    reason,
                    retry_in: self.retry,
                }));
                self.retry = (self.retry * 2).min(300);
            }
        }
        events
    }
}
impl<T: HttpTransport> QuoteFeed for MisProvider<T> {
    fn capabilities(&self) -> Capabilities {
        self.capabilities()
    }
    fn subscribe(&mut self, symbols: Vec<Symbol>) -> EventStream {
        self.resubscribe(symbols);
        let mut worker = Self {
            transport: self.transport.clone(),
            subscriptions: self.subscriptions.clone(),
            ready: false,
            retry: 5,
            bootstrap_error: None,
        };
        let (mut sender, stream) = latest_events(Capabilities::mis().max_subscriptions);
        thread::spawn(move || {
            while !sender.is_closed() {
                let symbols = worker.subscriptions.lock().unwrap().clone();
                let events = if symbols.is_empty() {
                    vec![MarketEvent::Link(LinkState::Live)]
                } else {
                    worker.poll_once(&symbols, super::now())
                };
                let delay = events
                    .iter()
                    .find_map(|e| {
                        if let MarketEvent::Link(LinkState::Failed { retry_in, .. }) = e {
                            Some(*retry_in)
                        } else {
                            None
                        }
                    })
                    .unwrap_or(5)
                    .max(5);
                sender.publish(events);
                let waiting = Instant::now();
                let until = waiting + Duration::from_secs(delay);
                while Instant::now() < until {
                    if sender.is_closed() {
                        return;
                    }
                    if waiting.elapsed() >= Duration::from_secs(5)
                        && *worker.subscriptions.lock().unwrap() != symbols
                    {
                        worker.retry = 5;
                        break;
                    }
                    thread::sleep(Duration::from_millis(100));
                }
            }
        });
        stream
    }
}

// The queue holds at most one merged update per subscribed symbol, plus poll/link state.
// A bounded wake channel only signals work; the snapshots never accumulate in it.
struct LatestSender {
    pending: Arc<Mutex<std::collections::VecDeque<MarketEvent>>>,
    wake: futures::channel::mpsc::Sender<()>,
    capacity: usize,
}
impl LatestSender {
    fn is_closed(&self) -> bool {
        self.wake.is_closed()
    }
    fn publish(&mut self, events: Vec<MarketEvent>) {
        let mut pending = self.pending.lock().unwrap();
        for mut event in events {
            let same = |old: &MarketEvent| match (&event, old) {
                (MarketEvent::Quote(a), MarketEvent::Quote(b)) => a.symbol == b.symbol,
                (MarketEvent::Polled { .. }, MarketEvent::Polled { .. })
                | (MarketEvent::Link(_), MarketEvent::Link(_)) => true,
                _ => false,
            };
            if let Some(index) = pending.iter().position(same) {
                let old = pending.remove(index).unwrap();
                if let (MarketEvent::Quote(old), MarketEvent::Quote(next)) = (old, &mut event) {
                    if old.as_of > next.as_of {
                        pending.push_back(MarketEvent::Quote(old));
                        continue;
                    }
                    if old.as_of.zip(next.as_of).is_some_and(|(a, b)| {
                        local_time(a).date_naive() == local_time(b).date_naive()
                    }) {
                        macro_rules! preserve {($($field:ident),*)=>{$(if next.$field.is_none(){next.$field=old.$field;})*};}
                        preserve!(
                            instrument, last, volume, open, high, low, prev_close, limit_up,
                            limit_down, bids, asks, source, session
                        );
                    }
                }
            } else if matches!(event, MarketEvent::Quote(_))
                && pending
                    .iter()
                    .filter(|e| matches!(e, MarketEvent::Quote(_)))
                    .count()
                    >= self.capacity
                && let Some(index) = pending
                    .iter()
                    .position(|e| matches!(e, MarketEvent::Quote(_)))
            {
                pending.remove(index);
            }
            pending.push_back(event);
        }
        drop(pending);
        let _ = self.wake.try_send(());
    }
}
fn latest_events(capacity: usize) -> (LatestSender, EventStream) {
    let (wake, mut receiver) = futures::channel::mpsc::channel(0);
    let pending = Arc::new(Mutex::new(std::collections::VecDeque::new()));
    let state = pending.clone();
    let stream = futures::stream::poll_fn(move |cx| {
        loop {
            if let Some(event) = state.lock().unwrap().pop_front() {
                return std::task::Poll::Ready(Some(event));
            }
            match std::pin::Pin::new(&mut receiver).poll_next(cx) {
                std::task::Poll::Ready(Some(())) => continue,
                std::task::Poll::Ready(None) => return std::task::Poll::Ready(None),
                std::task::Poll::Pending => return std::task::Poll::Pending,
            }
        }
    })
    .boxed();
    (
        LatestSender {
            pending,
            wake,
            capacity: capacity.max(1),
        },
        stream,
    )
}
#[cfg(test)]
mod review_tests {
    use super::*;
    use futures::StreamExt;
    #[test]
    fn r5_many_symbols_and_midnight_stay_bounded_and_daily() {
        let (mut sender, stream) = latest_events(100);
        for time in 1..=200 {
            let mut events = Vec::new();
            for n in 0..100 {
                events.push(MarketEvent::Quote(QuoteUpdate {
                    symbol: Symbol::new(Market::Listed, &format!("{}", 887000 + n)).unwrap(),
                    as_of: Some(time),
                    last: (time == 1).then_some(Price(123000)),
                    volume: Some(time as u64),
                    ..Default::default()
                }));
            }
            events.push(MarketEvent::Polled { at: time });
            events.push(MarketEvent::Link(LinkState::Live));
            sender.publish(events);
            assert!(sender.pending.lock().unwrap().len() <= 102);
        }
        drop(sender);
        let events = futures::executor::block_on(stream.collect::<Vec<_>>());
        assert_eq!(events.len(), 102);
        for event in events {
            if let MarketEvent::Quote(q) = event {
                assert_eq!(q.volume, Some(200));
                assert_eq!(q.last, Some(Price(123000)));
            }
        }
        let (mut sender, stream) = latest_events(1);
        let symbol = Symbol::new(Market::Listed, "889901").unwrap();
        sender.publish(vec![MarketEvent::Quote(QuoteUpdate {
            symbol: symbol.clone(),
            as_of: Some(100),
            last: Some(Price(123000)),
            ..Default::default()
        })]);
        sender.publish(vec![MarketEvent::Quote(QuoteUpdate {
            symbol,
            as_of: Some(86500),
            ..Default::default()
        })]);
        drop(sender);
        let events = futures::executor::block_on(stream.collect::<Vec<_>>());
        assert_eq!(events.len(), 1);
        assert!(matches!(&events[0],MarketEvent::Quote(q) if q.last.is_none()));
    }
    #[test]
    fn r5_slow_consumer_keeps_bounded_latest_snapshots() {
        let (mut sender, mut stream) = latest_events(2);
        let symbol = Symbol::new(Market::Listed, "889901").unwrap();
        for time in 1..=1000 {
            sender.publish(vec![
                MarketEvent::Quote(QuoteUpdate {
                    symbol: symbol.clone(),
                    as_of: Some(time),
                    volume: Some(time as u64),
                    last: if time == 1 { Some(Price(123000)) } else { None },
                    ..Default::default()
                }),
                MarketEvent::Link(LinkState::Live),
            ]);
            assert!(sender.pending.lock().unwrap().len() <= 3);
        }
        drop(sender);
        let events =
            futures::executor::block_on(async move { stream.by_ref().collect::<Vec<_>>().await });
        assert!(events.len() <= 3);
        let q = events
            .iter()
            .find_map(|e| {
                if let MarketEvent::Quote(q) = e {
                    Some(q)
                } else {
                    None
                }
            })
            .unwrap();
        assert_eq!(q.volume, Some(1000));
        assert_eq!(q.last, Some(Price(123000)));
    }
}
