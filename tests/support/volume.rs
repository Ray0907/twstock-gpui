use super::*;
use gpui_kit::component::plot::TooltipState;
use gpui_kit::{AnyWindowHandle, InputEvent, Modifiers, MouseMoveEvent, Pixels};
use twstock_gpui::{
    design::chart::{ChartGeometry, ChartLabels},
    market::*,
};
fn overlaps(a: Bounds<Pixels>, b: Bounds<Pixels>) -> bool {
    a.left() < b.right() && b.left() < a.right() && a.top() < b.bottom() && b.top() < a.bottom()
}
pub fn check(
    cx: &mut HeadlessAppContext,
    handle: AnyWindowHandle,
    bounds: Bounds<Pixels>,
    data: &ChartSeries,
    current: Option<Price>,
) {
    cx.update_window(handle, |_, w, cx| {
        let mut clipped_seen = false;
        let mut hidden_date = false;
        let mut hidden_price = false;
        let mut hidden_volume = false;
        for period in [Timeframe::Day, Timeframe::Week, Timeframe::Month] {
            let mut data = data.clone().for_period(period).unwrap();
            data.candles[0].volume = 0;
            for width in [bounds.size.width, px(240.)] {
                let g = ChartGeometry::new(
                    Bounds::new(point(px(0.), px(0.)), size(width, bounds.size.height)),
                    cx.theme().font_size,
                    &data,
                    current,
                );
                for index in 0..data.candles.len() {
                    let c = &data.candles[index];
                    let hover = TooltipState::new(
                        index,
                        point(px(g.candle(index, c).x), g.price_y(c.close)),
                        vec![],
                    );
                    let labels = ChartLabels::new(&g, &data, current, Some(&hover), period, w);
                    assert!(
                        (labels.volume.bounds.center().y - g.volume_bar(index, c.volume).top())
                            .abs()
                            < px(0.01),
                        "label and bar must use the identical clipped scale"
                    );
                    assert_eq!(
                        labels.volume.text.as_ref(),
                        twstock_gpui::design::format::volume(c.volume as f64 / data.lot_size as f64)
                    );
                    if index == 0 {
                        assert_eq!(labels.volume.text.as_ref(), "0");
                    }
                    if g.clipped_volume(c.volume).is_some() {
                        clipped_seen = true;
                        assert!(labels.volume.text.contains(','));
                        assert!(c.volume > g.volume_clip.unwrap());
                    }
                    hidden_date |= labels.date_ticks.len() < 5.min(data.candles.len());
                    hidden_price |= labels.price_ticks.len() < g.price_ticks.values.len();
                    hidden_volume |= labels.volume_ticks.len() < g.volume_ticks.values.len();
                    for tag in labels
                        .price
                        .iter()
                        .chain(std::iter::once(&labels.volume))
                        .chain(labels.date.iter())
                    {
                        for tick in labels
                            .price_ticks
                            .iter()
                            .chain(&labels.volume_ticks)
                            .chain(&labels.date_ticks)
                        {
                            assert!(
                                !overlaps(tag.bounds, tick.bounds),
                                "tag {} intersects visible tick {}",
                                tag.text,
                                tick.text
                            );
                        }
                    }
                    assert_eq!(
                        labels,
                        ChartLabels::new(&g, &data, current, Some(&hover), period, w)
                    );
                }
                let idle = ChartLabels::new(&g, &data, current, None, period, w);
                assert!(idle.date.is_none());
                let last = data.candles.last().unwrap();
                assert!(
                    (idle.volume.bounds.center().y
                        - g.volume_bar(data.candles.len() - 1, last.volume).top())
                    .abs()
                        < px(0.01)
                );
            }
        }
        assert!(clipped_seen, "fixture must exercise clipped volumes");
        assert!(
            hidden_date && hidden_price && hidden_volume,
            "all three axes must actually suppress intersecting ticks"
        );
    })
    .unwrap();
    let (g, gold, line_gold) = cx
        .update_window(handle, |_, _, cx| {
            (
                ChartGeometry::new(bounds, cx.theme().font_size, data, current),
                color_bytes(cx.theme().primary),
                color_bytes(cx.theme().link),
            )
        })
        .unwrap();
    let index = 50.min(data.candles.len() - 1);
    let candle = &data.candles[index];
    let x = px(g.candle(index, candle).x);
    let y = g.layout.volume.bottom() - g.layout.volume.size.height / 3.;
    cx.update_window(handle, |_, w, cx| {
        w.dispatch_event(
            MouseMoveEvent {
                position: bounds.origin + point(x, y),
                pressed_button: None,
                modifiers: Modifiers::default(),
            }
            .to_platform_input(),
            cx,
        );
        w.render_frame(cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, w, cx| w.render_frame(cx))
        .unwrap();
    let image = cx.capture_screenshot(handle).unwrap();
    image.save("/tmp/twstock-gpui-volume-hover.png").unwrap();
    let (tag, scale) = cx
        .update_window(handle, |_, w, _| {
            let hover = TooltipState::new(index, point(x, g.price_y(candle.close)), vec![]);
            (
                ChartLabels::new(&g, data, current, Some(&hover), Timeframe::Day, w)
                    .volume
                    .bounds,
                w.scale_factor(),
            )
        })
        .unwrap();
    let line_x =
        ((bounds.left() + g.layout.volume.right() - px(8.)).as_f32() * scale).round() as u32;
    let line_y =
        ((bounds.top() + g.volume_bar(index, candle.volume).top()).as_f32() * scale).round() as u32;
    assert!(
        (line_y.saturating_sub(2)..=line_y + 2).any(|y| image.get_pixel(line_x, y).0 == line_gold),
        "volume crosshair must follow the capped bar top, not the pointer y"
    );
    let contains_gold = |raw: &[u8], width: u32| {
        let frame = Bounds::new(bounds.origin + tag.origin, tag.size);
        ((frame.top().as_f32() * scale).ceil() as u32
            ..(frame.bottom().as_f32() * scale).floor() as u32)
            .any(|y| {
                ((frame.left().as_f32() * scale).ceil() as u32
                    ..(frame.right().as_f32() * scale).floor() as u32)
                    .any(|x| {
                        raw[((y * width + x) * 4) as usize..((y * width + x) * 4 + 4) as usize]
                            == gold
                    })
            })
    };
    assert!(
        contains_gold(image.as_raw(), image.width()),
        "volume hover must paint a gold actual-value label at the bar top"
    );
    cx.update_window(handle, |_, w, cx| w.render_frame(cx))
        .unwrap();
    assert_eq!(
        image.into_raw(),
        cx.capture_screenshot(handle).unwrap().into_raw(),
        "hover rendering must repeat exactly"
    );
    cx.update_window(handle, |_, w, cx| {
        w.hover("watch-name-991901", cx);
        w.render_frame(cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, w, cx| w.render_frame(cx))
        .unwrap();
    let left = cx.capture_screenshot(handle).unwrap();
    assert!(
        !contains_gold(left.as_raw(), left.width()),
        "active volume label must disappear after pointer exits"
    );
}
