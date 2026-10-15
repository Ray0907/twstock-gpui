use super::*;
#[path = "zoom_fix2.rs"]
mod liveness_regressions;
#[path = "zoom_fix.rs"]
mod regressions;
use gpui_kit::component::plot::Plot;
use gpui_kit::{
    AnyWindowHandle, InputEvent, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent,
    PinchEvent, ScrollDelta, ScrollWheelEvent,
};
use std::rc::Rc;
use twstock_gpui::{
    design::chart::{
        self, ChartGeometry, ChartLabels, ChartViewport, TradingPlot, range::VisibleRange,
    },
    market::{ChartSeries, MockProvider, Timeframe},
};

fn frames(cx: &mut HeadlessAppContext, handle: AnyWindowHandle) {
    for _ in 0..3 {
        cx.update_window(handle, |_, w, cx| w.render_frame(cx))
            .unwrap();
        cx.run_until_parked();
    }
}
fn context() -> HeadlessAppContext {
    let mut cx = HeadlessAppContext::with_platform(
        gpui_kit::platform::current_platform(true).text_system(),
        Arc::new(Assets),
        gpui_kit::platform::current_headless_renderer,
    );
    cx.update(gpui_kit::init);
    cx
}
fn options() -> WindowOptions {
    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds::new(
            Default::default(),
            size(px(1080.), px(720.)),
        ))),
        focus: false,
        show: false,
        ..TitleBar::window_options()
    }
}
fn scroll(position: gpui_kit::Point<gpui_kit::Pixels>, x: f32, y: f32) -> ScrollWheelEvent {
    ScrollWheelEvent {
        position,
        delta: ScrollDelta::Pixels(point(px(x), px(y))),
        ..Default::default()
    }
}

pub fn run() {
    liveness_regressions::run();
    regressions::run();
    panel_events();
    shared_range_events();
    trend_ma_clipping();
    println!(
        "zoom: wheel/anchor, pinch, horizontal/drag pan, reset, period/instrument, MA, axes and hover passed"
    );
}
fn panel_events() {
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
    let original = cx
        .update_window(handle, |_, w, _| {
            w.find("candle-region").label().unwrap().to_owned()
        })
        .unwrap();
    cx.update_window(handle, |_, w, cx| {
        let bounds = w.find("candle-region").bounds();
        // Events outside the plot must not zoom it.
        w.dispatch_event(
            scroll(point(px(0.), px(0.)), 0., 200.).to_platform_input(),
            cx,
        );
        w.render_frame(cx);
        assert_eq!(w.find("candle-region").label().unwrap(), original);
        w.dispatch_event(scroll(bounds.center(), 0., 275.).to_platform_input(), cx);
    })
    .unwrap();
    frames(&mut cx, handle);
    cx.update_window(handle, |_, w, cx| {
        let label = w.find("candle-region").label().unwrap().to_owned();
        assert!(
            label.contains("40 / 100 根"),
            "wheel zoom must show visible / full bars: {label}"
        );
        w.dispatch_event(
            MouseMoveEvent {
                position: point(px(0.), px(0.)),
                ..Default::default()
            }
            .to_platform_input(),
            cx,
        );
    })
    .unwrap();
    frames(&mut cx, handle);
    let image = cx.capture_screenshot(handle).unwrap();
    snapshot::assert_snapshot("zoomed-in", &image);
    cx.update_window(handle, |_, w, cx| {
        let bounds = w.find("candle-region").bounds();
        w.dispatch_event(scroll(bounds.center(), 120., 0.).to_platform_input(), cx);
        w.dispatch_event(
            MouseMoveEvent {
                position: point(px(0.), px(0.)),
                ..Default::default()
            }
            .to_platform_input(),
            cx,
        );
    })
    .unwrap();
    frames(&mut cx, handle);
    let panned = cx.capture_screenshot(handle).unwrap();
    assert_ne!(
        image, panned,
        "horizontal scroll must change the visible candles"
    );
    snapshot::assert_snapshot("panned-to-left", &panned);
    cx.update_window(handle, |_, w, cx| {
        let position = w.find("candle-region").bounds().center();
        w.dispatch_event(
            MouseDownEvent {
                position,
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
    cx.update_window(handle, |_, w, cx| {
        assert_eq!(w.find("candle-region").label().unwrap(), original);
        let position = w.find("candle-region").bounds().center();
        w.dispatch_event(scroll(position, 0., 200.).to_platform_input(), cx);
        w.render_frame(cx);
        w.click("period-week", cx);
    })
    .unwrap();
    frames(&mut cx, handle);
    cx.update_window(handle, |_, w, cx| {
        assert!(w.find("candle-region").label().unwrap().starts_with("週線"));
        assert!(!w.find("candle-region").label().unwrap().contains(" / "));
        w.click("period-day", cx);
    })
    .unwrap();
    frames(&mut cx, handle);
    cx.update_window(handle, |_, w, cx| {
        assert_eq!(w.find("candle-region").label().unwrap(), original);
        let position = w.find("candle-region").bounds().center();
        w.dispatch_event(scroll(position, 0., 200.).to_platform_input(), cx);
        w.render_frame(cx);
        assert!(w.find("candle-region").label().unwrap().contains(" / "));
        w.click(("row", 1usize), cx);
    })
    .unwrap();
    frames(&mut cx, handle);
    cx.update_window(handle, |_, w, cx| {
        assert!(!w.find("candle-region").label().unwrap().contains(" / "));
        w.click(("row", 0usize), cx);
    })
    .unwrap();
    frames(&mut cx, handle);
    cx.update_window(handle, |_, w, _| {
        assert_eq!(w.find("candle-region").label().unwrap(), original)
    })
    .unwrap();
}

fn trend_ma_clipping() {
    let store = MockProvider::new().store();
    let mut data = store.chart_series(&store.order[0]).unwrap();
    for (index, candle) in data.candles.iter_mut().enumerate() {
        let price = (index as i64 + 1) * 10000;
        candle.open = twstock_gpui::market::Price(price);
        candle.close = twstock_gpui::market::Price(price + 1000);
        candle.high = twstock_gpui::market::Price(price + 2000);
        candle.low = twstock_gpui::market::Price(price - 1000);
    }
    let data = Rc::new(data.for_period(Timeframe::Day).unwrap());
    let viewport = Rc::new(ChartViewport::new(data.candles.len()));
    viewport.range.set(VisibleRange {
        start: 80,
        count: 20,
    });
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
    let (bounds, g, scale) = cx
        .update_window(handle, |_, w, cx| {
            let bounds = w.find("candle-region").bounds();
            (
                bounds,
                ChartGeometry::for_range(
                    bounds,
                    cx.theme().font_size,
                    &data,
                    None,
                    viewport.range.get(),
                ),
                w.scale_factor(),
            )
        })
        .unwrap();
    assert!(g.price_y(data.averages[2][80].unwrap()) > g.layout.price.bottom());
    let image = cx.capture_screenshot(handle).unwrap();
    image.save("/tmp/zoom-trend.png").unwrap();
    let colors = (0..3)
        .map(|index| {
            color_bytes(gpui_kit::rgb(twstock_gpui::design::color::series_hex(index, true)).into())
        })
        .collect::<Vec<_>>();
    for y in (((bounds.top() + g.layout.price.bottom()).as_f32() + 2.) * scale) as u32
        ..(((bounds.top() + g.layout.volume.bottom()).as_f32() - 2.) * scale) as u32
    {
        for x in ((bounds.left().as_f32() + 2.) * scale) as u32
            ..((bounds.left() + g.layout.price.right()).as_f32() * scale) as u32
        {
            assert!(
                !colors.contains(&image.get_pixel(x, y).0),
                "offscreen MA must not leak into the gap/volume subplot"
            );
        }
    }
}

struct ZoomChart {
    data: Rc<ChartSeries>,
    viewport: Rc<ChartViewport>,
}
impl Render for ZoomChart {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .font_family(cx.theme().font_family.clone())
            .text_size(cx.theme().font_size)
            .child(chart::view_with_viewport(
                self.data.clone(),
                None,
                Timeframe::Day,
                self.viewport.clone(),
                cx,
            ))
    }
}
fn shared_range_events() {
    let store = MockProvider::new().store();
    let data = Rc::new(store.chart_series(&store.order[0]).unwrap());
    let total = data.candles.len();
    let viewport = Rc::new(ChartViewport::new(total));
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
            (w.find("candle-region").bounds(), cx.theme().font_size)
        })
        .unwrap();
    let full = ChartGeometry::new(bounds, font, &data, None);
    let local = point(
        px(full.candle(50, &data.candles[50]).x),
        full.layout.price.center().y,
    );
    let position = bounds.origin + local;
    cx.update_window(handle, |_, w, cx| {
        w.dispatch_event(scroll(position, 0., 275.).to_platform_input(), cx)
    })
    .unwrap();
    frames(&mut cx, handle);
    let range = viewport.range.get();
    assert_eq!(range.count, 40);
    let plot = TradingPlot::with_viewport(data.clone(), None, viewport.clone());
    let hover = cx.update(|cx| plot.tooltip_state(local, bounds, cx).unwrap());
    assert_eq!(
        hover.index, 50,
        "the candle under the wheel anchor must remain there"
    );
    let g = ChartGeometry::for_range(bounds, font, &data, None, range);
    assert!(
        (g.candle(hover.index - range.start, &data.candles[hover.index])
            .x
            - local.x.as_f32())
        .abs()
            <= g.x.step() / 2. + 1.
    );
    assert!(g.x.band_width() > full.x.band_width());
    // Visible axes/clipping must match an independently sliced series, not the full history.
    let mut visible = (*data).clone();
    visible.candles = data.candles[range.start..range.start + range.count].to_vec();
    visible.averages = data
        .averages
        .each_ref()
        .map(|ma| ma[range.start..range.start + range.count].to_vec());
    let expected = ChartGeometry::new(bounds, font, &visible, None);
    assert_eq!(g.price_ticks, expected.price_ticks);
    assert_eq!(g.volume_ticks, expected.volume_ticks);
    assert_eq!(g.volume_clip, expected.volume_clip);
    let far_quote = ChartGeometry::for_range(
        bounds,
        font,
        &data,
        Some(twstock_gpui::market::Price(1)),
        range,
    );
    assert_eq!(
        g.price_ticks, far_quote.price_ticks,
        "an offscreen quote must not change the visible domain"
    );
    let quiet = ChartGeometry::for_range(
        bounds,
        font,
        &data,
        None,
        VisibleRange {
            start: 0,
            count: 20,
        },
    );
    assert!(full.volume_clip.is_some());
    assert!(
        quiet.volume_clip.is_none(),
        "offscreen volume spikes must not clip the visible volume axis"
    );
    assert!(
        visible.averages.iter().all(|ma| ma[0].is_some()),
        "MA must not restart at the visible boundary"
    );
    let first_local = point(
        px(g.candle(0, &data.candles[range.start]).x),
        g.layout.volume.center().y,
    );
    let first = cx.update(|cx| plot.tooltip_state(first_local, bounds, cx).unwrap());
    assert_eq!(first.index, range.start);
    cx.update_window(handle, |_, w, cx| {
        w.dispatch_event(
            MouseMoveEvent {
                position: bounds.origin + first_local,
                ..Default::default()
            }
            .to_platform_input(),
            cx,
        );
        w.render_frame(cx);
        let labels =
            ChartLabels::for_range(&g, &data, None, Some(&first), Timeframe::Day, w, range);
        assert_eq!(
            labels.date.as_ref().unwrap().text.as_ref(),
            chart::date_text(data.candles[range.start].time, Timeframe::Day)
        );
        assert_eq!(
            labels.volume.text.as_ref(),
            twstock_gpui::design::format::volume(
                data.candles[range.start].volume as f64 / data.lot_size as f64
            )
        );
    })
    .unwrap();
    frames(&mut cx, handle);
    let actual_hover = cx.capture_screenshot(handle).unwrap();
    // A sliced reference with inherited full-series MAs must paint identical hover text/tags.
    let reference_data = Rc::new(visible);
    let reference_viewport = Rc::new(ChartViewport::new(range.count));
    let (reference, _) = cx
        .update(|cx| {
            gpui_kit::open_window(options(), cx, |w, cx| {
                twstock_gpui::theme::apply(ThemeMode::Dark, Some(w), cx);
                cx.new(|_| ZoomChart {
                    data: reference_data,
                    viewport: reference_viewport,
                })
            })
        })
        .unwrap();
    frames(&mut cx, reference);
    cx.update_window(reference, |_, w, cx| {
        w.dispatch_event(
            MouseMoveEvent {
                position: bounds.origin + first_local,
                ..Default::default()
            }
            .to_platform_input(),
            cx,
        );
    })
    .unwrap();
    frames(&mut cx, reference);
    assert_eq!(
        actual_hover,
        cx.capture_screenshot(reference).unwrap(),
        "info row, MA values, crosshair date and volume tags must read the full-series candle"
    );
    // Pinch is supported by InputEvent::to_platform_input and is dispatched for real.
    cx.update_window(handle, |_, w, cx| {
        w.dispatch_event(
            PinchEvent {
                position,
                delta: 0.25,
                ..Default::default()
            }
            .to_platform_input(),
            cx,
        )
    })
    .unwrap();
    frames(&mut cx, handle);
    assert!(viewport.range.get().count < range.count);
    let before_pan = viewport.range.get();
    cx.update_window(handle, |_, w, cx| {
        w.dispatch_event(scroll(position, -80., 0.).to_platform_input(), cx)
    })
    .unwrap();
    frames(&mut cx, handle);
    assert!(viewport.range.get().start > before_pan.start);
    let before_drag = viewport.range.get();
    cx.update_window(handle, |_, w, cx| {
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
        w.dispatch_event(
            MouseMoveEvent {
                position: position + point(px(150.), px(0.)),
                pressed_button: Some(MouseButton::Left),
                ..Default::default()
            }
            .to_platform_input(),
            cx,
        );
        w.dispatch_event(
            MouseUpEvent {
                position,
                button: MouseButton::Left,
                ..Default::default()
            }
            .to_platform_input(),
            cx,
        );
    })
    .unwrap();
    frames(&mut cx, handle);
    assert!(viewport.range.get().start < before_drag.start);
    let after_drag = viewport.range.get();
    cx.update_window(handle, |_, w, cx| {
        w.dispatch_event(
            MouseMoveEvent {
                position,
                ..Default::default()
            }
            .to_platform_input(),
            cx,
        );
        for delta in [0., f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            w.dispatch_event(scroll(position, 0., delta).to_platform_input(), cx);
            w.dispatch_event(
                PinchEvent {
                    position,
                    delta,
                    ..Default::default()
                }
                .to_platform_input(),
                cx,
            );
        }
    })
    .unwrap();
    frames(&mut cx, handle);
    assert_eq!(viewport.range.get(), after_drag);
    cx.update_window(handle, |_, w, cx| {
        w.dispatch_event(
            MouseDownEvent {
                position,
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
    assert_eq!(viewport.range.get(), VisibleRange::reset(total));
    for index in [0, 10, 25, 50, 75, 90, total - 1] {
        viewport.reset(total);
        let local = point(
            px(full.candle(index, &data.candles[index]).x),
            full.layout.price.center().y,
        );
        cx.update_window(handle, |_, w, cx| {
            w.refresh();
            w.render_frame(cx);
            w.dispatch_event(
                scroll(bounds.origin + local, 0., 450.).to_platform_input(),
                cx,
            );
        })
        .unwrap();
        frames(&mut cx, handle);
        let hover = cx.update(|cx| plot.tooltip_state(local, bounds, cx).unwrap());
        assert_eq!(hover.index, index, "cursor anchor at candle {index}");
    }
}
