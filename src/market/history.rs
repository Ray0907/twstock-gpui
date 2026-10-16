use super::*;
use chrono::{Datelike, NaiveDate, NaiveDateTime, Timelike};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::{BTreeMap, HashSet},
    path::{Path, PathBuf},
    sync::{Arc, Mutex, OnceLock},
    thread,
    time::{Duration, Instant},
};
pub fn parse_history(body: &str) -> Result<Vec<Candle>, String> {
    let value: Value = serde_json::from_str(body).map_err(|_| "invalid history JSON")?;
    if value["stat"].as_str() != Some("OK") {
        return Err(value["stat"]
            .as_str()
            .unwrap_or("history request failed")
            .into());
    }
    value["data"]
        .as_array()
        .ok_or("missing history data")?
        .iter()
        .map(|row| {
            let fields = row.as_array().ok_or("invalid history row")?;
            let text = |index: usize| {
                fields
                    .get(index)
                    .and_then(Value::as_str)
                    .ok_or("missing history field")
            };
            let date = text(0)?
                .split('/')
                .map(str::parse::<i32>)
                .collect::<Result<Vec<_>, _>>()
                .map_err(|_| "invalid ROC date")?;
            if date.len() != 3 {
                return Err("invalid ROC date".into());
            }
            let year = date[0].checked_add(1911).ok_or("ROC year overflow")?;
            let time = NaiveDate::from_ymd_opt(year, date[1] as u32, date[2] as u32)
                .ok_or("invalid history date")?;
            let volume = text(1)?
                .replace(',', "")
                .parse::<u64>()
                .map_err(|_| "invalid history volume")?;
            let price = |i| -> Result<Price, String> {
                let p = text(i)?.parse::<Price>()?;
                if p.0 < 0 {
                    return Err("negative history price".into());
                }
                Ok(p)
            };
            let candle = Candle {
                time,
                open: price(3)?,
                high: price(4)?,
                low: price(5)?,
                close: price(6)?,
                volume,
            };
            if candle.low > candle.high
                || candle.open < candle.low
                || candle.close < candle.low
                || candle.open > candle.high
                || candle.close > candle.high
            {
                return Err("inconsistent history OHLC".into());
            }
            Ok(candle)
        })
        .collect()
}
fn monthly_rows(body: &str) -> Result<Vec<Candle>, String> {
    let value: Value = serde_json::from_str(body).map_err(|_| "invalid history JSON")?;
    if value["stat"]
        .as_str()
        .is_some_and(|s| s.contains("沒有符合條件"))
    {
        return Ok(vec![]);
    }
    parse_history(body)
}

const REQUEST_SPACING: Duration = Duration::from_secs(2);

pub trait HistoryClock: Send {
    fn elapsed(&self) -> Duration;
    fn sleep(&mut self, duration: Duration);
}
pub struct RealHistoryClock(Instant);
impl HistoryClock for RealHistoryClock {
    fn elapsed(&self) -> Duration {
        self.0.elapsed()
    }
    fn sleep(&mut self, duration: Duration) {
        thread::sleep(duration);
    }
}
struct Throttle<C> {
    clock: C,
    last: Option<Duration>,
}
impl<C: HistoryClock> Throttle<C> {
    fn remaining(&self) -> Duration {
        self.last.map_or(Duration::ZERO, |last| {
            REQUEST_SPACING.saturating_sub(self.clock.elapsed().saturating_sub(last))
        })
    }
    fn wait(&mut self) {
        let delay = self.remaining();
        if !delay.is_zero() {
            self.clock.sleep(delay);
        }
        self.last = Some(self.clock.elapsed());
    }
}
type SharedThrottle<C> = Arc<Mutex<Throttle<C>>>;

#[derive(Serialize, Deserialize)]
struct CachedMonth {
    fetched_on: NaiveDate,
    #[serde(default)]
    complete_through: Option<NaiveDate>,
    rows: Vec<Candle>,
}
#[derive(Serialize, Deserialize)]
struct Cache {
    version: u8,
    months: BTreeMap<String, CachedMonth>,
}
impl Cache {
    fn final_rows(&self, month: NaiveDate, now: NaiveDateTime) -> Option<&[Candle]> {
        let today = now.date();
        self.months
            .get(&month.format("%Y-%m").to_string())
            .filter(|cached| {
                if month < today.with_day(1).unwrap() {
                    cached.fetched_on.with_day(1).unwrap() > month
                } else if month == today.with_day(1).unwrap() {
                    cached.complete_through.is_some_and(|complete_through| {
                        weekday_or_before(last_closed(now)) <= weekday_or_before(complete_through)
                    })
                } else {
                    false
                }
            })
            .map(|cached| cached.rows.as_slice())
    }
}
fn last_closed(now: NaiveDateTime) -> NaiveDate {
    let date = if now.hour() >= 14 {
        now.date()
    } else {
        now.date().pred_opt().unwrap_or(now.date())
    };
    weekday_or_before(date)
}
fn weekday_or_before(mut date: NaiveDate) -> NaiveDate {
    while date.weekday().number_from_monday() > 5 {
        date = date.pred_opt().unwrap_or(date);
    }
    date
}
impl Default for Cache {
    fn default() -> Self {
        Self {
            version: 1,
            months: BTreeMap::new(),
        }
    }
}
fn valid_rows(rows: &[Candle], month: NaiveDate) -> bool {
    rows.windows(2).all(|p| p[0].time < p[1].time)
        && rows.iter().all(|c| {
            c.time.with_day(1) == Some(month)
                && c.low.0 >= 0
                && c.low <= c.high
                && (c.low..=c.high).contains(&c.open)
                && (c.low..=c.high).contains(&c.close)
        })
}
fn previous_month(month: NaiveDate) -> Result<NaiveDate, String> {
    month
        .pred_opt()
        .and_then(|d| d.with_day(1))
        .ok_or("history date underflow".into())
}
pub fn history_cache_dir() -> Result<PathBuf, String> {
    let root = match std::env::var_os("TWSTOCK_GPUI_CACHE_DIR") {
        Some(path) => PathBuf::from(path),
        None => PathBuf::from(std::env::var_os("HOME").ok_or("HOME is unavailable")?)
            .join(".cache/twstock-gpui"),
    };
    Ok(root.join("history"))
}
pub struct HistoryProvider<T: HttpTransport = CurlSession, C: HistoryClock = RealHistoryClock> {
    transport: T,
    cache: PathBuf,
    throttle: SharedThrottle<C>,
    warned: HashSet<PathBuf>,
    requests: usize,
}
impl HistoryProvider<CurlSession> {
    pub fn new() -> Result<Self, String> {
        Self::new_with_cache(history_cache_dir()?)
    }
    pub fn new_with_cache(cache: PathBuf) -> Result<Self, String> {
        Ok(Self::with_cache(
            CurlSession::new("https://www.twse.com.tw/")?,
            cache,
        ))
    }
}
impl<T: HttpTransport> HistoryProvider<T> {
    pub fn with_cache(transport: T, cache: PathBuf) -> Self {
        // All live providers share request spacing, including across instruments/windows.
        static THROTTLE: OnceLock<SharedThrottle<RealHistoryClock>> = OnceLock::new();
        Self {
            transport,
            cache,
            throttle: THROTTLE
                .get_or_init(|| {
                    Arc::new(Mutex::new(Throttle {
                        clock: RealHistoryClock(Instant::now()),
                        last: None,
                    }))
                })
                .clone(),
            warned: HashSet::new(),
            requests: 0,
        }
    }
}
impl<T: HttpTransport, C: HistoryClock> HistoryProvider<T, C> {
    pub fn with_clock(transport: T, cache: PathBuf, clock: C) -> Self {
        Self {
            transport,
            cache,
            throttle: Arc::new(Mutex::new(Throttle { clock, last: None })),
            warned: HashSet::new(),
            requests: 0,
        }
    }
    pub fn request_count(&self) -> usize {
        self.requests
    }
    fn warn(&mut self, path: &Path, error: impl std::fmt::Display) {
        if self.warned.insert(path.to_owned()) {
            eprintln!("history cache {}: {error}; using network", path.display());
        }
    }
    fn read_cache(&mut self, path: &Path, today: NaiveDate) -> Cache {
        let result = (|| -> Result<Cache, String> {
            let bytes = match std::fs::read(path) {
                Ok(bytes) => bytes,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Cache::default()),
                Err(e) => return Err(e.to_string()),
            };
            let cache: Cache = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
            if cache.version != 1
                || !cache.months.iter().all(|(key, value)| {
                    NaiveDate::parse_from_str(&format!("{key}-01"), "%Y-%m-%d").is_ok_and(|month| {
                        month.format("%Y-%m").to_string() == *key
                            && value.fetched_on <= today
                            && value.complete_through.is_none_or(|d| d <= value.fetched_on)
                            && valid_rows(&value.rows, month)
                    })
                })
            {
                return Err("invalid version or month data".into());
            }
            Ok(cache)
        })();
        match result {
            Ok(cache) => cache,
            Err(e) => {
                self.warn(path, e);
                Cache::default()
            }
        }
    }
    fn month(
        &mut self,
        symbol: &Symbol,
        month: NaiveDate,
        now: NaiveDateTime,
    ) -> Result<Vec<Candle>, String> {
        let today = now.date();
        Symbol::new(symbol.market, &symbol.code)?;
        let path = self
            .cache
            .join(format!("{}-{}.json", symbol.market.exchange(), symbol.code));
        let mut cache = self.read_cache(&path, today);
        let key = month.format("%Y-%m").to_string();
        if let Some(rows) = cache.final_rows(month, now) {
            return Ok(rows.to_vec());
        }
        let url = format!(
            "https://www.twse.com.tw/rwd/zh/afterTrading/STOCK_DAY?date={}&stockNo={}&response=json",
            month.format("%Y%m%d"),
            symbol.code
        );
        let body = {
            let mut throttle = self.throttle.lock().unwrap();
            throttle.wait();
            self.requests += 1;
            // Keep the guard through the request so providers cannot overtake one another.
            self.transport.get(&url)?
        };
        let mut rows = monthly_rows(&body)?;
        rows.sort_by_key(|c| c.time);
        rows.dedup_by_key(|c| c.time);
        if !valid_rows(&rows, month) {
            return Err("invalid history month rows".into());
        }
        cache.months.insert(
            key,
            CachedMonth {
                fetched_on: today,
                complete_through: Some(if now.hour() >= 14 {
                    today
                } else {
                    today.pred_opt().unwrap_or(today)
                }),
                rows: rows.clone(),
            },
        );
        let written = serde_json::to_vec(&cache)
            .map_err(|e| e.to_string())
            .and_then(|bytes| atomic_write(&path, &bytes));
        if let Err(e) = written {
            self.warn(&path, e);
        }
        Ok(rows)
    }
    pub fn recent(
        &mut self,
        symbol: &Symbol,
        count: usize,
        now: i64,
    ) -> Result<Vec<Candle>, String> {
        let mut load = HistoryLoad::new(symbol.clone(), count, now)?;
        loop {
            if let Some(rows) = load.advance(self)? {
                return Ok(rows);
            }
        }
    }
}
/// One month per step lets the workspace reconsider selection after every request.
pub struct HistoryLoad {
    symbol: Symbol,
    count: usize,
    end: NaiveDate,
    now: NaiveDateTime,
    month: NaiveDate,
    months: usize,
    rows: Vec<Candle>,
}
impl HistoryLoad {
    pub fn new(symbol: Symbol, count: usize, now: i64) -> Result<Self, String> {
        Symbol::new(symbol.market, &symbol.code)?;
        if symbol.market != Market::Listed {
            return Err("上櫃歷史尚未接入".into());
        }
        let now = local_time(now).naive_local();
        let end = now.date();
        Ok(Self {
            symbol,
            count,
            end,
            now,
            month: end.with_day(1).unwrap(),
            months: 0,
            rows: Vec::new(),
        })
    }
    /// Allows an interruptible workspace wait before choosing the next network request.
    pub fn ready_in<T: HttpTransport, C: HistoryClock>(
        &self,
        provider: &mut HistoryProvider<T, C>,
    ) -> Duration {
        if self.count == 0 {
            return Duration::ZERO;
        }
        let path = provider.cache.join(format!(
            "{}-{}.json",
            self.symbol.market.exchange(),
            self.symbol.code
        ));
        if provider
            .read_cache(&path, self.end)
            .final_rows(self.month, self.now)
            .is_some()
        {
            Duration::ZERO
        } else {
            provider.throttle.lock().unwrap().remaining()
        }
    }
    pub fn advance<T: HttpTransport, C: HistoryClock>(
        &mut self,
        provider: &mut HistoryProvider<T, C>,
    ) -> Result<Option<Vec<Candle>>, String> {
        if self.count == 0 {
            return Ok(Some(Vec::new()));
        }
        self.rows.extend(
            provider
                .month(&self.symbol, self.month, self.now)?
                .into_iter()
                .filter(|c| c.time <= self.end),
        );
        self.rows.sort_by_key(|c| c.time);
        self.rows.dedup_by_key(|c| c.time);
        if self.rows.len() >= self.count {
            return Ok(Some(self.rows.split_off(self.rows.len() - self.count)));
        }
        self.months += 1;
        if self.months >= 12 {
            return Err(format!(
                "只有 {} 根歷史，未滿 {}",
                self.rows.len(),
                self.count
            ));
        }
        self.month = previous_month(self.month)?;
        Ok(None)
    }
}
impl<T: HttpTransport, C: HistoryClock> History for HistoryProvider<T, C> {
    fn candles(
        &mut self,
        symbol: &Symbol,
        timeframe: Timeframe,
        range: DateRange,
    ) -> Result<Vec<Candle>, String> {
        if timeframe != Timeframe::Day || symbol.market != Market::Listed {
            return Err("unsupported history".into());
        }
        if range.start > range.end {
            return Err("invalid range".into());
        }
        let now = local_time(now()).naive_local();
        let mut month = range.end.with_day(1).ok_or("invalid range")?;
        let start = range.start.with_day(1).ok_or("invalid range")?;
        let mut rows = Vec::new();
        loop {
            rows.extend(
                self.month(symbol, month, now)?
                    .into_iter()
                    .filter(|c| c.time >= range.start && c.time <= range.end),
            );
            if month == start {
                break;
            }
            month = previous_month(month)?;
        }
        rows.sort_by_key(|c| c.time);
        rows.dedup_by_key(|c| c.time);
        Ok(rows)
    }
}
