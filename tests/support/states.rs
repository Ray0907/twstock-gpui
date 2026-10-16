use super::*;
use twstock_gpui::market::*;
struct ChartHost {
    store: MarketStore,
    small: bool,
    dimensions: (f32, f32),
    history: std::sync::mpsc::Receiver<Result<Vec<Candle>, String>>,
}
impl Render for ChartHost {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let symbol = self.store.order[0].clone();
        if let Ok(result) = self.history.try_recv() {
            self.store.set_history(symbol, result);
        }
        let symbol = &self.store.order[0];
        gpui_kit::component::v_flex()
            .w(px(if self.small { 160. } else { self.dimensions.0 }))
            .h(px(if self.small { 90. } else { self.dimensions.1 }))
            .child(twstock_gpui::design::chart::history_view(
                self.store.chart_series(symbol).map(|s| (s, None)),
                self.store.history_errors.get(symbol).map(String::as_str),
                Timeframe::Day,
                cx,
            ))
    }
}
struct PendingHistory;
impl History for PendingHistory {
    fn candles(&mut self, _: &Symbol, _: Timeframe, _: DateRange) -> Result<Vec<Candle>, String> {
        loop {
            std::thread::park();
        }
    }
}
pub fn run() {
    for mode in [ThemeMode::Dark, ThemeMode::Light] {
        for (width, height) in [(1080., 720.), (900., 600.)] {
            let mut cx = HeadlessAppContext::with_platform(
                gpui_kit::platform::current_platform(true).text_system(),
                Arc::new(Assets),
                gpui_kit::platform::current_headless_renderer,
            );
            cx.update(gpui_kit::init);
            let (sender, history) = std::sync::mpsc::channel();
            let pending_sender = sender.clone();
            let (handle, host) = cx
                .update(|cx| {
                    gpui_kit::open_window(
                        WindowOptions {
                            window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                                point(px(0.), px(0.)),
                                size(px(width), px(height)),
                            ))),
                            focus: false,
                            show: false,
                            ..Default::default()
                        },
                        cx,
                        |w, cx| {
                            twstock_gpui::theme::apply(mode, Some(w), cx);
                            cx.new(|_| {
                                let mut store = MockProvider::new().store();
                                store.histories.clear();
                                ChartHost {
                                    store,
                                    small: false,
                                    dimensions: (width, height),
                                    history,
                                }
                            })
                        },
                    )
                })
                .unwrap();
            let symbol = Symbol::new(Market::Listed, "9910").unwrap();
            std::thread::spawn(move || {
                let result = PendingHistory.candles(
                    &symbol,
                    Timeframe::Day,
                    DateRange {
                        start: chrono::NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(),
                        end: chrono::NaiveDate::from_ymd_opt(2026, 1, 2).unwrap(),
                    },
                );
                let _ = pending_sender.send(result);
            });
            for state in ["loading", "failed", "empty", "small"] {
                cx.update(|cx| {
                    host.update(cx, |h, cx| {
                        h.small = state == "small";
                        let s = &mut h.store;
                        let symbol = s.order[0].clone();
                        s.histories.clear();
                        s.history_errors.clear();
                        if state == "failed" {
                            s.history_errors
                                .insert(symbol.clone(), "編造的失敗原因".into());
                        }
                        if state == "empty" {
                            let instrument = s.instruments[&symbol].clone();
                            *s = MarketStore::new(Capabilities::mis());
                            s.add_instrument(instrument);
                            s.histories.insert(symbol, vec![]);
                        }
                        if state == "small" {
                            *s = MockProvider::new().store();
                        }
                        cx.notify();
                    })
                });
                for _ in 0..4 {
                    cx.update_window(handle, |_, w, cx| w.render_frame(cx))
                        .unwrap();
                    cx.run_until_parked();
                }
                cx.update_window(handle, |_, w, cx| {
                    let r = w.find("chart-region").bounds();
                    let c = w.find("panel-state-content").bounds();
                    let tolerance = cx.theme().font_size.as_f32();
                    assert!(
                        (r.center().x - c.center().x).as_f32().abs() < tolerance,
                        "{state}: horizontal"
                    );
                    assert!(
                        (r.center().y - c.center().y).as_f32().abs() < tolerance,
                        "{state}: vertical"
                    );
                    let text = w.find("panel-state-text");
                    let label = text.label().unwrap();
                    assert!(label.contains(match state {
                        "loading" => "載入中",
                        "failed" => "編造的失敗原因",
                        "empty" => "無資料",
                        _ => "空間不足",
                    }));
                })
                .unwrap();
                if state == "loading" && mode == ThemeMode::Dark && width == 1080. {
                    cx.capture_screenshot(handle)
                        .unwrap()
                        .save("/tmp/twstock-gpui-loading.png")
                        .unwrap();
                }
            }
            cx.update(|cx| {
                host.update(cx, |h, cx| {
                    h.small = false;
                    h.store.histories.clear();
                    cx.notify();
                })
            });
            sender
                .send(Err("temporary history failure".into()))
                .unwrap();
            for _ in 0..2 {
                cx.update_window(handle, |_, w, cx| w.render_frame(cx))
                    .unwrap();
                cx.run_until_parked();
            }
            // Keep the failure reason visible while a retry has no result yet.
            cx.update(|cx| host.update(cx, |_, cx| cx.notify()));
            cx.update_window(handle, |_, w, cx| {
                w.render_frame(cx);
                assert_eq!(
                    w.find("panel-state-text").label().unwrap(),
                    "載入失敗：temporary history failure"
                );
                assert!(w.try_find("candle-region").is_none());
            })
            .unwrap();
            let loaded = MockProvider::new().store();
            sender
                .send(Ok(loaded.histories[&loaded.order[0]].clone()))
                .unwrap();
            cx.update(|cx| {
                host.update(cx, |h, cx| {
                    h.small = false;
                    h.store.histories.clear();
                    cx.notify();
                })
            });
            for _ in 0..4 {
                cx.update_window(handle, |_, w, cx| w.render_frame(cx))
                    .unwrap();
                cx.run_until_parked();
            }
            cx.update_window(handle, |_, w, _| {
                assert!(w.try_find("panel-state-content").is_none());
                assert!(w.try_find("candle-region").is_some());
            })
            .unwrap();
        }
    }
}
