use super::*;
use std::collections::HashMap;
#[derive(Clone, Debug, Default)]
pub struct Snapshot {
    pub last: Option<Price>,
    pub volume: Option<u64>,
    pub open: Option<Price>,
    pub high: Option<Price>,
    pub low: Option<Price>,
    pub prev_close: Option<Price>,
    pub limit_up: Option<Price>,
    pub limit_down: Option<Price>,
    pub bids: Vec<Depth>,
    pub asks: Vec<Depth>,
    pub as_of: Option<i64>,
    pub source: Option<String>,
    pub session: Option<SessionState>,
    pub received_at: Option<i64>,
    pub trades: Vec<Trade>,
    last_update: Option<QuoteUpdate>,
}
impl Snapshot {
    pub fn reference_price(&self) -> Option<Price> {
        if self.last.is_some() {
            return None;
        }
        let bid = self
            .bids
            .iter()
            .filter(|d| d.quantity > 0 && d.price.0 > 0)
            .map(|d| d.price)
            .max()?;
        let ask = self
            .asks
            .iter()
            .filter(|d| d.quantity > 0 && d.price.0 > 0)
            .map(|d| d.price)
            .min()?;
        (bid <= ask).then(|| Price(((i128::from(bid.0) + i128::from(ask.0)) / 2) as i64))
    }
    pub fn display_price(&self) -> Option<Price> {
        self.last.or_else(|| self.reference_price())
    }
    pub fn display_change(&self) -> Option<Price> {
        Some(Price(self.display_price()?.0 - self.prev_close?.0))
    }
    pub fn display_change_percent_units(&self) -> Option<i64> {
        let previous = self.prev_close?.0;
        if previous == 0 {
            return None;
        }
        i64::try_from(i128::from(self.display_change()?.0) * 1_000_000 / i128::from(previous)).ok()
    }
    fn candle(&self) -> Option<Candle> {
        Some(Candle {
            time: local_time(self.as_of?).date_naive(),
            open: self.open?,
            high: self.high?,
            low: self.low?,
            close: self.last?,
            volume: self.volume?,
        })
    }
    pub fn near_limits(&self, instrument: &Instrument) -> bool {
        let Some(last) = self.last else {
            return false;
        };
        self.limit_up.is_some_and(|up| {
            last >= instrument
                .tick_rule
                .previous_price(instrument.tick_rule.previous_price(up))
        }) || self.limit_down.is_some_and(|down| {
            last <= instrument
                .tick_rule
                .next_price(instrument.tick_rule.next_price(down))
        })
    }
    pub fn change(&self) -> Option<Price> {
        Some(Price(self.last?.0 - self.prev_close?.0))
    }
    pub fn change_percent_units(&self) -> Option<i64> {
        let previous = self.prev_close?.0;
        if previous == 0 {
            return None;
        }
        i64::try_from(i128::from(self.change()?.0) * 1_000_000 / i128::from(previous)).ok()
    }
}
#[derive(Clone)]
pub struct MarketStore {
    pub capabilities: Capabilities,
    pub link: LinkState,
    pub last_successful_poll: Option<i64>,
    pub order: Vec<Symbol>,
    pub indices: Vec<IndexSnapshot>,
    pub instruments: HashMap<Symbol, Instrument>,
    pub histories: HashMap<Symbol, Vec<Candle>>,
    pub history_errors: HashMap<Symbol, String>,
    snapshots: HashMap<Symbol, Snapshot>,
}
impl MarketStore {
    pub fn new(capabilities: Capabilities) -> Self {
        Self {
            capabilities,
            link: LinkState::Connecting,
            last_successful_poll: None,
            order: vec![],
            indices: vec![],
            instruments: HashMap::new(),
            histories: HashMap::new(),
            history_errors: HashMap::new(),
            snapshots: HashMap::new(),
        }
    }
    pub fn add_instrument(&mut self, instrument: Instrument) {
        let symbol = instrument.symbol.clone();
        if !self.instruments.contains_key(&symbol) {
            self.order.push(symbol.clone());
        }
        self.snapshots.entry(symbol.clone()).or_default();
        self.instruments.insert(symbol, instrument);
    }
    pub fn chart_series(&self, symbol: &Symbol) -> Option<ChartSeries> {
        let instrument = self.instruments.get(symbol)?;
        let snapshot = self.snapshot(symbol)?;
        let mut candles = self.histories.get(symbol)?.clone();
        if let Some(candle) = snapshot.candle()
            && candles.last().is_none_or(|c| c.time <= candle.time)
        {
            if candles.last().is_some_and(|c| c.time == candle.time) {
                candles.pop();
            }
            candles.push(candle);
        }
        let averages = [5, 10, 20].map(|p| moving_average(&candles, p));
        Some(ChartSeries {
            candles,
            averages,
            lot_size: instrument.lot_size,
        })
    }
    pub fn snapshot(&self, symbol: &Symbol) -> Option<&Snapshot> {
        self.snapshots.get(symbol)
    }
    pub fn snapshots(&self) -> impl Iterator<Item = &Snapshot> {
        self.snapshots.values()
    }
    pub fn subscriptions(&self, visible: &[Symbol]) -> Vec<Symbol> {
        let mut result = Vec::new();
        if self.capabilities.max_subscriptions == 0 {
            return result;
        }
        for s in visible.iter().chain(self.order.iter()) {
            if self.instruments.contains_key(s) && !result.contains(s) {
                result.push(s.clone());
            }
            if result.len() == self.capabilities.max_subscriptions {
                break;
            }
        }
        result
    }
    pub fn apply(&mut self, event: MarketEvent, received_at: i64) -> bool {
        match event {
            MarketEvent::Polled { at } => {
                self.last_successful_poll =
                    Some(self.last_successful_poll.map_or(at, |old| old.max(at)));
                true
            }
            MarketEvent::Link(link) => {
                let changed = self.link != link;
                self.link = link;
                changed
            }
            MarketEvent::Trade(trade) => {
                let Some(snapshot) = self.snapshots.get_mut(&trade.symbol) else {
                    return false;
                };
                if snapshot.trades.iter().any(|t| t.time >= trade.time) {
                    return false;
                }
                snapshot.trades.insert(0, trade);
                snapshot.trades.truncate(100);
                true
            }
            MarketEvent::Quote(update) => {
                let Some(as_of) = update.as_of else {
                    return false;
                };
                if let Some(old) = self.snapshots.get_mut(&update.symbol) {
                    if old.as_of.is_some_and(|t| as_of < t) {
                        return false;
                    }
                    if old.last_update.as_ref() == Some(&update) {
                        old.received_at = Some(received_at);
                        return false;
                    }
                }
                if !self.instruments.contains_key(&update.symbol) {
                    self.add_instrument(Instrument::new(update.symbol.clone()));
                }
                if let Some(instrument) = update.instrument.clone() {
                    self.add_instrument(*instrument);
                }
                let old = self.snapshots.get(&update.symbol).unwrap();
                if old
                    .as_of
                    .is_some_and(|at| local_time(at).date_naive() != local_time(as_of).date_naive())
                {
                    if let Some(candle) = old.candle() {
                        let rows = self.histories.entry(update.symbol.clone()).or_default();
                        match rows.binary_search_by_key(&candle.time, |c| c.time) {
                            Ok(i) => rows[i] = candle,
                            Err(i) => rows.insert(i, candle),
                        }
                        if rows.len() > 100 {
                            rows.drain(..rows.len() - 100);
                        }
                    }
                    self.snapshots
                        .insert(update.symbol.clone(), Snapshot::default());
                }
                let snapshot = self.snapshots.get_mut(&update.symbol).unwrap();
                macro_rules! merge {($($field:ident),*)=>{$(if update.$field.is_some(){snapshot.$field=update.$field.clone();})*};}
                merge!(
                    last, volume, open, high, low, prev_close, limit_up, limit_down, source,
                    session
                );
                if let Some(book) = update.bids.as_ref() {
                    snapshot.bids = book
                        .iter()
                        .take(self.capabilities.depth_levels)
                        .cloned()
                        .collect();
                }
                if let Some(book) = update.asks.as_ref() {
                    snapshot.asks = book
                        .iter()
                        .take(self.capabilities.depth_levels)
                        .cloned()
                        .collect();
                }
                snapshot.as_of = Some(as_of);
                snapshot.received_at = Some(received_at);
                snapshot.last_update = Some(update);
                if matches!(self.link, LinkState::Stale { .. }) {
                    self.link = LinkState::Live;
                }
                true
            }
        }
    }
    pub fn tick(&mut self, now: i64) -> bool {
        if !matches!(self.link, LinkState::Live | LinkState::Stale { .. }) {
            return false;
        }
        let last = self
            .snapshots
            .values()
            .filter(|q| q.session == Some(SessionState::Open))
            .filter_map(|q| q.received_at)
            .max();
        let last = self.last_successful_poll.or(last);
        // Leave room for an in-flight request instead of expiring exactly at the poll interval.
        let next = match last {
            Some(since)
                if self
                    .snapshots
                    .values()
                    .any(|q| q.session == Some(SessionState::Open))
                    && now.saturating_sub(since) > self.capabilities.min_interval as i64 * 3 =>
            {
                LinkState::Stale { since }
            }
            _ => LinkState::Live,
        };
        let changed = next != self.link;
        self.link = next;
        changed || self.last_successful_poll.is_some()
    }
    pub fn status_text(&self, now: i64) -> String {
        match &self.link {
            LinkState::Connecting => "連線中".into(),
            LinkState::Failed { reason, retry_in } => {
                if *retry_in == 0 {
                    format!("失敗：{reason}")
                } else {
                    format!("失敗：{reason} · {retry_in} 秒後重試")
                }
            }
            LinkState::Stale { since } => {
                format!("過期 · 最後更新 {} 秒前", now.saturating_sub(*since))
            }
            LinkState::Live => {
                let session = if self.snapshots.values().all(|q| {
                    matches!(
                        q.session,
                        Some(SessionState::Closed | SessionState::PostClose)
                    )
                }) {
                    "休市"
                } else if self
                    .snapshots
                    .values()
                    .any(|q| q.session == Some(SessionState::PreOpen))
                {
                    "盤前"
                } else {
                    "正常"
                };
                self.last_successful_poll.map_or_else(
                    || session.into(),
                    |at| {
                        format!(
                            "{session} · 最後更新 {} 秒前",
                            now.saturating_sub(at).max(0)
                        )
                    },
                )
            }
        }
    }
    pub fn data_time_range(&self) -> Option<(i64, i64)> {
        let mut times = self.snapshots.values().filter_map(|q| q.as_of);
        let first = times.next()?;
        Some(times.fold((first, first), |(min, max), time| {
            (min.min(time), max.max(time))
        }))
    }
    pub fn set_history(&mut self, symbol: Symbol, history: Result<Vec<Candle>, String>) {
        match history {
            Ok(mut rows) => {
                rows.sort_by_key(|c| c.time);
                rows.dedup_by_key(|c| c.time);
                let history = self.histories.entry(symbol.clone()).or_default();
                for row in rows {
                    match history.binary_search_by_key(&row.time, |c| c.time) {
                        Ok(i) => history[i] = row,
                        Err(i) => history.insert(i, row),
                    }
                }
                if history.len() > 100 {
                    history.drain(..history.len() - 100);
                }
                self.history_errors.remove(&symbol);
            }
            Err(reason) => {
                self.history_errors.insert(symbol, reason);
            }
        }
    }
}
