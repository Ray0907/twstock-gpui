use gpui_kit::component::plot::{Plot, scale::Scale};
use gpui_kit::{Bounds, point, px, size};
use twstock_gpui::{
    design::{
        chart::{ChartGeometry, TradingPlot},
        direction::CandleBody,
    },
    market::*,
};

fn data() -> ChartSeries {
    let store = MockProvider::new().store();
    store.chart_series(&store.order[0]).unwrap()
}
#[test]
fn plot_has_synchronized_price_volume_and_shared_price_projection() {
    let data = data();
    assert_eq!(data.candles.len(), 100);
    assert!(data.averages[2][..19].iter().all(Option::is_none));
    for size_px in [(450., 420.), (620., 540.)] {
        let bounds = Bounds::new(point(px(0.), px(0.)), size(px(size_px.0), px(size_px.1)));
        let g = ChartGeometry::new(bounds, px(13.), &data, None);
        assert!((g.layout.price.size.height / g.layout.volume.size.height - 3.5).abs() < 0.001);
        assert_eq!(g.layout.price.size.width, g.layout.volume.size.width);
        assert!(
            g.layout.price.bottom() + g.layout.caption < g.layout.volume.top(),
            "price and volume axis labels must not collide"
        );
        for &p in &g.price_ticks.values {
            let p = g.axis_label_bounds(gpui_kit::px(g.price.tick(&(p as f64)).unwrap()));
            for &v in &g.volume_ticks.values {
                let v = g.axis_label_bounds(gpui_kit::px(g.volume.tick(&(v as f64)).unwrap()));
                assert!(
                    p.bottom() + px(8.) <= v.top(),
                    "price and volume label bounds must have a visible separator"
                );
            }
        }
        for (ix, c) in data.candles.iter().enumerate() {
            let candle = g.candle(ix, c);
            let y = g.price_y(c.close);
            assert!((g.price_y(g.cursor_price(y)) - y).abs() < px(0.02));
            assert!(candle.body.top() >= g.layout.price.top());
            assert!(candle.body.bottom() <= g.layout.price.bottom());
            assert_eq!(
                candle.style,
                if c.close > c.open {
                    CandleBody::Solid
                } else if c.close < c.open {
                    CandleBody::Hollow
                } else {
                    CandleBody::Doji
                }
            );
            assert!((candle.x - (g.x.tick(&ix).unwrap() + g.x.band_width() / 2.)).abs() < 0.001);
        }
    }
}
#[gpui_kit::test]
fn plot_hover_snaps_to_the_same_candle_and_never_interpolates_ohlc(
    cx: &mut gpui_kit::TestAppContext,
) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        twstock_gpui::theme::apply(gpui_kit::component::ThemeMode::Dark, None, cx);
        let data = data();
        let bounds = Bounds::new(point(px(0.), px(0.)), size(px(500.), px(450.)));
        let g = ChartGeometry::new(bounds, px(13.), &data, None);
        let c = &data.candles[40];
        let x = g.candle(40, c).x;
        let y = g.price_y(c.close);
        let plot = TradingPlot::new(data, None);
        let tiny = Bounds::new(point(px(0.), px(0.)), size(px(50.), px(30.)));
        assert!(
            plot.tooltip_state(point(px(0.5), px(27.8)), tiny, cx)
                .is_none(),
            "collapsed dock must not expose invalid plot coordinates"
        );
        let hover = plot.tooltip_state(point(px(x), y), bounds, cx).unwrap();
        assert_eq!(hover.index, 40);
        assert!((hover.cross_line.x.as_f32() - x).abs() < 0.001);
        assert!((hover.cross_line.y.as_f32() - y.as_f32()).abs() < 0.001);
    });
}
#[test]
fn fixed_point_ma_has_correct_values_and_empty_prefix() {
    let mut candles = data().candles;
    candles.truncate(7);
    for (ix, c) in candles.iter_mut().enumerate() {
        c.close = Price((ix + 1) as i64 * 10000);
    }
    assert_eq!(
        moving_average(&candles, 5),
        vec![
            None,
            None,
            None,
            None,
            Some(Price(30000)),
            Some(Price(40000)),
            Some(Price(50000))
        ]
    );
    assert!(moving_average(&candles, 10).iter().all(Option::is_none));
    assert!(moving_average(&[], 5).is_empty());
}
#[test]
fn main_chart_does_not_use_kit_candlestick_or_overlay_canvas() {
    let source = include_str!("../src/design/parts.rs");
    assert!(
        !source.contains("CandlestickChart"),
        "Phase 2 must replace CandlestickChart"
    );
    assert!(
        !source.contains("fn current_price_marker"),
        "no div/canvas plot overlays"
    );
}
