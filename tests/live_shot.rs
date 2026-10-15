//! Opt-in real-data capture. AppKit initialization stays on the main thread.
use futures::StreamExt;
use gpui_kit::{
    AppContext, Bounds, HeadlessAppContext, WindowBounds, WindowOptions,
    assets::Assets,
    component::{ThemeMode, TitleBar},
    px, size,
    test::TestWindowExt,
};
use twstock_gpui::{Workspace, market::*};
use std::{
    sync::{Arc, mpsc},
    thread,
    time::{Duration, Instant},
};

enum Loaded {
    Quote(Vec<MarketEvent>),
    History(Symbol, Result<Vec<Candle>, String>),
    Error(String),
}
fn ready(store: &MarketStore) -> bool {
    store.link == LinkState::Live
        && !store.order.is_empty()
        && store.order.iter().all(|symbol| {
            store.snapshot(symbol).is_some_and(|q| q.as_of.is_some())
                && store
                    .histories
                    .get(symbol)
                    .is_some_and(|rows| rows.len() >= 100)
        })
}
fn load() -> MarketStore {
    let instruments = load_watchlist()
        .expect("invalid private watchlist")
        .expect("private watchlist required; mock is forbidden");
    assert!(
        instruments.len() <= Capabilities::mis().max_subscriptions,
        "watchlist exceeds MIS subscription capacity"
    );
    let directory = IsinDirectory::new()
        .and_then(|mut source| source.instruments())
        .expect("live directory metadata required");
    let mut store = MarketStore::new(Capabilities::mis());
    for instrument in instruments {
        let canonical = directory
            .iter()
            .find(|i| i.symbol == instrument.symbol)
            .expect("configured symbol must be in supported directory")
            .clone();
        store.add_instrument(canonical);
    }
    assert!(
        !ready(&store),
        "an empty-data store cannot be capture-ready"
    );
    let started = Instant::now();
    let (tx, rx) = mpsc::channel();
    let symbols = store.subscriptions(&[]);
    let quote_symbols = symbols.clone();
    let quotes = tx.clone();
    thread::spawn(move || match MisProvider::new() {
        Ok(mut provider) => futures::executor::block_on(async move {
            let mut events = provider.subscribe(quote_symbols);
            while let Some(event) = events.next().await {
                if quotes.send(Loaded::Quote(vec![event])).is_err() {
                    break;
                }
            }
        }),
        Err(error) => {
            let _ = quotes.send(Loaded::Error(error));
        }
    });
    for symbol in symbols {
        let history = tx.clone();
        thread::spawn(move || {
            let rows = HistoryProvider::new()
                .and_then(|mut provider| provider.recent(&symbol, 100, now()));
            let _ = history.send(Loaded::History(symbol, rows));
        });
    }
    drop(tx);
    while !ready(&store) {
        let remaining = Duration::from_secs(20)
            .checked_sub(started.elapsed())
            .expect("live data exceeded the 20-second deadline");
        match rx.recv_timeout(remaining).expect(
            "live data incomplete after 20 seconds (or workers disconnected); no mock fallback",
        ) {
            Loaded::Quote(events) => {
                for event in events {
                    if let MarketEvent::Link(LinkState::Failed { reason, .. }) = &event {
                        panic!("live quotes failed: {reason}");
                    }
                    store.apply(event, now());
                }
            }
            Loaded::History(symbol, result) => {
                store.set_history(symbol, Ok(result.expect("live history failed")));
            }
            Loaded::Error(error) => panic!("live provider initialization failed: {error}"),
        }
    }
    assert!(
        started.elapsed() <= Duration::from_secs(20),
        "live data exceeded the 20-second deadline"
    );
    store.tick(now());
    for (index, symbol) in store.order.iter().enumerate() {
        let day = store.chart_series(symbol).expect("loaded daily series");
        eprintln!(
            "live periods #{}: daily {}; weekly {}; monthly {}",
            index + 1,
            day.candles.len(),
            day.clone()
                .for_period(Timeframe::Week)
                .unwrap()
                .candles
                .len(),
            day.for_period(Timeframe::Month).unwrap().candles.len()
        );
    }
    eprintln!(
        "live shot: {} instruments ready in {:.1}s; {}; data {:?}",
        store.instruments.len(),
        started.elapsed().as_secs_f32(),
        store.status_text(now()),
        store
            .data_time_range()
            .map(|(first, last)| (local_time(first).to_string(), local_time(last).to_string()))
    );
    store
}
fn capture(store: MarketStore, mode: ThemeMode, path: &str) {
    let mut cx = HeadlessAppContext::with_platform(
        gpui_kit::platform::current_platform(true).text_system(),
        Arc::new(Assets),
        gpui_kit::platform::current_headless_renderer,
    );
    cx.update(gpui_kit::init);
    let (handle, _) = cx
        .update(|cx| {
            gpui_kit::open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                        Default::default(),
                        size(px(1080.), px(720.)),
                    ))),
                    focus: false,
                    show: false,
                    ..TitleBar::window_options()
                },
                cx,
                |window, cx| {
                    twstock_gpui::theme::apply(mode, Some(window), cx);
                    cx.new(|cx| Workspace::with_store(store, window, cx))
                },
            )
        })
        .expect("headless live workspace must open");
    cx.update_window(handle, |_, window, cx| window.render_frame(cx))
        .unwrap();
    cx.run_until_parked();
    cx.advance_clock(Duration::from_secs(2));
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        for id in [
            "watchlist-panel",
            "chart-panel",
            "quote-panel",
            "market-link-status",
        ] {
            assert!(window.find(id).visible(), "missing {id}");
        }
        assert!(
            window.try_find("trade-feed").is_none(),
            "MIS must not show mock trades"
        );
    })
    .unwrap();
    cx.run_until_parked();
    let image = cx
        .capture_screenshot(handle)
        .expect("live offscreen capture failed");
    assert_eq!(image.dimensions(), (2160, 1440));
    image.save(path).expect("cannot save /tmp live capture");
    println!("live shot: {path} (2160x1440)");
}
fn main() {
    if std::env::var("TWSTOCK_GPUI_LIVE").as_deref() != Ok("1") {
        println!("live_shot skipped: set TWSTOCK_GPUI_LIVE=1");
        return;
    }
    let store = load();
    capture(
        store.clone(),
        ThemeMode::Dark,
        "/tmp/twstock-gpui-live-dark.png",
    );
    capture(store, ThemeMode::Light, "/tmp/twstock-gpui-live-light.png");
}
