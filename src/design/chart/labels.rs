use super::*;

#[derive(Clone, Debug, PartialEq)]
pub struct ChartLabel {
    pub text: SharedString,
    pub bounds: Bounds<Pixels>,
}
#[derive(Debug, PartialEq)]
pub struct ChartLabels {
    pub volume: ChartLabel,
    pub price: Vec<ChartLabel>,
    pub date: Option<ChartLabel>,
    pub price_ticks: Vec<ChartLabel>,
    pub volume_ticks: Vec<ChartLabel>,
    pub date_ticks: Vec<ChartLabel>,
}
fn intersects(a: Bounds<Pixels>, b: Bounds<Pixels>) -> bool {
    a.left() < b.right() && b.left() < a.right() && a.top() < b.bottom() && b.top() < a.bottom()
}
impl ChartLabels {
    /// These are the same bounds used for painting, collision suppression and tests.
    pub fn new(
        g: &ChartGeometry,
        data: &ChartSeries,
        current: Option<Price>,
        hover: Option<&TooltipState>,
        period: Timeframe,
        window: &mut Window,
    ) -> Self {
        let font = g.layout.caption;
        let last = data.candles.len() - 1;
        let index = hover.map_or(last, |h| h.index.min(last));
        let candle = &data.candles[index];
        let value: SharedString = exact_volume(candle.volume, data.lot_size).into();
        let width = (px(measure_text_width(&value, font, window)) + font * 2. / 3.)
            .max(g.layout.axis_width - font * 2. / 3.);
        let height = font * 5. / 3.;
        let volume = ChartLabel {
            text: value,
            bounds: Bounds::new(
                point(
                    g.layout.volume.right() + g.layout.axis_width - font / 3. - width,
                    g.volume_bar(index, candle.volume).top() - height / 2.,
                ),
                size(width, height),
            ),
        };
        let mut price = Vec::new();
        if let Some(hover) = hover {
            price.push(ChartLabel {
                text: price_text(g.cursor_price(hover.cross_line.y)).into(),
                bounds: g.price_tag(hover.cross_line.y),
            });
        }
        if let Some(current) = current {
            let frame = g.price_tag(g.price_y(current));
            if !price.iter().any(|l| intersects(l.bounds, frame)) {
                price.push(ChartLabel {
                    text: price_text(current).into(),
                    bounds: frame,
                });
            }
        }
        let date = hover.map(|hover| {
            let text: SharedString = date_text(candle.time, period).into();
            let width = px(measure_text_width(&text, font, window)) + font * 2. / 3.;
            ChartLabel {
                text,
                bounds: Bounds::new(
                    point(
                        (hover.cross_line.x - width / 2.)
                            .clamp(px(0.), (g.layout.price.right() - width).max(px(0.))),
                        g.layout.volume.bottom() + px(3.),
                    ),
                    size(width, height),
                ),
            }
        });
        let mut y_label = |text: String, y: Pixels| {
            let text: SharedString = text.into();
            let width = px(measure_text_width(&text, font, window));
            ChartLabel {
                text,
                bounds: Bounds::new(g.axis_label_bounds(y).origin, size(width, font * 1.25)),
            }
        };
        let mut price_ticks: Vec<_> = g
            .price_ticks
            .values
            .iter()
            .map(|v| {
                y_label(
                    price_tick(*v, g.price_ticks.step),
                    px(g.price.tick(&(*v as f64)).unwrap()),
                )
            })
            .collect();
        let mut volume_ticks: Vec<_> = g
            .volume_ticks
            .values
            .iter()
            .map(|v| {
                y_label(
                    volume_text(*v as f64, data.lot_size),
                    px(g.volume.tick(&(*v as f64)).unwrap()),
                )
            })
            .collect();
        let mut indices = vec![0, last / 4, last / 2, last * 3 / 4, last];
        indices.dedup();
        let mut date_ticks: Vec<_> = indices
            .into_iter()
            .map(|ix| {
                let text: SharedString = date_text(data.candles[ix].time, period).into();
                let width = px(measure_text_width(&text, font, window));
                let x = px(g.candle(ix, &data.candles[ix]).x);
                let left = if ix == 0 {
                    x
                } else if ix == last {
                    x - width
                } else {
                    x - width / 2.
                };
                ChartLabel {
                    text,
                    bounds: Bounds::new(
                        point(left, g.layout.volume.bottom() + px(TEXT_GAP * 3.)),
                        size(width, font * 1.25),
                    ),
                }
            })
            .collect();
        let blocked: Vec<_> = price
            .iter()
            .chain(std::iter::once(&volume))
            .chain(date.iter())
            .map(|l| l.bounds)
            .collect();
        for ticks in [&mut price_ticks, &mut volume_ticks, &mut date_ticks] {
            let mut visible = Vec::new();
            ticks.retain(|t| {
                if blocked
                    .iter()
                    .chain(&visible)
                    .any(|b| intersects(*b, t.bounds))
                {
                    return false;
                }
                visible.push(t.bounds);
                true
            });
        }
        Self {
            volume,
            price,
            date,
            price_ticks,
            volume_ticks,
            date_ticks,
        }
    }
}
