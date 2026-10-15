use twstock_gpui::market::*;
use std::{fs, path::Path};

fn symbol() -> Symbol {
    Symbol::new(Market::Listed, "889901").unwrap()
}
fn update(time: i64, last: Option<&str>) -> QuoteUpdate {
    QuoteUpdate {
        symbol: symbol(),
        as_of: Some(time),
        last: last.map(|p| p.parse().unwrap()),
        session: Some(SessionState::Open),
        ..Default::default()
    }
}
#[test]
fn store_merges_incrementally_rejects_older_and_duplicate_ticks() {
    let mut store = MarketStore::new(Capabilities::mis());
    assert!(store.apply(MarketEvent::Quote(update(100, Some("98.15"))), 100));
    let mut next = update(101, None);
    next.volume = Some(123_000);
    next.prev_close = Some("97.00".parse().unwrap());
    assert!(store.apply(MarketEvent::Quote(next.clone()), 101));
    assert!(!store.apply(MarketEvent::Quote(next), 102));
    assert!(!store.apply(MarketEvent::Quote(update(99, Some("12.00"))), 103));
    let snapshot = store.snapshot(&symbol()).unwrap();
    assert_eq!(snapshot.last, Some("98.15".parse().unwrap()));
    assert_eq!(snapshot.volume, Some(123_000));
    assert_eq!(snapshot.change(), Some("1.15".parse().unwrap()));
}
#[test]
fn freshness_and_closed_sessions_are_different_from_flat_prices() {
    let mut store = MarketStore::new(Capabilities::mis());
    store.apply(MarketEvent::Quote(update(100, Some("98.15"))), 100);
    store.apply(MarketEvent::Link(LinkState::Live), 100);
    store.tick(116);
    assert!(matches!(store.link, LinkState::Stale { since: 100 }));
    let mut closed = update(107, None);
    closed.session = Some(SessionState::Closed);
    store.apply(MarketEvent::Quote(closed), 107);
    store.apply(MarketEvent::Link(LinkState::Live), 107);
    store.tick(1000);
    assert_eq!(store.link, LinkState::Live);
    assert!(store.status_text(1000).contains("休市"));
    store.apply(
        MarketEvent::Link(LinkState::Failed {
            reason: "fixture disconnected".into(),
            retry_in: 5,
        }),
        1001,
    );
    assert!(store.status_text(1001).contains("fixture disconnected"));
}
#[test]
fn watchlist_accepts_singular_symbol_table() {
    let instruments = parse_watchlist("[[symbol]]\nmarket='tse'\ncode='889901'").unwrap();
    assert_eq!(instruments[0].symbol, symbol());
}
#[test]
fn data_time_range_uses_source_time_not_reception_time() {
    let mut store = MarketStore::new(Capabilities::mis());
    assert_eq!(store.data_time_range(), None);
    for (code, time) in [("889901", 100), ("889902", 200)] {
        let symbol = Symbol::new(Market::Listed, code).unwrap();
        store.add_instrument(Instrument::new(symbol.clone()));
        store.apply(
            MarketEvent::Quote(QuoteUpdate {
                symbol,
                as_of: Some(time),
                ..Default::default()
            }),
            999,
        );
    }
    assert_eq!(store.data_time_range(), Some((100, 200)));
}
#[test]
fn price_and_instrument_ticks_are_fixed_point() {
    assert_eq!("97.05".parse::<Price>().unwrap().0, 970500);
    assert!("0.00001".parse::<Price>().is_err());
    assert!("NaN".parse::<Price>().is_err());
    assert_eq!(
        TickRule::Etf.tick("97".parse().unwrap()),
        "0.05".parse().unwrap()
    );
    assert_eq!(
        TickRule::Stock.tick("97".parse().unwrap()),
        "0.10".parse().unwrap()
    );
    assert_eq!(
        TickRule::Etf.tick("49.99".parse().unwrap()),
        "0.01".parse().unwrap()
    );
}
#[test]
fn subscriptions_prioritize_visible_symbols_and_respect_capabilities() {
    let mut cap = Capabilities::mis();
    cap.max_subscriptions = 1;
    let mut store = MarketStore::new(cap);
    let other = Symbol::new(Market::Listed, "889902").unwrap();
    store.add_instrument(Instrument::new(symbol()));
    store.add_instrument(Instrument::new(other.clone()));
    assert_eq!(
        store.subscriptions(std::slice::from_ref(&other)),
        vec![other]
    );
}
#[test]
fn mis_dash_depth_timestamps_and_source_limits() {
    let body = include_str!("fixtures/mis.json");
    let parsed = parse_mis(body, 1_768_457_700).unwrap();
    assert_eq!(parsed.len(), 2);
    assert_eq!(parsed[0].bids.as_ref().unwrap().len(), 5);
    assert_ne!(parsed[0].as_of, parsed[1].as_of);
    assert_eq!(parsed[0].limit_up, Some("111.30".parse().unwrap()));
    assert_eq!(parsed[0].limit_down, Some("91.70".parse().unwrap()));
    assert!(parsed[1].last.is_none());
    let mut store = MarketStore::new(Capabilities::mis());
    store.apply(
        MarketEvent::Quote(update(parsed[0].as_of.unwrap() - 1, Some("98.10"))),
        1_768_457_700,
    );
    let mut no_trade = parsed[0].clone();
    no_trade.last = None;
    store.apply(MarketEvent::Quote(no_trade), 1_768_457_700);
    assert_eq!(
        store.snapshot(&symbol()).unwrap().last,
        Some("98.10".parse().unwrap())
    );
    assert!(parse_mis(r#"{"rtcode":"5000","msgArray":[]}"#, 1).is_err());
}
#[test]
fn history_roc_commas_and_fractional_lots() {
    let candles = parse_history(include_str!("fixtures/history.json")).unwrap();
    assert_eq!(candles[0].time.to_string(), "2026-01-02");
    assert_eq!(candles[0].open, "1,234.50".parse::<Price>().unwrap());
    assert_eq!(candles[0].volume, 1_500);
    assert_eq!(candles[0].volume_lots(1000), 1.5);
    assert!(parse_history(r#"{"stat":"not found","data":[]}"#).is_err());
}
fn scan(path: &Path) {
    for file in fs::read_dir(path).unwrap() {
        let file = file.unwrap().path();
        if file.is_dir() {
            scan(&file);
            continue;
        }
        if file.extension().is_none_or(|e| e != "rs") {
            continue;
        }
        for (line, text) in fs::read_to_string(&file).unwrap().lines().enumerate() {
            if text.trim_start().starts_with("//") {
                continue;
            }
            assert!(
                ![
                    "use gpui",
                    "gpui::",
                    "gpui_kit::",
                    "crate::design",
                    "crate::panels",
                    "crate::quotes"
                ]
                .iter()
                .any(|p| text.contains(p)),
                "{}:{}: market may not depend on UI",
                file.display(),
                line + 1
            );
        }
    }
}
#[test]
fn duplicate_poll_confirms_freshness_without_reapplying_prices() {
    let mut store = MarketStore::new(Capabilities::mis());
    let event = MarketEvent::Quote(update(100, Some("98.15")));
    store.apply(event.clone(), 100);
    store.apply(MarketEvent::Link(LinkState::Live), 100);
    assert!(!store.apply(event, 104));
    store.tick(108);
    assert_eq!(store.link, LinkState::Live);
    store.tick(120);
    assert!(matches!(store.link, LinkState::Stale { since: 104 }));
}
#[test]
fn zero_subscription_capacity_and_invalid_configuration_are_safe() {
    let mut cap = Capabilities::mis();
    cap.max_subscriptions = 0;
    let mut store = MarketStore::new(cap);
    store.add_instrument(Instrument::new(symbol()));
    assert!(store.subscriptions(&[]).is_empty());
    assert!(parse_watchlist("[[watchlist]]\nmarket='tse'\ncode='../bad'").is_err());
    let config = parse_watchlist(
        "[[watchlist]]\nmarket='tse'\ncode='889901'\n[[watchlist]]\nmarket='tse'\ncode='889901'",
    )
    .unwrap();
    assert_eq!(config.len(), 1);
    assert!(Symbol::new(Market::Listed, "0099X").is_ok());
}
#[test]
fn history_backfills_an_empty_current_month() {
    use std::collections::VecDeque;
    struct Http(VecDeque<String>);
    impl HttpTransport for Http {
        fn get(&mut self, _: &str) -> Result<String, String> {
            Ok(self.0.pop_front().expect("unexpected month"))
        }
    }
    let history = include_str!("fixtures/history.json").replace("115/01/02", "114/12/02");
    let mut provider = HistoryProvider::with_transport(Http(VecDeque::from([
        r#"{"stat":"很抱歉，沒有符合條件的資料!","data":[]}"#.into(),
        history,
    ])));
    let rows = provider.recent(&symbol(), 1, 1_768_457_700).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].time.to_string(), "2025-12-02");
}
#[test]
fn limit_proximity_uses_instrument_ticks_without_floating_point() {
    let mut instrument = Instrument::new(Symbol::new(Market::Listed, "0099X").unwrap());
    let mut store = MarketStore::new(Capabilities::mis());
    let mut quote = update(100, Some("97.00"));
    quote.limit_up = Some("97.10".parse().unwrap());
    store.apply(MarketEvent::Quote(quote), 100);
    let mut snapshot = store.snapshot(&symbol()).unwrap().clone();
    assert!(snapshot.near_limits(&instrument));
    snapshot.last = Some("96.95".parse().unwrap());
    assert!(!snapshot.near_limits(&instrument));
    instrument.tick_rule = TickRule::Stock;
    assert!(snapshot.near_limits(&instrument));
}
#[test]
fn market_has_no_gpui_dependency() {
    scan(&Path::new(env!("CARGO_MANIFEST_DIR")).join("src/market"));
}
