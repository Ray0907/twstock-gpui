use super::*;
use chrono::{Duration, FixedOffset, NaiveDate, TimeZone};
use futures::StreamExt;

fn mock_limits(previous: Price, rule: TickRule) -> (Price, Price) {
    let upper = previous.0 * 110;
    let lower = previous.0 * 90;
    let up_tick = rule.tick(Price(upper / 100)).0;
    let down_tick = rule.tick(Price(lower / 100)).0;
    (
        Price(upper / (100 * up_tick) * up_tick),
        Price((lower + 100 * down_tick - 1) / (100 * down_tick) * down_tick),
    )
}
pub struct MockProvider {
    instruments: Vec<Instrument>,
    events: Vec<MarketEvent>,
    histories: Vec<(Symbol, Vec<Candle>)>,
}
struct Rng(u64);
impl Rng {
    fn next(&mut self, n: u64) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.0 >> 32) % n
    }
}
impl Default for MockProvider {
    fn default() -> Self {
        Self::new()
    }
}
impl MockProvider {
    pub fn new() -> Self {
        let names = [
            "範例電子",
            "示意航運",
            "樣本生技",
            "模擬金控",
            "測試半導體",
            "假想食品",
            "練習鋼鐵",
            "演練光電",
            "虛構通訊",
            "樣本材料",
            "模擬零售",
            "假想能源",
            "練習軟體",
            "示範物流",
            "測試設備",
        ];
        let previous = [
            1285000, 463500, 3120000, 248000, 10850000, 86400, 312000, 764000, 1855000, 568000,
            397000, 2480000, 1685000, 672000, 4150000,
        ];
        let changes = [-9, -5, 5, -13, 4, 1, 2, -3, 6, 0, -4, 8, -2, 3, -6];
        let date = NaiveDate::from_ymd_opt(2026, 10, 13).unwrap();
        let as_of = FixedOffset::east_opt(8 * 3600)
            .unwrap()
            .from_local_datetime(&date.and_hms_opt(13, 29, 59).unwrap())
            .single()
            .unwrap()
            .timestamp();
        let mut result = Self {
            instruments: vec![],
            events: vec![MarketEvent::Link(LinkState::Connecting)],
            histories: vec![],
        };
        for ix in 0..names.len() {
            let symbol = Symbol::new(
                if ix % 3 == 2 {
                    Market::Otc
                } else {
                    Market::Listed
                },
                &format!("9919{:02}", ix + 1),
            )
            .unwrap();
            let mut instrument = Instrument::new(symbol.clone());
            instrument.name = names[ix].into();
            instrument.limit_rule = LimitRule::Percent(10);
            let rule = instrument.tick_rule;
            let prev_close = Price(previous[ix]);
            let tick = rule.tick(prev_close).0;
            let last = Price(prev_close.0 + changes[ix] * tick);
            let mut rng = Rng((ix + 1) as u64);
            let open = rule.round(Price(last.0 + ((rng.next(7) as i64) - 3) * tick));
            let high = Price(last.0.max(open.0) + 2 * tick);
            let low = Price(last.0.min(open.0) - tick);
            let (limit_up, limit_down) = mock_limits(prev_close, rule);
            let depth = |buy: bool, rng: &mut Rng| {
                (0..5)
                    .map(|level| Depth {
                        price: rule.round(Price(
                            last.0
                                + if buy {
                                    -level * tick
                                } else {
                                    (level + 1) * tick
                                },
                        )),
                        quantity: (50 + rng.next(350)) * 1000,
                        as_of,
                    })
                    .collect()
            };
            let bids = depth(true, &mut rng);
            let asks = depth(false, &mut rng);
            let volume = (if ix == 0 { 6146 } else { 2000 + rng.next(8000) }) * 1000;
            result.events.push(MarketEvent::Quote(QuoteUpdate {
                symbol: symbol.clone(),
                instrument: Some(Box::new(instrument.clone())),
                last: Some(last),
                volume: Some(volume),
                open: Some(open),
                high: Some(high),
                low: Some(low),
                prev_close: Some(prev_close),
                limit_up: Some(limit_up),
                limit_down: Some(limit_down),
                bids: Some(bids),
                asks: Some(asks),
                as_of: Some(as_of),
                source: Some("Mock".into()),
                session: Some(SessionState::Closed),
            }));
            let mut price = prev_close;
            let mut candles = Vec::new();
            for day in (0..99).rev() {
                let tick = rule.tick(price).0;
                let open = rule.round(Price(price.0 + (rng.next(9) as i64 - 4) * tick));
                candles.push(Candle {
                    time: date - Duration::days(99 - day),
                    open,
                    close: price,
                    high: Price(open.0.max(price.0) + 2 * tick),
                    low: Price(open.0.min(price.0) - tick),
                    volume: (1000 + rng.next(9000)) * 1000,
                });
                price = open;
            }
            candles.reverse();
            // A fictional one-day spike exercises the robust volume scale in screenshots.
            candles[50].volume = 600_000_000;
            result.histories.push((symbol.clone(), candles));
            for trade in (0..12).rev() {
                result.events.push(MarketEvent::Trade(Trade {
                    symbol: symbol.clone(),
                    time: as_of - trade * 2,
                    price: if trade == 0 {
                        last
                    } else {
                        rule.round(Price(last.0 + (rng.next(7) as i64 - 3) * tick))
                    },
                    quantity: (1 + rng.next(80)) * 1000,
                    side: None,
                }));
            }
            result.instruments.push(instrument);
        }
        result.events.push(MarketEvent::Link(LinkState::Live));
        result
    }
    pub fn instruments(&self) -> &[Instrument] {
        &self.instruments
    }
    pub fn store(&self) -> MarketStore {
        let mut store = MarketStore::new(self.capabilities());
        store.indices = [
            ("模擬指數", 218435700, 6200),
            ("示意中小", 2674100, -3500),
            ("虛構科技", 5128300, 10800),
        ]
        .map(|(name, last, change_percent_units)| IndexSnapshot {
            name: name.into(),
            last: Price(last),
            change_percent_units,
        })
        .to_vec();
        for i in &self.instruments {
            store.add_instrument(i.clone());
        }
        for event in self.events.clone() {
            store.apply(event, super::now());
        }
        for (symbol, rows) in &self.histories {
            store.set_history(symbol.clone(), Ok(rows.clone()));
        }
        store
    }
}
impl QuoteFeed for MockProvider {
    fn capabilities(&self) -> Capabilities {
        Capabilities {
            has_trades: true,
            ..Capabilities::mis()
        }
    }
    fn subscribe(&mut self, symbols: Vec<Symbol>) -> EventStream {
        let symbols = symbols
            .into_iter()
            .take(self.capabilities().max_subscriptions)
            .collect::<Vec<_>>();
        let events = self
            .events
            .iter()
            .filter(|event| match event {
                MarketEvent::Quote(q) => symbols.contains(&q.symbol),
                MarketEvent::Trade(t) => symbols.contains(&t.symbol),
                MarketEvent::Link(_) | MarketEvent::Polled { .. } => true,
            })
            .cloned()
            .collect::<Vec<_>>();
        futures::stream::iter(events).boxed()
    }
}
impl History for MockProvider {
    fn candles(
        &mut self,
        symbol: &Symbol,
        _: Timeframe,
        range: DateRange,
    ) -> Result<Vec<Candle>, String> {
        Ok(self
            .histories
            .iter()
            .find(|(s, _)| s == symbol)
            .ok_or("unknown mock instrument")?
            .1
            .iter()
            .filter(|c| c.time >= range.start && c.time <= range.end)
            .cloned()
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mock_limits_round_inward_using_each_boundary_tick() {
        for (previous, up, down) in [
            ("128.50", "141", "116"),
            ("24.80", "27.25", "22.35"),
            ("1285", "1410", "1160"),
            ("100", "110", "90"),
            ("49.95", "54.90", "45.00"),
            ("9.10", "10", "8.19"),
        ] {
            assert_eq!(
                mock_limits(previous.parse().unwrap(), TickRule::Stock),
                (up.parse().unwrap(), down.parse().unwrap())
            );
        }
        assert_eq!(
            mock_limits("97.05".parse().unwrap(), TickRule::Etf),
            ("106.75".parse().unwrap(), "87.35".parse().unwrap())
        );
    }
}
