use chrono::{NaiveDate, TimeZone};
use twstock_gpui::market::*;
fn symbol() -> Symbol {
    Symbol::new(Market::Listed, "889901").unwrap()
}
fn time(day: u32) -> i64 {
    chrono::FixedOffset::east_opt(8 * 3600)
        .unwrap()
        .with_ymd_and_hms(2026, 1, day, 11, 0, 0)
        .unwrap()
        .timestamp()
}
fn update(day: u32, close: &str) -> QuoteUpdate {
    let p = close.parse::<Price>().unwrap();
    QuoteUpdate {
        symbol: symbol(),
        as_of: Some(time(day)),
        last: Some(p),
        open: Some(p),
        high: Some(p),
        low: Some(p),
        volume: Some(7000),
        session: Some(SessionState::Open),
        ..Default::default()
    }
}
fn store() -> MarketStore {
    let mut s = MarketStore::new(Capabilities::mis());
    s.add_instrument(Instrument::new(symbol()));
    s
}
#[test]
fn r1_rollover_clears_daily_fields_before_missing_values_merge() {
    let mut s = store();
    s.apply(MarketEvent::Quote(update(14, "61.20")), time(14));
    s.apply(
        MarketEvent::Quote(QuoteUpdate {
            symbol: symbol(),
            as_of: Some(time(15)),
            volume: Some(0),
            ..Default::default()
        }),
        time(15),
    );
    let q = s.snapshot(&symbol()).unwrap();
    assert!(q.last.is_none() && q.open.is_none() && q.high.is_none() && q.low.is_none());
    assert_eq!(q.volume, Some(0));
    assert!(
        s.chart_series(&symbol())
            .unwrap()
            .candles
            .iter()
            .all(|c| c.time != NaiveDate::from_ymd_opt(2026, 1, 15).unwrap())
    );
}
#[test]
fn r2_rollover_archives_every_day_and_ma_survives_late_history() {
    let mut s = store();
    s.set_history(symbol(), Ok(vec![]));
    for day in 10..=16 {
        s.apply(MarketEvent::Quote(update(day, &day.to_string())), time(day));
    }
    let rows = s.chart_series(&symbol()).unwrap();
    assert_eq!(
        rows.candles.iter().map(|c| c.time).collect::<Vec<_>>(),
        (10..=16)
            .map(|d| NaiveDate::from_ymd_opt(2026, 1, d).unwrap())
            .collect::<Vec<_>>()
    );
    assert_eq!(rows.averages[0][6], Some("14".parse().unwrap()));
    s.set_history(symbol(), Ok(vec![rows.candles[0].clone()]));
    assert_eq!(
        s.chart_series(&symbol()).unwrap().candles.len(),
        7,
        "a late one-shot historical load must not erase archived days"
    );
}
#[test]
fn r3_wrong_protocol_types_are_errors_not_missing_fields() {
    for key in [
        "v", "b", "a", "g", "f", "z", "o", "h", "l", "y", "u", "w", "n",
    ] {
        for bad in [
            serde_json::json!(123),
            serde_json::json!(true),
            serde_json::Value::Null,
            serde_json::json!({}),
            serde_json::json!([]),
            serde_json::json!(""),
        ] {
            let mut v: serde_json::Value =
                serde_json::from_str(include_str!("fixtures/mis.json")).unwrap();
            v["msgArray"][0][key] = bad;
            assert!(
                parse_mis(&v.to_string(), time(15)).is_err(),
                "{key} cannot silently become missing"
            );
        }
    }
    for missing in [
        serde_json::Value::String("-".into()),
        serde_json::Value::Null,
    ] {
        let mut v: serde_json::Value =
            serde_json::from_str(include_str!("fixtures/mis.json")).unwrap();
        if missing.is_null() {
            v["msgArray"][0].as_object_mut().unwrap().remove("v");
        } else {
            v["msgArray"][0]["v"] = missing;
        }
        assert!(parse_mis(&v.to_string(), time(15)).is_ok());
    }
}
#[test]
fn r4_negative_book_prices_fail_the_whole_poll() {
    for (old, new) in [
        ("98.10_98.05", "-98.10_98.05"),
        ("98.20_98.25", "-98.20_98.25"),
    ] {
        let body = include_str!("fixtures/mis.json").replace(old, new);
        assert!(parse_mis(&body, time(15)).is_err());
    }
}
#[test]
fn r6_limit_distance_walks_actual_stock_and_etf_ticks() {
    for (code, limit, near, far) in [
        ("0099X", "50", "49.98", "49.93"),
        ("889901", "10", "9.98", "9.95"),
        ("889901", "50", "49.90", "49.85"),
        ("889901", "100", "99.80", "99.70"),
        ("889901", "500", "499.00", "498.50"),
        ("889901", "1000", "998.00", "997.00"),
    ] {
        let symbol = Symbol::new(Market::Listed, code).unwrap();
        let i = Instrument::new(symbol.clone());
        let mut s = MarketStore::new(Capabilities::mis());
        s.add_instrument(i.clone());
        for (last, expected) in [(near, true), (far, false)] {
            s.apply(
                MarketEvent::Quote(QuoteUpdate {
                    symbol: symbol.clone(),
                    as_of: Some(time(15)),
                    last: Some(last.parse().unwrap()),
                    limit_up: Some(limit.parse().unwrap()),
                    ..Default::default()
                }),
                time(15),
            );
            assert_eq!(
                s.snapshot(&symbol).unwrap().near_limits(&i),
                expected,
                "{code} upper {limit}, last {last}"
            );
        }
    }
    for (code, limit, near, far) in [
        ("0099X", "49.99", "50.05", "50.10"),
        ("889901", "9.99", "10.05", "10.10"),
        ("889901", "49.95", "50.10", "50.20"),
        ("889901", "99.90", "100.50", "101.00"),
        ("889901", "499.50", "501.00", "502.00"),
        ("889901", "999.00", "1005.00", "1010.00"),
    ] {
        let symbol = Symbol::new(Market::Listed, code).unwrap();
        let i = Instrument::new(symbol.clone());
        let mut s = MarketStore::new(Capabilities::mis());
        s.add_instrument(i.clone());
        for (last, expected) in [(near, true), (far, false)] {
            s.apply(
                MarketEvent::Quote(QuoteUpdate {
                    symbol: symbol.clone(),
                    as_of: Some(time(15)),
                    last: Some(last.parse().unwrap()),
                    limit_down: Some(limit.parse().unwrap()),
                    ..Default::default()
                }),
                time(15),
            );
            assert_eq!(
                s.snapshot(&symbol).unwrap().near_limits(&i),
                expected,
                "{code} lower {limit}, last {last}"
            );
        }
    }
}
#[test]
fn r8_roc_year_overflow_returns_error_without_unwinding() {
    let bad = include_str!("fixtures/history.json").replace("115/01/02", "2147483647/01/02");
    assert!(
        std::panic::catch_unwind(|| parse_history(&bad))
            .expect("worker must not panic")
            .is_err()
    );
    assert!(parse_history(include_str!("fixtures/history.json")).is_ok());
}
#[test]
fn r9_midpoint_is_reference_only_and_never_becomes_a_candle() {
    let mut s = store();
    let mut q = update(15, "61.20");
    q.last = None;
    q.prev_close = Some("60".parse().unwrap());
    q.bids = Some(vec![Depth {
        price: "61.10".parse().unwrap(),
        quantity: 1000,
        as_of: time(15),
    }]);
    q.asks = Some(vec![Depth {
        price: "61.30".parse().unwrap(),
        quantity: 1000,
        as_of: time(15),
    }]);
    s.apply(MarketEvent::Quote(q), time(15));
    let q = s.snapshot(&symbol()).unwrap();
    assert_eq!(q.reference_price(), Some("61.20".parse().unwrap()));
    assert!(q.last.is_none());
    assert!(
        s.chart_series(&symbol())
            .is_none_or(|series| series.candles.is_empty())
    );
    s.apply(
        MarketEvent::Quote(QuoteUpdate {
            symbol: symbol(),
            as_of: Some(time(15) + 5),
            last: Some("61.25".parse().unwrap()),
            ..Default::default()
        }),
        time(15) + 5,
    );
    assert!(s.snapshot(&symbol()).unwrap().reference_price().is_none());
}
#[test]
fn r10_fresh_poll_is_independent_of_old_trade_timestamps() {
    let mut s = store();
    s.apply(MarketEvent::Quote(update(15, "61.20")), time(15));
    s.apply(MarketEvent::Link(LinkState::Live), time(15));
    s.apply(MarketEvent::Polled { at: time(15) + 300 }, time(15) + 300);
    s.tick(time(15) + 305);
    assert_eq!(s.link, LinkState::Live);
    assert!(s.status_text(time(15) + 305).contains("最後更新 5 秒前"));
    s.tick(time(15) + 316);
    assert!(matches!(s.link, LinkState::Stale { .. }));
}

#[test]
fn r2_rollover_refreshes_an_existing_history_date() {
    let mut s = store();
    let mut old = update(14, "15");
    s.apply(MarketEvent::Quote(old.clone()), time(14));
    let row = s.snapshot(&symbol()).unwrap();
    s.set_history(
        symbol(),
        Ok(vec![Candle {
            time: local_time(time(14)).date_naive(),
            open: row.open.unwrap(),
            high: row.high.unwrap(),
            low: row.low.unwrap(),
            close: Price(100000),
            volume: 1,
        }]),
    );
    old.last = Some(Price(160000));
    old.high = Some(Price(170000));
    old.as_of = Some(time(14) + 1);
    s.apply(MarketEvent::Quote(old), time(14) + 1);
    s.apply(MarketEvent::Quote(update(15, "17")), time(15));
    assert_eq!(s.histories[&symbol()][0].close, Price(160000));
}
