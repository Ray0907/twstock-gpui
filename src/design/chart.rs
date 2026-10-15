//! One plot owns both subplots, all projections and native hover tracking.
use super::{
    color::{self, ColorRole},
    direction::{CandleBody, Direction},
    space::{Dimension, Space},
    text::{self, TextRole},
};
use crate::{
    market::{Candle, ChartSeries, NiceTicks, Price, Timeframe, nice_ticks, volume_clip_limit},
    quotes::format_price,
};
use gpui_kit::component::{
    ActiveTheme,
    plot::{
        AxisText, Curve, Grid, Plot, PlotAxis, PlotElement, PlotHover, TooltipState, axis_gutter,
        label::{PlotLabel, TEXT_GAP, TEXT_SIZE, Text, measure_text_width, truncate_text_to_width},
        scale::{Scale, ScaleBand, ScaleLinear},
        shape::{Bar, Line},
    },
};
use gpui_kit::*;

pub struct ChartLayout {
    pub price: Bounds<Pixels>,
    pub volume: Bounds<Pixels>,
    pub caption: Pixels,
    axis_width: Pixels,
}
impl ChartLayout {
    pub fn new(bounds: Bounds<Pixels>, font: Pixels) -> Self {
        let caption = text::pixels(TextRole::Caption, font);
        let axis_width = Dimension::Axis.rem().to_pixels(font);
        let header = caption * 2.5 + Space::Tight.rem().to_pixels(font);
        let footer = px(axis_gutter(caption)) + Space::Micro.rem().to_pixels(font);
        let gap = Space::Large.rem().to_pixels(font) * 2.;
        let width = (bounds.size.width - Dimension::Axis.rem().to_pixels(font)).max(px(1.));
        let available = (bounds.size.height - header - footer - gap).max(px(1.));
        let price = Bounds::new(point(px(0.), header), size(width, available * (7. / 9.)));
        let volume = Bounds::new(
            point(px(0.), price.bottom() + gap),
            size(width, available * (2. / 9.)),
        );
        Self {
            price,
            volume,
            caption,
            axis_width,
        }
    }
}
pub struct CandleGeometry {
    pub x: f32,
    pub body: Bounds<Pixels>,
    pub high: Pixels,
    pub low: Pixels,
    pub style: CandleBody,
}
pub struct ChartGeometry {
    pub layout: ChartLayout,
    pub x: ScaleBand<usize>,
    pub price: ScaleLinear<f64>,
    pub volume: ScaleLinear<f64>,
    pub domain: [f64; 2],
    pub price_ticks: NiceTicks,
    pub volume_ticks: NiceTicks,
    pub volume_clip: Option<u64>,
}
impl ChartGeometry {
    pub fn new(
        bounds: Bounds<Pixels>,
        font: Pixels,
        data: &ChartSeries,
        current: Option<Price>,
    ) -> Self {
        let layout = ChartLayout::new(bounds, font);
        let low = data
            .candles
            .iter()
            .map(|c| c.low.0)
            .chain(current.map(|p| p.0))
            .min()
            .unwrap_or(0) as i128;
        let high = data
            .candles
            .iter()
            .map(|c| c.high.0)
            .chain(current.map(|p| p.0))
            .max()
            .unwrap_or(10000) as i128;
        let price_ticks = nice_ticks(low, high, 5);
        let domain = [price_ticks.min as f64, price_ticks.max as f64];
        let max_volume = data
            .candles
            .iter()
            .map(|c| c.volume)
            .max()
            .unwrap_or(1)
            .max(1) as i128;
        let volumes: Vec<_> = data.candles.iter().map(|c| c.volume).collect();
        let volume_clip = volume_clip_limit(&volumes);
        let volume_ticks = nice_ticks(0, volume_clip.map_or(max_volume, i128::from), 3);
        let price = ScaleLinear::new(
            domain,
            [layout.price.bottom().as_f32(), layout.price.top().as_f32()],
        );
        let volume = ScaleLinear::new(
            [volume_ticks.min as f64, volume_ticks.max as f64],
            [
                layout.volume.bottom().as_f32(),
                layout.volume.top().as_f32(),
            ],
        );
        let x = ScaleBand::new(
            0..data.candles.len(),
            [0., layout.price.size.width.as_f32()],
        )
        .padding_inner(0.15)
        .padding_outer(0.5);
        Self {
            layout,
            x,
            price,
            volume,
            domain,
            price_ticks,
            volume_ticks,
            volume_clip,
        }
    }
    pub fn clipped_volume(&self, volume: u64) -> Option<u64> {
        self.volume_clip
            .filter(|limit| volume > *limit)
            .map(|_| volume)
    }
    pub fn volume_bar(&self, index: usize, volume: u64) -> Bounds<Pixels> {
        let volume = self.volume_clip.map_or(volume, |limit| volume.min(limit));
        let y = self.volume.tick(&(volume as f64)).unwrap();
        let x = self.x.tick(&index).unwrap_or(0.) + self.x.band_width() / 2.;
        let width = self.x.band_width() * 0.85;
        Bounds::new(
            point(px(x - width / 2.), px(y)),
            size(px(width), self.layout.volume.bottom() - px(y)),
        )
    }
    pub fn axis_label_bounds(&self, y: Pixels) -> Bounds<Pixels> {
        Bounds::new(
            point(
                self.layout.price.right() + px(TEXT_GAP),
                y - px(TEXT_SIZE / 2.),
            ),
            size(
                self.layout.axis_width - px(TEXT_GAP),
                self.layout.caption * 1.25,
            ),
        )
    }
    pub fn price_y(&self, price: Price) -> Pixels {
        px(self
            .price
            .tick(&(price.0 as f64))
            .unwrap_or(self.layout.price.bottom().as_f32()))
    }
    pub fn cursor_price(&self, y: Pixels) -> Price {
        let inverse = ScaleLinear::new(
            [
                self.layout.price.top().as_f32() as f64,
                self.layout.price.bottom().as_f32() as f64,
            ],
            [self.domain[1] as f32, self.domain[0] as f32],
        );
        Price(inverse.tick(&(y.as_f32() as f64)).unwrap_or(0.).round() as i64)
    }
    pub fn candle(&self, index: usize, candle: &Candle) -> CandleGeometry {
        let x = self.x.tick(&index).unwrap_or(0.) + self.x.band_width() / 2.;
        let width = self.x.band_width() * 0.85;
        let top = self.price_y(candle.open).min(self.price_y(candle.close));
        let bottom = self.price_y(candle.open).max(self.price_y(candle.close));
        CandleGeometry {
            x,
            body: Bounds::new(
                point(px(x - width / 2.), top),
                size(px(width), (bottom - top).max(px(0.8))),
            ),
            high: self.price_y(candle.high),
            low: self.price_y(candle.low),
            style: Direction::of((candle.close.0 - candle.open.0) as f64).candle_body(),
        }
    }
    pub fn price_tag(&self, y: Pixels) -> Bounds<Pixels> {
        let pad = self.layout.caption / 3.;
        let height = self.layout.caption + pad * 2.;
        Bounds::new(
            point(
                self.layout.price.right() + pad,
                (y - height / 2.).clamp(
                    self.layout.price.top(),
                    (self.layout.price.bottom() - height).max(self.layout.price.top()),
                ),
            ),
            size(self.layout.axis_width - pad * 2., height),
        )
    }
}
pub struct TradingPlot {
    data: ChartSeries,
    current: Option<Price>,
    hover: Option<TooltipState>,
    period: Timeframe,
}
impl TradingPlot {
    pub fn new(data: ChartSeries, current: Option<Price>) -> Self {
        Self {
            data,
            current,
            hover: None,
            period: Timeframe::Day,
        }
    }
}
pub fn date_text(date: chrono::NaiveDate, period: Timeframe) -> String {
    date.format(if period == Timeframe::Month {
        "%Y/%m"
    } else {
        "%m/%d"
    })
    .to_string()
}
impl IntoElement for TradingPlot {
    type Element = PlotElement<Self>;
    fn into_element(self) -> Self::Element {
        PlotElement::new(self)
    }
}
fn price_text(price: Price) -> String {
    format_price(price.0 as f64 / 10000.)
}
fn price_tick(value: i128, step: i128) -> String {
    let precision = (0..=4).find(|p| step % 10i128.pow(4 - p) == 0).unwrap();
    let value = format!("{:.*}", precision as usize, value as f64 / 10000.);
    if precision > 0 {
        value.trim_end_matches('0').trim_end_matches('.').into()
    } else {
        value
    }
}
fn volume_text(shares: f64, lot: u64) -> String {
    let lots = shares / lot.max(1) as f64;
    let (value, suffix) = if lots >= 1_000_000. {
        (lots / 1_000_000., "M")
    } else if lots >= 1000. {
        (lots / 1000., "k")
    } else {
        (lots, "")
    };
    let precision = if suffix.is_empty() { 3 } else { 1 };
    let number = format!("{value:.precision$}");
    format!(
        "{}{suffix}",
        number.trim_end_matches('0').trim_end_matches('.')
    )
}
fn exact_volume(shares: u64, lot: u64) -> String {
    let lot = lot.max(1);
    if shares.is_multiple_of(lot) {
        (shares / lot).to_string()
    } else {
        format!("{:.3}", shares as f64 / lot as f64)
            .trim_end_matches('0')
            .to_string()
    }
}
pub(super) fn line(
    points: Vec<Point<f32>>,
    stroke: Hsla,
    bounds: &Bounds<Pixels>,
    window: &mut Window,
) {
    Line::new()
        .data(points)
        .x(|p: &Point<f32>| Some(p.x))
        .y(|p| Some(p.y))
        .curve(Curve::Linear)
        .stroke(stroke)
        .stroke_width(px(0.8))
        .paint(bounds, window);
}
pub(super) fn bar(
    frame: Bounds<Pixels>,
    fill: Hsla,
    bounds: &Bounds<Pixels>,
    window: &mut Window,
    cx: &mut App,
) {
    Bar::new()
        .data(vec![frame])
        .cross(|b: &Bounds<Pixels>| Some(b.left().as_f32()))
        .base(|b| b.bottom().as_f32())
        .value(|b| Some(b.top().as_f32()))
        .band_width(frame.size.width.as_f32())
        .fill(move |_, _, _| fill)
        .paint(bounds, window, cx);
}
pub(super) fn label(
    value: impl Into<SharedString>,
    position: Point<Pixels>,
    color: Hsla,
    font: Pixels,
    bounds: &Bounds<Pixels>,
    window: &mut Window,
    cx: &mut App,
) {
    PlotLabel::new(vec![Text::new(value, position, color).font_size(font)])
        .paint(bounds, window, cx);
}
fn tag(
    value: SharedString,
    frame: Bounds<Pixels>,
    font: Pixels,
    bounds: &Bounds<Pixels>,
    window: &mut Window,
    cx: &mut App,
) {
    bar(
        frame,
        color::get(ColorRole::FocusFill, cx),
        bounds,
        window,
        cx,
    );
    label(
        value,
        frame.origin + point(font / 3., font / 3.),
        color::get(ColorRole::FocusInk, cx),
        font,
        bounds,
        window,
        cx,
    );
}
fn has_space(bounds: Bounds<Pixels>, font: Pixels) -> bool {
    bounds.size.width > Dimension::Axis.rem().to_pixels(font) + font * 2.
        && bounds.size.height > font * 8.
}
impl Plot for TradingPlot {
    fn id(&self) -> Option<ElementId> {
        Some("daily-market-plot".into())
    }
    fn tooltip_state(
        &self,
        position: Point<Pixels>,
        bounds: Bounds<Pixels>,
        cx: &App,
    ) -> Option<TooltipState> {
        if self.data.candles.is_empty() || !has_space(bounds, cx.theme().font_size) {
            return None;
        }
        let g = ChartGeometry::new(bounds, cx.theme().font_size, &self.data, self.current);
        if !g.layout.price.contains(&position) && !g.layout.volume.contains(&position) {
            return None;
        }
        let index =
            g.x.nearest_index(position.x.as_f32() - g.x.band_width() / 2.)
                .min(self.data.candles.len() - 1);
        let candle = &self.data.candles[index];
        let x = g.candle(index, candle).x;
        let y = if g.layout.price.contains(&position) {
            position.y
        } else {
            g.price_y(candle.close)
        };
        Some(TooltipState::new(index, point(px(x), y), vec![]))
    }
    fn hover(&mut self, hover: Option<&PlotHover>, _: &mut Window, _: &mut App) {
        self.hover = hover.filter(|h| h.is_hovered()).map(|h| h.state().clone());
    }
    fn paint(&mut self, bounds: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
        if self.data.candles.is_empty() {
            return;
        }
        if !has_space(bounds, cx.theme().font_size) {
            let font = text::pixels(TextRole::Caption, cx.theme().font_size);
            let hint = truncate_text_to_width(
                &"圖表空間不足".into(),
                font,
                bounds.size.width.as_f32(),
                window,
            );
            label(
                hint,
                point(px(0.), px(0.)),
                color::get(ColorRole::Muted, cx),
                font,
                &bounds,
                window,
                cx,
            );
            return;
        }
        let g = ChartGeometry::new(bounds, cx.theme().font_size, &self.data, self.current);
        let font = g.layout.caption;
        let foreground = color::get(ColorRole::Foreground, cx);
        let muted = color::get(ColorRole::Muted, cx);
        let price_bounds = Bounds::new(bounds.origin + g.layout.price.origin, g.layout.price.size);
        let volume_bounds =
            Bounds::new(bounds.origin + g.layout.volume.origin, g.layout.volume.size);
        let ticks = &g.price_ticks.values;
        Grid::new()
            .y(ticks
                .iter()
                .map(|v| px(g.price.tick(&(*v as f64)).unwrap()) - g.layout.price.top()))
            .stroke(color::get(ColorRole::Border, cx))
            .dash_array(&[px(3.), px(3.)])
            .paint(&price_bounds, window);
        if let Some(current) = self.current {
            Grid::new()
                .y([g.price_y(current) - g.layout.price.top()])
                .stroke(color::get(ColorRole::LastPriceLine, cx))
                .dash_array(&[px(4.), px(3.)])
                .paint(&price_bounds, window);
        }
        for (index, candle) in self.data.candles.iter().enumerate() {
            let c = g.candle(index, candle);
            let direction = Direction::of((candle.close.0 - candle.open.0) as f64);
            let stroke = direction.color(cx);
            // Hollow bodies must not have a wick running through their interior.
            line(
                vec![
                    point(c.x, c.high.as_f32()),
                    point(c.x, c.body.top().as_f32()),
                ],
                stroke,
                &bounds,
                window,
            );
            line(
                vec![
                    point(c.x, c.body.bottom().as_f32()),
                    point(c.x, c.low.as_f32()),
                ],
                stroke,
                &bounds,
                window,
            );
            match c.style {
                CandleBody::Solid => bar(c.body, stroke, &bounds, window, cx),
                CandleBody::Hollow => line(
                    vec![
                        point(c.body.left().as_f32(), c.body.top().as_f32()),
                        point(c.body.right().as_f32(), c.body.top().as_f32()),
                        point(c.body.right().as_f32(), c.body.bottom().as_f32()),
                        point(c.body.left().as_f32(), c.body.bottom().as_f32()),
                        point(c.body.left().as_f32(), c.body.top().as_f32()),
                    ],
                    stroke,
                    &bounds,
                    window,
                ),
                CandleBody::Doji => line(
                    vec![
                        point(c.body.left().as_f32(), c.body.top().as_f32()),
                        point(c.body.right().as_f32(), c.body.top().as_f32()),
                    ],
                    stroke,
                    &bounds,
                    window,
                ),
            }
            let frame = g.volume_bar(index, candle.volume);
            bar(frame, stroke.opacity(0.55), &bounds, window, cx);
            if let Some(actual) = g.clipped_volume(candle.volume) {
                // Contrasting slashes explicitly break the capped bar's top.
                for x in [c.x - 3., c.x + 1.] {
                    Line::new()
                        .data(vec![
                            point(x - 1., frame.top().as_f32() + 5.),
                            point(x + 2., frame.top().as_f32() - 1.),
                        ])
                        .x(|p: &Point<f32>| Some(p.x))
                        .y(|p| Some(p.y))
                        .stroke(foreground)
                        .stroke_width(px(2.))
                        .paint(&bounds, window);
                }
                let value: SharedString = volume_text(actual as f64, self.data.lot_size).into();
                let width = px(measure_text_width(&value, font, window));
                label(
                    value,
                    point(
                        (frame.right() + px(3.)).min((g.layout.volume.right() - width).max(px(0.))),
                        frame.top() + px(3.),
                    ),
                    foreground,
                    font,
                    &bounds,
                    window,
                    cx,
                );
            }
        }
        for (index, average) in self.data.averages.iter().enumerate() {
            let points = average
                .iter()
                .enumerate()
                .filter_map(|(ix, value)| {
                    value.map(|v| {
                        point(
                            g.candle(ix, &self.data.candles[ix]).x,
                            g.price_y(v).as_f32(),
                        )
                    })
                })
                .collect::<Vec<_>>();
            Line::new()
                .data(points)
                .x(|p: &Point<f32>| Some(p.x))
                .y(|p| Some(p.y))
                .curve(Curve::Linear)
                .stroke(rgb(color::series_hex(index, cx.theme().mode.is_dark())))
                .stroke_width(px(1.25))
                .paint(&bounds, window);
        }
        PlotAxis::new()
            .y(g.layout.price.right())
            .y_label(ticks.iter().map(|v| {
                AxisText::new(
                    price_tick(*v, g.price_ticks.step),
                    px(g.price.tick(&(*v as f64)).unwrap()),
                    foreground,
                )
                .font_size(font)
            }))
            .paint(&bounds, window, cx);
        PlotAxis::new()
            .y(g.layout.volume.right())
            .y_label(g.volume_ticks.values.iter().map(|v| {
                AxisText::new(
                    volume_text(*v as f64, self.data.lot_size),
                    px(g.volume.tick(&(*v as f64)).unwrap()),
                    foreground,
                )
                .font_size(font)
            }))
            .paint(&bounds, window, cx);
        let last = self.data.candles.len() - 1;
        PlotAxis::new()
            .x(g.layout.volume.bottom())
            .x_axis(false)
            .x_label([0, last / 4, last / 2, last * 3 / 4, last].map(|ix| {
                let x = g.candle(ix, &self.data.candles[ix]).x;
                AxisText::new(
                    date_text(self.data.candles[ix].time, self.period),
                    px(x),
                    muted,
                )
                .font_size(font)
                .align(if ix == 0 {
                    TextAlign::Left
                } else if ix == last {
                    TextAlign::Right
                } else {
                    TextAlign::Center
                })
            }))
            .paint(&bounds, window, cx);
        Grid::new()
            .y([g.layout.volume.size.height])
            .stroke(color::get(ColorRole::Border, cx))
            .paint(&volume_bounds, window);
        let highest = self
            .data
            .candles
            .iter()
            .enumerate()
            .max_by_key(|(_, c)| c.high)
            .unwrap();
        let lowest = self
            .data
            .candles
            .iter()
            .enumerate()
            .min_by_key(|(_, c)| c.low)
            .unwrap();
        for ((ix, c), high) in [(highest, true), (lowest, false)] {
            let x = g.candle(ix, c).x;
            let value = if high { c.high } else { c.low };
            let y = g.price_y(value);
            let text: SharedString =
                format!("{} {}", if high { "H" } else { "L" }, price_text(value)).into();
            let width = measure_text_width(&text, font, window);
            let left = (px(x) + px(5.))
                .min(g.layout.price.right() - px(width))
                .max(px(0.));
            let top = if high { y - font - px(4.) } else { y + px(4.) };
            label(
                text,
                point(
                    left,
                    top.clamp(g.layout.price.top(), g.layout.price.bottom() - font),
                ),
                foreground,
                font,
                &bounds,
                window,
                cx,
            );
        }
        let index = self.hover.as_ref().map_or(last, |h| h.index.min(last));
        let c = &self.data.candles[index];
        let info: SharedString = if self.hover.is_some() {
            format!(
                "{}  開 {}  高 {}  低 {}  收 {}  量 {} 張",
                date_text(c.time, self.period),
                price_text(c.open),
                price_text(c.high),
                price_text(c.low),
                price_text(c.close),
                exact_volume(c.volume, self.data.lot_size)
            )
            .into()
        } else {
            format!(
                "{}線 · 價格／成交量 · {} 根",
                match self.period {
                    Timeframe::Day => "日",
                    Timeframe::Week => "週",
                    Timeframe::Month => "月",
                },
                self.data.candles.len()
            )
            .into()
        };
        let info = truncate_text_to_width(&info, font, bounds.size.width.as_f32(), window);
        label(
            info,
            point(px(0.), px(0.)),
            if self.hover.is_some() {
                foreground
            } else {
                muted
            },
            font,
            &bounds,
            window,
            cx,
        );
        let mut x = px(0.);
        for (series, period) in [5, 10, 20].into_iter().enumerate() {
            let value = self.data.averages[series][index].map_or_else(|| "—".into(), price_text);
            let text: SharedString = format!("MA{period} {value}").into();
            let width = measure_text_width(&text, font, window);
            label(
                text,
                point(x, font * 1.25),
                rgb(color::series_hex(series, cx.theme().mode.is_dark())).into(),
                font,
                &bounds,
                window,
                cx,
            );
            x += px(width) + Space::Medium.rem().to_pixels(cx.theme().font_size);
        }
        label(
            volume_title(g.volume_clip, self.data.lot_size),
            point(px(0.), g.layout.volume.top() - font - px(1.)),
            muted,
            font,
            &bounds,
            window,
            cx,
        );
        if let Some(current) = self.current {
            tag(
                price_text(current).into(),
                g.price_tag(g.price_y(current)),
                font,
                &bounds,
                window,
                cx,
            );
        }
        if let Some(hover) = &self.hover {
            let focus = color::get(ColorRole::Focus, cx);
            let cross = Bounds::new(
                price_bounds.origin,
                size(
                    price_bounds.size.width,
                    g.layout.volume.bottom() - g.layout.price.top(),
                ),
            );
            Grid::new()
                .x([hover.cross_line.x])
                .stroke(focus)
                .paint(&cross, window);
            Grid::new()
                .y([hover.cross_line.y - g.layout.price.top()])
                .stroke(focus)
                .paint(&price_bounds, window);
            tag(
                price_text(g.cursor_price(hover.cross_line.y)).into(),
                g.price_tag(hover.cross_line.y),
                font,
                &bounds,
                window,
                cx,
            );
            let date: SharedString = date_text(c.time, self.period).into();
            let width = px(measure_text_width(&date, font, window)) + font * 2. / 3.;
            let frame = Bounds::new(
                point(
                    (hover.cross_line.x - width / 2.).clamp(px(0.), g.layout.price.right() - width),
                    g.layout.volume.bottom() + px(3.),
                ),
                size(width, font * 5. / 3.),
            );
            tag(date, frame, font, &bounds, window, cx);
        }
    }
}
pub fn volume_title(limit: Option<u64>, lot: u64) -> String {
    limit.map_or_else(
        || "量（張）".into(),
        |v| format!("量（張） · 截斷於 {}", exact_volume(v, lot)),
    )
}
pub fn view(data: ChartSeries, current: Option<Price>, period: Timeframe, cx: &App) -> Div {
    let clip = volume_clip_limit(&data.candles.iter().map(|c| c.volume).collect::<Vec<_>>());
    let clip_title = clip.map_or(String::new(), |v| {
        format!(" · {}", volume_title(Some(v), data.lot_size))
    });
    gpui_kit::component::v_flex()
        .flex_1()
        .min_h_0()
        .px(Space::Small.rem())
        .py(Space::Tight.rem())
        .child(
            text::frame(TextRole::Caption, cx)
                .id("candle-region")
                .aria_label(format!(
                    "{}線 · {} 根{}",
                    match period {
                        Timeframe::Day => "日",
                        Timeframe::Week => "週",
                        Timeframe::Month => "月",
                    },
                    data.candles.len(),
                    clip_title
                ))
                .test_support()
                .flex_1()
                .min_w_0()
                .h_full()
                .child(TradingPlot {
                    period,
                    ..TradingPlot::new(data, current)
                }),
        )
}

#[cfg(test)]
mod tests {
    #[test]
    fn volume_abbreviations_match_axis_and_clipped_labels() {
        for (lots, expected) in [
            (999, "999"),
            (1000, "1k"),
            (1500, "1.5k"),
            (600000, "600k"),
            (1200000, "1.2M"),
        ] {
            assert_eq!(super::volume_text((lots * 1000) as f64, 1000), expected);
        }
        let source = include_str!("chart.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();
        let clipped = source
            .split("if let Some(actual) = g.clipped_volume(candle.volume) {")
            .nth(1)
            .unwrap()
            .split("for (index, average)")
            .next()
            .unwrap();
        assert!(
            clipped.contains("volume_text(actual as f64, self.data.lot_size)"),
            "clipped labels must use the existing axis formatter, not a copy or the exact hover formatter"
        );
        assert!(source.contains("volume_text(*v as f64, self.data.lot_size)"));
    }
    #[test]
    fn axes_format_nice_prices_and_scaled_quantities() {
        assert_eq!(super::price_tick(1_000_000, 50_000), "100");
        assert_eq!(super::price_tick(234_500, 100), "23.45");
        assert_eq!(super::price_tick(13, 1), "0.0013");
        assert_eq!(super::volume_text(600_000_000., 1000), "600k");
        assert_eq!(super::volume_text(2_500_000_000., 1000), "2.5M");
        assert_eq!(super::volume_text(20., 1000), "0.02");
    }
}
