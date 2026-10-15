use super::regressions::{chart_image, chart_label};
use super::*;
use std::cell::Cell;

pub(super) fn run() {
    let mut failures = Vec::new();
    cancel_paths(&mut failures);
    removed_plot(&mut failures);
    same_frame_scroll(&mut failures);
    assert!(
        failures.is_empty(),
        "drag liveness regressions:\n{}",
        failures.join("\n")
    );
    println!(
        "drag liveness: cancel paths, plot removal, outside drag, current-scale scroll/zoom passed"
    );
}
fn down(position: gpui_kit::Point<gpui_kit::Pixels>, w: &mut Window, cx: &mut gpui_kit::App) {
    w.dispatch_event(
        MouseDownEvent {
            position,
            button: MouseButton::Left,
            click_count: 1,
            ..Default::default()
        }
        .to_platform_input(),
        cx,
    );
}
fn motion(
    position: gpui_kit::Point<gpui_kit::Pixels>,
    held: bool,
    w: &mut Window,
    cx: &mut gpui_kit::App,
) {
    w.dispatch_event(
        MouseMoveEvent {
            position,
            pressed_button: held.then_some(MouseButton::Left),
            ..Default::default()
        }
        .to_platform_input(),
        cx,
    );
}
fn key(key: &str, w: &mut Window, cx: &mut gpui_kit::App) {
    w.dispatch_event(
        gpui_kit::KeyDownEvent {
            keystroke: gpui_kit::Keystroke::parse(key).unwrap(),
            is_held: false,
            prefer_character_input: false,
        }
        .to_platform_input(),
        cx,
    );
}
fn cancel_paths(failures: &mut Vec<String>) {
    for path in [
        "released-outside-window",
        "overlay",
        "deactivation",
        "not-painted",
        "double-click",
    ] {
        let mut cx = context();
        let (handle, _) = cx
            .update(|cx| {
                gpui_kit::open_window(options(), cx, |w, cx| {
                    twstock_gpui::theme::apply(ThemeMode::Dark, Some(w), cx);
                    cx.new(|cx| Workspace::new(w, cx))
                })
            })
            .unwrap();
        frames(&mut cx, handle);
        cx.update_window(handle, |_, w, _| w.activate_window())
            .unwrap();
        frames(&mut cx, handle);
        let bounds = cx
            .update_window(handle, |_, w, cx| {
                let bounds = w.find("candle-region").bounds();
                w.dispatch_event(scroll(bounds.center(), 0., 275.).to_platform_input(), cx);
                bounds
            })
            .unwrap();
        frames(&mut cx, handle);
        let before = chart_image(&mut cx, handle);
        cx.update_window(handle, |_, w, cx| down(bounds.center(), w, cx))
            .unwrap();
        match path {
            "released-outside-window" => {
                // The OS did not send MouseUp; the next move reports the real button state.
                cx.update_window(handle, |_, w, cx| {
                    motion(point(px(-10.), px(-10.)), false, w, cx)
                })
                .unwrap();
            }
            "overlay" => {
                cx.update_window(handle, |_, w, cx| key("cmd-k", w, cx))
                    .unwrap();
                frames(&mut cx, handle);
                cx.update_window(handle, |_, w, cx| {
                    assert!(w.try_find("directory-palette").is_some());
                    let backdrop = point(bounds.left() + px(12.), bounds.bottom() - px(12.));
                    assert!(!w.find("directory-palette").bounds().contains(&backdrop));
                    motion(backdrop, true, w, cx);
                    key("escape", w, cx);
                })
                .unwrap();
                frames(&mut cx, handle);
            }
            "deactivation" => {
                let (other, _) = cx
                    .update(|cx| {
                        gpui_kit::open_window(options(), cx, |_, cx| cx.new(|_| FontWeights))
                    })
                    .unwrap();
                cx.update_window(other, |_, w, _| w.activate_window())
                    .unwrap();
                frames(&mut cx, handle);
                assert!(
                    !cx.update_window(handle, |_, w, _| w.is_window_active())
                        .unwrap()
                );
                cx.update_window(handle, |_, w, _| w.activate_window())
                    .unwrap();
                frames(&mut cx, handle);
            }
            "not-painted" => {
                cx.update_window(handle, |_, w, cx| {
                    w.resize(size(px(1080.), px(200.)));
                    w.bounds_changed(cx);
                })
                .unwrap();
                frames(&mut cx, handle);
                cx.update_window(handle, |_, w, cx| {
                    assert_eq!(w.find("panel-state-text").label().unwrap(), "圖表空間不足");
                    w.dispatch_event(
                        MouseUpEvent {
                            position: point(px(0.), px(0.)),
                            button: MouseButton::Left,
                            ..Default::default()
                        }
                        .to_platform_input(),
                        cx,
                    );
                    w.resize(size(px(1080.), px(720.)));
                    w.bounds_changed(cx);
                })
                .unwrap();
                frames(&mut cx, handle);
            }
            "double-click" => {
                cx.update_window(handle, |_, w, cx| {
                    w.dispatch_event(
                        MouseDownEvent {
                            position: bounds.center(),
                            button: MouseButton::Left,
                            click_count: 2,
                            ..Default::default()
                        }
                        .to_platform_input(),
                        cx,
                    );
                })
                .unwrap();
                frames(&mut cx, handle);
            }
            _ => unreachable!(),
        }
        cx.update_window(handle, |_, w, cx| {
            // A later press starts outside the window: no fresh plot MouseDown is delivered.
            down(point(px(-10.), px(-10.)), w, cx);
            motion(bounds.center() - point(px(160.), px(0.)), true, w, cx);
        })
        .unwrap();
        frames(&mut cx, handle);
        if path == "double-click" {
            if chart_label(&mut cx, handle).contains(" / ") {
                failures.push(format!("{path}: stale drag restored zoom after reset"));
            }
        } else if chart_image(&mut cx, handle) != before {
            failures.push(format!(
                "{path}: a later move without a fresh plot MouseDown panned the chart"
            ));
        }
    }
}
struct ConditionalChart {
    data: Rc<ChartSeries>,
    viewport: Rc<ChartViewport>,
    shown: Rc<Cell<bool>>,
}
impl Render for ConditionalChart {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut root = v_flex().size_full();
        if self.shown.get() {
            root = root.child(chart::view_with_viewport(
                self.data.clone(),
                None,
                Timeframe::Day,
                self.viewport.clone(),
                cx,
            ));
        }
        root
    }
}
fn removed_plot(failures: &mut Vec<String>) {
    let store = MockProvider::new().store();
    let data = Rc::new(store.chart_series(&store.order[0]).unwrap());
    let viewport = Rc::new(ChartViewport::new(data.candles.len()));
    let shown = Rc::new(Cell::new(true));
    let mut cx = context();
    let (handle, _) = cx
        .update(|cx| {
            gpui_kit::open_window(options(), cx, |_, cx| {
                cx.new(|_| ConditionalChart {
                    data: data.clone(),
                    viewport: viewport.clone(),
                    shown: shown.clone(),
                })
            })
        })
        .unwrap();
    frames(&mut cx, handle);
    let bounds = cx
        .update_window(handle, |_, w, cx| {
            let bounds = w.find("candle-region").bounds();
            w.dispatch_event(scroll(bounds.center(), 0., 275.).to_platform_input(), cx);
            bounds
        })
        .unwrap();
    frames(&mut cx, handle);
    let before = viewport.range.get();
    cx.update_window(handle, |_, w, cx| down(bounds.center(), w, cx))
        .unwrap();
    shown.set(false);
    cx.update_window(handle, |_, w, _| w.refresh()).unwrap();
    frames(&mut cx, handle);
    cx.update_window(handle, |_, w, cx| {
        assert!(w.try_find("candle-region").is_none());
        w.dispatch_event(
            MouseUpEvent {
                position: point(px(0.), px(0.)),
                button: MouseButton::Left,
                ..Default::default()
            }
            .to_platform_input(),
            cx,
        );
    })
    .unwrap();
    shown.set(true);
    cx.update_window(handle, |_, w, _| w.refresh()).unwrap();
    frames(&mut cx, handle);
    cx.update_window(handle, |_, w, cx| {
        down(point(px(-10.), px(-10.)), w, cx);
        motion(bounds.center() - point(px(200.), px(0.)), true, w, cx);
    })
    .unwrap();
    if viewport.range.get() != before {
        failures.push("removed plot: drag survived removal and a missed MouseUp".into());
    }
    // Drag capture must continue outside bounds when nothing is occluding the chart.
    let normal_start = viewport.range.get();
    cx.update_window(handle, |_, w, cx| {
        motion(bounds.center(), false, w, cx);
        down(bounds.center(), w, cx);
        motion(
            point(bounds.left() - px(200.), bounds.center().y),
            true,
            w,
            cx,
        );
    })
    .unwrap();
    let outside = viewport.range.get();
    assert!(
        outside.start > normal_start.start,
        "normal drag outside chart must keep panning"
    );
    cx.update_window(handle, |_, w, cx| motion(bounds.center(), true, w, cx))
        .unwrap();
    if viewport.range.get() != normal_start {
        failures.push("normal outside drag was incorrectly cancelled".into());
    }
}
fn same_frame_scroll(failures: &mut Vec<String>) {
    let store = MockProvider::new().store();
    let data = Rc::new(store.chart_series(&store.order[0]).unwrap());
    let viewport = Rc::new(ChartViewport::new(data.candles.len()));
    let mut cx = context();
    let (handle, _) = cx
        .update(|cx| {
            gpui_kit::open_window(options(), cx, |w, cx| {
                twstock_gpui::theme::apply(ThemeMode::Dark, Some(w), cx);
                cx.new(|_| ZoomChart {
                    data: data.clone(),
                    viewport: viewport.clone(),
                })
            })
        })
        .unwrap();
    frames(&mut cx, handle);
    let (bounds, font) = cx
        .update_window(handle, |_, w, cx| {
            let bounds = w.find("candle-region").bounds();
            w.dispatch_event(scroll(bounds.center(), 0., 275.).to_platform_input(), cx);
            (bounds, cx.theme().font_size)
        })
        .unwrap();
    frames(&mut cx, handle);
    let baseline = viewport.range.get();
    assert_eq!(baseline.count, 40);
    for first in ["pinch", "wheel"] {
        for second in ["horizontal", "wheel", "pinch"] {
            let mut expected = None;
            for redraw in [true, false] {
                viewport.reset(data.candles.len());
                viewport.range.set(baseline);
                cx.update_window(handle, |_, w, _| w.refresh()).unwrap();
                frames(&mut cx, handle);
                let (zoomed, local) = cx
                    .update_window(handle, |_, w, cx| {
                        if first == "pinch" {
                            w.dispatch_event(
                                PinchEvent {
                                    position: bounds.center(),
                                    delta: 1.,
                                    ..Default::default()
                                }
                                .to_platform_input(),
                                cx,
                            );
                        } else {
                            w.dispatch_event(
                                scroll(bounds.center(), 0., 275.).to_platform_input(),
                                cx,
                            );
                        }
                        let zoomed = viewport.range.get();
                        assert_eq!(zoomed.count, 20);
                        let g = ChartGeometry::for_range(bounds, font, &data, None, zoomed);
                        let local = point(
                            px(g.candle(8, &data.candles[zoomed.start + 8]).x),
                            g.layout.price.center().y,
                        );
                        if redraw {
                            w.render_frame(cx);
                        }
                        let position = bounds.origin + local;
                        match second {
                            "horizontal" => {
                                w.dispatch_event(
                                    scroll(position, -g.x.step() * 5., 0.).to_platform_input(),
                                    cx,
                                );
                            }
                            "wheel" => {
                                w.dispatch_event(
                                    scroll(position, 0., -120.).to_platform_input(),
                                    cx,
                                );
                            }
                            "pinch" => {
                                w.dispatch_event(
                                    PinchEvent {
                                        position,
                                        delta: -0.4,
                                        ..Default::default()
                                    }
                                    .to_platform_input(),
                                    cx,
                                );
                            }
                            _ => unreachable!(),
                        }
                        (zoomed, local)
                    })
                    .unwrap();
                let actual = viewport.range.get();
                if redraw {
                    expected = Some(actual);
                    if second == "horizontal" {
                        assert_eq!(actual, zoomed.pan_by(data.candles.len(), 5.));
                    }
                } else if Some(actual) != expected {
                    failures.push(format!(
                        "same-frame {first} -> {second}: got {actual:?}, expected {:?}",
                        expected.unwrap()
                    ));
                }
                if second != "horizontal" {
                    let plot = TradingPlot::with_viewport(data.clone(), None, viewport.clone());
                    let hover = cx.update(|cx| plot.tooltip_state(local, bounds, cx).unwrap());
                    if hover.index != zoomed.start + 8 {
                        failures.push(format!("same-frame {first} -> {second}: zoom anchor moved"));
                    }
                }
            }
        }
    }
}
