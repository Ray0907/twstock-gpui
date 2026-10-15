use std::time::Instant;
use twstock_gpui::market::*;
fn html(market: &str) -> String {
    format!(
        "<table><tr><td>股票</td></tr><tr><td>889901　編造晶圓</td><td>fake</td><td>2026/01/01</td><td>{market}</td><td>半導體</td><td>ESVUFR</td><td></td></tr><tr><td>ETF</td></tr><tr><td>889902　編造基金</td><td>fake</td><td>2026/01/01</td><td>{market}</td><td></td><td>CEOTHR</td><td></td></tr><tr><td>ETN</td></tr><tr><td>889903　編造票券</td><td></td><td></td><td>{market}</td><td></td><td></td><td></td></tr></table>"
    )
}
#[test]
fn directory_classification_is_official_section_not_code_prefix() {
    assert_eq!(
        Instrument::new(Symbol::new(Market::Listed, "0099X").unwrap()).tick_rule,
        TickRule::Stock
    );
    let items = parse_isin(&html("上市"), Market::Listed).unwrap();
    assert_eq!(items.len(), 2);
    assert_eq!(items[0].tick_rule, TickRule::Stock);
    assert_eq!(items[1].tick_rule, TickRule::Etf);
    assert!(parse_isin(&html("上櫃"), Market::Listed).is_err());
    assert!(parse_isin("blocked by proxy", Market::Listed).is_err());
    let mut unknown = MarketStore::new(Capabilities::mis());
    unknown.apply(
        MarketEvent::Quote(QuoteUpdate {
            symbol: items[1].symbol.clone(),
            instrument: Some(Box::new(items[1].clone())),
            as_of: Some(100),
            ..Default::default()
        }),
        100,
    );
    assert_eq!(
        unknown.instruments[&items[1].symbol].tick_rule,
        TickRule::Etf
    );
    let mut store = MarketStore::new(Capabilities::mis());
    store.add_instrument(items[1].clone());
    store.apply(
        MarketEvent::Quote(QuoteUpdate {
            symbol: items[1].symbol.clone(),
            instrument: Some(Box::new(Instrument::new(items[1].symbol.clone()))),
            as_of: Some(100),
            ..Default::default()
        }),
        100,
    );
    assert_eq!(store.instruments[&items[1].symbol].tick_rule, TickRule::Etf);
}
#[test]
fn search_exact_then_code_prefix_then_name_and_stable_code_market_tiebreak() {
    let mut items = parse_isin(&html("上市"), Market::Listed).unwrap();
    let mut a = items[0].clone();
    a.name = "889".into();
    a.symbol = Symbol::new(Market::Otc, "889905").unwrap();
    items.push(a);
    assert_eq!(
        search_directory(&items, "889")
            .iter()
            .map(|i| i.symbol.code.as_str())
            .collect::<Vec<_>>(),
        vec!["889905", "889901", "889902"]
    );
    assert_eq!(search_directory(&items, "889902")[0].symbol.code, "889902");
    assert_eq!(search_directory(&items, "基金").len(), 1);
    assert!(search_directory(&items, "不存在").is_empty());
    assert_eq!(search_directory(&items, "").len(), 3);
}
#[test]
fn cache_freshness_and_source_failure_never_return_fakes() {
    assert!(directory_fresh(Some(100), 101));
    assert!(!directory_fresh(Some(100), 86500));
    assert!(!directory_fresh(Some(100), 99));
    assert!(!directory_fresh(None, 101));
    struct Source {
        calls: usize,
        fail: bool,
    }
    impl HttpTransport for Source {
        fn get(&mut self, url: &str) -> Result<String, String> {
            self.calls += 1;
            if self.fail {
                Err("directory transport offline".into())
            } else {
                Ok(html(if url.ends_with("=2") {
                    "上市"
                } else {
                    "上櫃"
                }))
            }
        }
    }
    let dir = tempfile::tempdir().unwrap();
    let cache = dir.path().join("directory.json");
    let mut source = IsinDirectory::with_transport(
        Source {
            calls: 0,
            fail: false,
        },
        cache.clone(),
    );
    assert_eq!(source.load_at(100).unwrap().len(), 4);
    assert!(cache.exists());
    let mut cached = IsinDirectory::with_transport(
        Source {
            calls: 0,
            fail: true,
        },
        cache,
    );
    assert_eq!(cached.load_at(101).unwrap().len(), 4);
    assert!(cached.load_at(100 + 86401).unwrap_err().contains("offline"));
}
#[test]
fn directory_live_opt_in() {
    if std::env::var("TWSTOCK_GPUI_LIVE").as_deref() != Ok("1") {
        return;
    }
    let start = Instant::now();
    let mut source = IsinDirectory::new().unwrap();
    let items = source.fetch().unwrap();
    let listed = items
        .iter()
        .filter(|i| i.symbol.market == Market::Listed)
        .count();
    let etf = items
        .iter()
        .filter(|i| i.tick_rule == TickRule::Etf)
        .count();
    assert!(listed > 0 && items.len() > listed && etf > 0);
    eprintln!(
        "directory live: {} instruments; listed {listed}; otc {}; ETF {etf}; {:.2}s",
        items.len(),
        items.len() - listed,
        start.elapsed().as_secs_f64()
    );
}
