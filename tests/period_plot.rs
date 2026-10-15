use gpui_kit::component::plot::scale::Scale;
use gpui_kit::{Bounds, point, px, size};
use twstock_gpui::{
    design::chart::{ChartGeometry, date_text},
    market::*,
};
#[test]
fn volume_clipping_also_applies_to_weekly_and_monthly_bars() {
    let rows: Vec<_> = (0..24)
        .map(|i| Candle {
            time: chrono::NaiveDate::from_ymd_opt(2024 + i / 12, (i % 12 + 1) as u32, 10).unwrap(),
            open: Price(100000),
            high: Price(120000),
            low: Price(80000),
            close: Price(100000),
            volume: if i == 10 { 600000 } else { 1000 },
        })
        .collect();
    let day = ChartSeries {
        averages: [5, 10, 20].map(|p| moving_average(&rows, p)),
        candles: rows,
        lot_size: 1000,
    };
    for period in [Timeframe::Day, Timeframe::Week, Timeframe::Month] {
        let data = day.clone().for_period(period).unwrap();
        let g = ChartGeometry::new(
            Bounds::new(point(px(0.), px(0.)), size(px(640.), px(450.))),
            px(13.),
            &data,
            None,
        );
        assert!(g.volume_clip.is_some());
        assert_eq!(
            g.volume_bar(10, data.candles[10].volume).top(),
            g.layout.volume.top()
        );
    }
}
#[test]
fn period_axes_and_projection_use_the_aggregated_series() {
    let store = MockProvider::new().store();
    let symbol = &store.order[0];
    let day = store.chart_series(symbol).unwrap();
    for period in [Timeframe::Day, Timeframe::Week, Timeframe::Month] {
        let data = day.clone().for_period(period).unwrap();
        let current = store.snapshot(symbol).unwrap().last;
        let g = ChartGeometry::new(
            Bounds::new(point(px(0.), px(0.)), size(px(640.), px(450.))),
            px(13.),
            &data,
            current,
        );
        let max = data.candles.iter().map(|c| c.volume).max().unwrap();
        assert!(g.volume_ticks.max >= i128::from(g.volume_clip.unwrap_or(max)));
        for (i, c) in data.candles.iter().enumerate() {
            let bar = g.volume_bar(i, c.volume);
            assert!(bar.top() >= g.layout.volume.top() - px(0.001));
            assert!(bar.bottom() <= g.layout.volume.bottom() + px(0.001));
        }
        assert!(g.price_ticks.min <= data.candles.iter().map(|c| c.low.0).min().unwrap() as i128);
        assert!(g.price_ticks.max >= data.candles.iter().map(|c| c.high.0).max().unwrap() as i128);
        assert!(g.price_y(current.unwrap()) >= g.layout.price.top());
        for (i, c) in data.candles.iter().enumerate() {
            let x = g.candle(i, c).x;
            assert_eq!(g.x.nearest_index(x - g.x.band_width() / 2.), i);
            assert!((g.cursor_price(g.price_y(c.close)).0 - c.close.0).abs() < 2);
        }
        let label = date_text(data.candles.last().unwrap().time, period);
        assert_eq!(label.len(), if period == Timeframe::Month { 7 } else { 5 });
    }
}
