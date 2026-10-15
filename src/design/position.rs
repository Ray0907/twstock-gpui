//! Today's visual scale and label layout use Kit plot primitives.
use super::{
    chart::{bar, label, line},
    color::{self, ColorRole},
    text::{self, TextRole},
};
use crate::{
    market::{Price, TickRule},
    quotes::Quote,
};
use gpui_kit::{
    component::{
        ActiveTheme,
        plot::{
            Plot, PlotAxis, PlotElement,
            label::measure_text_width,
            scale::{Scale, ScaleLinear},
        },
    },
    *,
};

pub struct PositionLabel {
    pub name: &'static str,
    pub value: f64,
    pub x: Pixels,
    pub bounds: Bounds<Pixels>,
}
pub struct PositionLayout {
    pub domain: [f64; 2],
    pub axis: Bounds<Pixels>,
    pub labels: Vec<PositionLabel>,
    scale: ScaleLinear<f64>,
}
impl PositionLayout {
    /// Local plot coordinates; labels may move, but their price ticks never do.
    pub fn new(
        markers: &[(&'static str, f64)],
        rule: TickRule,
        frame: Size<Pixels>,
        base_font: Pixels,
        window: &mut Window,
    ) -> Option<Self> {
        let mut markers: Vec<_> = markers
            .iter()
            .copied()
            .filter(|(_, value)| value.is_finite())
            .collect();
        if markers.is_empty() || frame.width < px(100.) {
            return None;
        }
        markers.sort_by(|a, b| a.1.total_cmp(&b.1));
        let min = markers.first().unwrap().1;
        let max = markers.last().unwrap().1;
        let tick = [min, max]
            .map(|value| rule.tick(Price((value * 10000.).round() as i64)).0)
            .into_iter()
            .max()
            .unwrap() as f64
            / 10000.;
        let pad = ((max - min) * 0.08).max(tick);
        let domain = [min - pad, max + pad];
        let font = text::pixels(TextRole::Caption, base_font);
        let axis = Bounds::new(
            point(font, font * 2.5),
            size(frame.width - font * 2., px(0.)),
        );
        let scale = ScaleLinear::new(domain, [axis.left().as_f32(), axis.right().as_f32()]);
        let gap = font / 4.;
        let mut labels: Vec<PositionLabel> = Vec::new();
        for (name, value) in markers {
            let width = px(measure_text_width(&name.into(), font, window));
            let x = px(scale.tick(&value).unwrap());
            let left = (x - width / 2.).max(
                labels
                    .last()
                    .map_or(px(0.), |last| last.bounds.right() + gap),
            );
            labels.push(PositionLabel {
                name,
                value,
                x,
                bounds: Bounds::new(point(left, px(0.)), size(width, font * 1.25)),
            });
        }
        for i in (0..labels.len()).rev() {
            let right = if i + 1 == labels.len() {
                frame.width
            } else {
                labels[i + 1].bounds.left() - gap
            };
            labels[i].bounds.origin.x = labels[i]
                .bounds
                .left()
                .min(right - labels[i].bounds.size.width);
        }
        Some(Self {
            domain,
            axis,
            labels,
            scale,
        })
    }
}
pub struct PositionPlot {
    markers: Vec<(&'static str, f64)>,
    low: f64,
    high: f64,
    tick_rule: TickRule,
}
impl PositionPlot {
    pub fn new(q: &Quote) -> Self {
        Self {
            markers: [
                ("低", q.low),
                ("昨收", q.prev_close),
                ("開", q.open),
                ("現", if q.is_reference { f64::NAN } else { q.last }),
                ("高", q.high),
            ]
            .into_iter()
            .filter(|(_, value)| value.is_finite())
            .collect(),
            low: q.low,
            high: q.high,
            tick_rule: q.tick_rule,
        }
    }
}
impl IntoElement for PositionPlot {
    type Element = PlotElement<Self>;
    fn into_element(self) -> Self::Element {
        PlotElement::new(self)
    }
}
impl Plot for PositionPlot {
    fn paint(&mut self, bounds: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
        let Some(layout) = PositionLayout::new(
            &self.markers,
            self.tick_rule,
            bounds.size,
            cx.theme().font_size,
            window,
        ) else {
            return;
        };
        let font = text::pixels(TextRole::Caption, cx.theme().font_size);
        let y = layout.axis.top().as_f32();
        let muted = color::get(ColorRole::Muted, cx);
        let axis_bounds = Bounds::new(
            bounds.origin + layout.axis.origin,
            size(
                layout.axis.size.width,
                bounds.size.height - layout.axis.top(),
            ),
        );
        PlotAxis::new()
            .x(px(0.))
            .stroke(muted)
            .paint(&axis_bounds, window, cx);
        if self.low.is_finite() && self.high.is_finite() && self.low <= self.high {
            let low = layout.scale.tick(&self.low).unwrap();
            let high = layout.scale.tick(&self.high).unwrap();
            bar(
                Bounds::new(
                    point(px(low), px(y - 3.)),
                    size(px((high - low).max(1.)), px(6.)),
                ),
                muted.opacity(0.25),
                &bounds,
                window,
                cx,
            );
        }
        for item in layout.labels {
            let tick = item.x.as_f32();
            let ink = color::get(
                if item.name == "現" {
                    ColorRole::Focus
                } else {
                    ColorRole::Foreground
                },
                cx,
            );
            line(
                vec![
                    point(item.bounds.center().x.as_f32(), font.as_f32() * 1.35),
                    point(tick, y - 5.),
                ],
                muted.opacity(0.5),
                &bounds,
                window,
            );
            if item.name == "現" {
                bar(
                    Bounds::new(point(px(tick - 1.5), px(y - 6.)), size(px(3.), px(12.))),
                    ink,
                    &bounds,
                    window,
                    cx,
                );
            } else {
                line(
                    vec![point(tick, y - 5.), point(tick, y + 5.)],
                    ink,
                    &bounds,
                    window,
                );
            }
            label(
                item.name,
                item.bounds.origin,
                ink,
                font,
                &bounds,
                window,
                cx,
            );
        }
    }
}
#[cfg(test)]
mod tests {
    #[test]
    fn daily_axis_excludes_price_limits() {
        let mut q = crate::quotes::sample().remove(0);
        let plot = super::PositionPlot::new(&q);
        assert_eq!(
            plot.markers
                .iter()
                .map(|(name, _)| *name)
                .collect::<Vec<_>>(),
            ["低", "昨收", "開", "現", "高"]
        );
        q.is_reference = true;
        q.last = 99999.;
        let reference = super::PositionPlot::new(&q);
        assert!(
            reference
                .markers
                .iter()
                .all(|(name, value)| *name != "現" && *value != q.last)
        );
    }
}
