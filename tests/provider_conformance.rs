use futures::StreamExt;
use twstock_gpui::market::*;
use std::collections::VecDeque;

struct FixtureHttp(VecDeque<Result<String, String>>);
impl HttpTransport for FixtureHttp {
    fn get(&mut self, _: &str) -> Result<String, String> {
        self.0.pop_front().expect("unexpected request")
    }
}
fn contract(events: Vec<MarketEvent>, cap: Capabilities) {
    assert!(matches!(
        events.first(),
        Some(MarketEvent::Link(LinkState::Connecting))
    ));
    let mut store = MarketStore::new(cap);
    for event in events {
        store.apply(event, 1_768_457_700);
    }
    assert!(matches!(store.link, LinkState::Live));
    assert!(store.snapshots().count() > 0);
    assert!(
        store
            .snapshots()
            .all(|q| q.last.is_some() || q.volume == Some(0))
    );
}
#[test]
fn mock_and_fixture_mis_share_the_contract() {
    let mut mock = MockProvider::new();
    let symbols = mock
        .instruments()
        .iter()
        .map(|i| i.symbol.clone())
        .collect::<Vec<_>>();
    let events = futures::executor::block_on(mock.subscribe(symbols).collect());
    contract(events, mock.capabilities());
    let mut mis = MisProvider::with_transport(FixtureHttp(VecDeque::from([
        Ok("session".into()),
        Ok(include_str!("fixtures/mis.json").into()),
    ])));
    let symbols = vec![
        Symbol::new(Market::Listed, "889901").unwrap(),
        Symbol::new(Market::Listed, "889902").unwrap(),
    ];
    let events = mis.poll_once(&symbols, 1_768_457_700);
    contract(events, mis.capabilities());
}
#[test]
fn failures_and_reconnects_emit_link_states() {
    let mut provider = MisProvider::with_transport(FixtureHttp(VecDeque::from([
        Ok("session".into()),
        Ok(r#"{"rtcode":"5000","msgArray":[]}"#.into()),
        Ok("renewed".into()),
        Ok(include_str!("fixtures/mis.json").into()),
    ])));
    let symbols = vec![
        Symbol::new(Market::Listed, "889901").unwrap(),
        Symbol::new(Market::Listed, "889902").unwrap(),
    ];
    let failed = provider.poll_once(&symbols, 1_768_457_700);
    assert!(
        matches!(failed.last(),Some(MarketEvent::Link(LinkState::Failed{retry_in,..})) if *retry_in>=5)
    );
    let recovered = provider.poll_once(&symbols, 1_768_457_701);
    contract(recovered, provider.capabilities());
}
#[test]
fn bootstrap_404_is_nonfatal_and_closed_dash_is_not_stale() {
    use chrono::TimeZone;
    let now = chrono::FixedOffset::east_opt(8 * 3600)
        .unwrap()
        .with_ymd_and_hms(2026, 1, 15, 14, 0, 0)
        .unwrap()
        .timestamp();
    let fixture = include_str!("fixtures/mis.json")
        .replace("10:15:00", "13:30:00")
        .replace("98.15", "-");
    let mut provider = MisProvider::with_transport(FixtureHttp(VecDeque::from([
        Err("HTTP 404".into()),
        Ok(fixture),
    ])));
    let symbols = vec![
        Symbol::new(Market::Listed, "889901").unwrap(),
        Symbol::new(Market::Listed, "889902").unwrap(),
    ];
    let events = provider.poll_once(&symbols, now);
    assert!(
        matches!(events.last(), Some(MarketEvent::Link(LinkState::Live))),
        "{events:?}"
    );
    let mut store = MarketStore::new(provider.capabilities());
    for event in events {
        store.apply(event, now);
    }
    assert!(store.snapshot(&symbols[0]).unwrap().last.is_none());
    store.tick(now + 3600);
    assert_eq!(store.link, LinkState::Live);
    assert!(store.status_text(now + 3600).contains("休市"));
}
#[test]
fn live_smoke_opt_in_only() {
    if std::env::var("TWSTOCK_GPUI_LIVE").as_deref() != Ok("1") {
        return;
    }
    let codes = std::env::var("TWSTOCK_GPUI_LIVE_SYMBOLS")
        .expect("live symbols must be supplied outside the repository");
    let symbols = codes
        .split(',')
        .map(|c| Symbol::new(Market::Listed, c).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(symbols.len(), 3);
    let mut mis = MisProvider::new().unwrap();
    let now = now();
    let events = mis.poll_once(&symbols, now);
    let mut store = MarketStore::new(mis.capabilities());
    for e in events {
        store.apply(e, now);
    }
    let mut failures = Vec::new();
    if store.link != LinkState::Live {
        failures.push(format!("quotes: {}", store.status_text(now)));
    }
    if store.link == LinkState::Live {
        for symbol in &symbols {
            let q = store.snapshot(symbol).expect("requested quote missing");
            assert!(
                q.prev_close.is_some() && q.as_of.is_some(),
                "quote metadata missing"
            );
        }
        if store.snapshots().all(|q| {
            matches!(
                q.session,
                Some(SessionState::Closed | SessionState::PostClose)
            )
        }) {
            store.tick(now + 60);
            assert_eq!(store.link, LinkState::Live);
            assert!(store.status_text(now + 60).contains("休市"));
        }
        eprintln!("live market status: {}", store.status_text(now));
    }
    let mut history = HistoryProvider::new().unwrap();
    for symbol in symbols {
        match history.recent(&symbol, 100, now) {
            Ok(rows) => {
                assert!(rows.len() >= 100);
                eprintln!(
                    "live smoke {}: {} history candles; quote {}",
                    symbol.code,
                    rows.len(),
                    if store.snapshot(&symbol).is_some() {
                        "received"
                    } else {
                        "unavailable"
                    }
                );
            }
            Err(reason) => failures.push(format!("history {}: {reason}", symbol.code)),
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn r3_type_failure_enters_backoff_and_r10_success_emits_poll_clock() {
    struct Body(String);
    impl HttpTransport for Body {
        fn get(&mut self, _: &str) -> Result<String, String> {
            Ok(self.0.clone())
        }
    }
    let symbols = vec![Symbol::new(Market::Listed, "889901").unwrap()];
    let body = include_str!("fixtures/mis.json");
    let mut good = MisProvider::with_transport(Body(body.into()));
    assert!(
        good.poll_once(&symbols, 100)
            .iter()
            .any(|event| matches!(event, MarketEvent::Polled { at: 100 }))
    );
    let bad = body.replace("\"v\":\"123\"", "\"v\":123");
    let mut bad = MisProvider::with_transport(Body(bad));
    for retry in [5, 10] {
        let events = bad.poll_once(&symbols, 100);
        assert!(
            !events
                .iter()
                .any(|event| matches!(event, MarketEvent::Polled { .. } | MarketEvent::Quote(_)))
        );
        assert!(events.iter().any(|event|matches!(event,MarketEvent::Link(LinkState::Failed {retry_in,..}) if *retry_in==retry)));
    }
}

#[test]
fn r8_history_provider_recovers_after_an_overflow_response() {
    struct Responses(std::collections::VecDeque<String>);
    impl HttpTransport for Responses {
        fn get(&mut self, _: &str) -> Result<String, String> {
            Ok(self.0.pop_front().unwrap())
        }
    }
    let body = include_str!("fixtures/history.json");
    let mut provider = HistoryProvider::with_transport(Responses(
        [body.replace("115/01/02", "2147483647/01/02"), body.into()].into(),
    ));
    let symbol = Symbol::new(Market::Listed, "889901").unwrap();
    let range = DateRange {
        start: chrono::NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(),
        end: chrono::NaiveDate::from_ymd_opt(2026, 1, 31).unwrap(),
    };
    assert!(
        provider
            .candles(&symbol, Timeframe::Day, range.clone())
            .is_err()
    );
    assert_eq!(
        provider
            .candles(&symbol, Timeframe::Day, range)
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn subscriptions_change_without_recreating_the_feed() {
    use futures::StreamExt;
    struct Fixture;
    impl HttpTransport for Fixture {
        fn get(&mut self, _: &str) -> Result<String, String> {
            Ok(include_str!("fixtures/mis.json").into())
        }
    }
    let a = Symbol::new(Market::Listed, "889901").unwrap();
    let b = Symbol::new(Market::Listed, "889902").unwrap();
    let mut provider = MisProvider::with_transport(Fixture);
    let mut stream = provider.subscribe(vec![a.clone()]);
    futures::executor::block_on(async {
        loop {
            if matches!(stream.next().await,Some(MarketEvent::Quote(q)) if q.symbol==a) {
                break;
            }
        }
        provider.resubscribe(vec![b.clone()]);
        let timeout = futures::channel::oneshot::channel();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_secs(12));
            let _ = timeout.0.send(());
        });
        let next = async {
            loop {
                if let Some(MarketEvent::Quote(q)) = stream.next().await
                    && q.symbol == b
                {
                    return;
                }
            }
        };
        futures::pin_mut!(next);
        assert!(
            matches!(
                futures::future::select(next, timeout.1).await,
                futures::future::Either::Left(_)
            ),
            "subscription update did not reach the existing worker"
        );
    });
}
