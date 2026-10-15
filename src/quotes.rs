//! UI projection only. Providers and the canonical snapshots live in market.
pub use crate::market::Market;
use crate::market::{self, MarketStore, Price};
use gpui_kit::SharedString;
#[derive(Clone)]
pub struct Trade {
    pub time: SharedString,
    pub price: f64,
    pub quantity: u64,
    pub change: f64,
}
#[derive(Clone)]
pub struct Quote {
    pub code: SharedString,
    pub name: SharedString,
    pub market: Market,
    pub last: f64,
    pub is_reference: bool,
    pub has_traded: bool,
    pub prev_close: f64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub limit_up: f64,
    pub limit_down: f64,
    pub volume: u64,
    pub volume_known: bool,
    pub bids: Vec<(f64, u64)>,
    pub asks: Vec<(f64, u64)>,
    pub closes: Vec<f64>,
    pub trades: Vec<Trade>,
    pub has_trades: bool,
    pub history_error: Option<String>,
    near_limit: bool,
    computed_change: f64,
    computed_change_percent: f64,
}
fn value(price: Option<Price>) -> f64 {
    price.map_or(f64::NAN, |p| p.0 as f64 / 10000.)
}
impl Quote {
    pub fn change(&self) -> f64 {
        self.computed_change
    }
    pub fn change_percent(&self) -> f64 {
        self.computed_change_percent
    }
    pub fn near_limits(&self) -> bool {
        self.near_limit
    }
}
pub fn from_store(store: &MarketStore, row: usize) -> Option<Quote> {
    let symbol = store.order.get(row)?;
    let instrument = store.instruments.get(symbol)?;
    let q = store.snapshot(symbol)?;
    let closes = store
        .histories
        .get(symbol)
        .into_iter()
        .flatten()
        .map(|c| value(Some(c.close)))
        .collect();
    let trades = q
        .trades
        .iter()
        .enumerate()
        .map(|(ix, t)| Trade {
            time: market::local_time(t.time)
                .format("%H:%M:%S")
                .to_string()
                .into(),
            price: value(Some(t.price)),
            quantity: t.quantity / instrument.lot_size,
            change: q
                .trades
                .get(ix + 1)
                .map_or(0., |old| value(Some(t.price)) - value(Some(old.price))),
        })
        .collect();
    let book = |levels: &Vec<market::Depth>| {
        levels
            .iter()
            .map(|d| (value(Some(d.price)), d.quantity / instrument.lot_size))
            .collect()
    };
    Some(Quote {
        code: symbol.code.clone().into(),
        name: instrument.name.clone().into(),
        market: symbol.market,
        last: value(q.display_price()),
        is_reference: q.reference_price().is_some(),
        has_traded: q.volume.is_some_and(|v| v > 0),
        prev_close: value(q.prev_close),
        open: value(q.open),
        high: value(q.high),
        low: value(q.low),
        limit_up: value(q.limit_up),
        limit_down: value(q.limit_down),
        volume: q.volume.unwrap_or(0) / instrument.lot_size,
        volume_known: q.volume.is_some(),
        bids: book(&q.bids),
        asks: book(&q.asks),
        closes,
        trades,
        has_trades: store.capabilities.has_trades,
        history_error: store.history_errors.get(symbol).cloned(),
        near_limit: q.near_limits(instrument),
        computed_change: value(q.display_change()),
        computed_change_percent: q
            .display_change_percent_units()
            .map_or(f64::NAN, |p| p as f64 / 10000.),
    })
}
pub fn format_price(price: f64) -> String {
    if !price.is_finite() {
        "—".into()
    } else if price >= 1000. && price.fract().abs() < 1e-9 {
        format!("{price:.0}")
    } else {
        format!("{price:.2}")
    }
}
#[cfg(test)]
pub fn sample() -> Vec<Quote> {
    let store = market::MockProvider::new().store();
    (0..store.order.len())
        .filter_map(|row| from_store(&store, row))
        .collect()
}
#[cfg(test)]
mod tests {
    #[test]
    fn r9_reference_projection_does_not_claim_a_trade() {
        use crate::market::*;
        let mut store = MarketStore::new(Capabilities::mis());
        let symbol = Symbol::new(Market::Listed, "889901").unwrap();
        store.add_instrument(Instrument::new(symbol.clone()));
        store.apply(
            MarketEvent::Quote(QuoteUpdate {
                symbol: symbol.clone(),
                as_of: Some(100),
                prev_close: Some(Price(120000)),
                volume: Some(7000),
                bids: Some(vec![Depth {
                    price: Price(123000),
                    quantity: 1000,
                    as_of: 100,
                }]),
                asks: Some(vec![Depth {
                    price: Price(125000),
                    quantity: 1000,
                    as_of: 100,
                }]),
                ..Default::default()
            }),
            100,
        );
        let q = super::from_store(&store, 0).unwrap();
        assert!(q.is_reference);
        assert_eq!(q.last, 12.4);
        assert!(q.change() > 0.);
        store.apply(
            MarketEvent::Quote(QuoteUpdate {
                symbol,
                as_of: Some(101),
                last: Some(Price(124500)),
                ..Default::default()
            }),
            101,
        );
        assert!(!super::from_store(&store, 0).unwrap().is_reference);
    }

    #[test]
    fn missing_last_is_not_a_synthetic_price() {
        let mut store = crate::market::MarketStore::new(crate::market::Capabilities::mis());
        store.add_instrument(crate::market::Instrument::new(
            crate::market::Symbol::new(crate::market::Market::Listed, "889901").unwrap(),
        ));
        assert_eq!(
            super::format_price(super::from_store(&store, 0).unwrap().last),
            "—"
        );
    }
}
