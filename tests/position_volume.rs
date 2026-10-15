use chrono::NaiveDate;
use twstock_gpui::market::*;

fn candles(volumes: &[u64]) -> Vec<Candle> {
    volumes
        .iter()
        .enumerate()
        .map(|(i, &volume)| Candle {
            time: NaiveDate::from_ymd_opt(2026, 6, i as u32 + 1).unwrap(),
            open: Price(100000),
            high: Price(120000),
            low: Price(80000),
            close: Price(100000),
            volume,
        })
        .collect()
}
#[test]
fn range_handles_flat_outside_and_missing_without_fabrication() {
    assert_eq!(
        range_percentile(Price(10), Price(10), Some(Price(10))),
        Some(50.)
    );
    assert_eq!(
        range_percentile(Price(10), Price(10), Some(Price(11))),
        Some(100.)
    );
    assert_eq!(
        range_percentile(Price(10), Price(20), Some(Price(5))),
        Some(0.)
    );
    assert_eq!(
        range_percentile(Price(10), Price(20), Some(Price(30))),
        Some(100.)
    );
    assert_eq!(range_percentile(Price(10), Price(20), None), None);
    assert_eq!(
        range_percentile(Price(20), Price(10), Some(Price(15))),
        None
    );
    let mut q = Snapshot::default();
    q.last = Some(Price(100000));
    let p = position_summary(&q, &candles(&[100, 200]));
    assert_eq!(p.recent_count, 2);
    assert_eq!(p.recent_low, Some(Price(80000)));
    assert_eq!(p.recent_high, Some(Price(120000)));
    assert_eq!(p.recent_percentile, Some(50.));
    let p = position_summary(&Snapshot::default(), &[]);
    assert_eq!(p.recent_count, 0);
    assert_eq!(p.recent_percentile, None);
    assert_eq!(p.volume_ratio, None);
}
#[test]
fn volume_ratio_uses_five_completed_days_not_todays_partial_candle() {
    let history = candles(&[100, 100, 100, 100, 100, 1000]);
    let mut q = Snapshot::default();
    q.as_of = Some(
        chrono::DateTime::parse_from_rfc3339("2026-06-06T10:00:00+08:00")
            .unwrap()
            .timestamp(),
    );
    q.volume = Some(50);
    q.session = Some(SessionState::Open);
    let p = position_summary(&q, &history);
    assert_eq!(p.volume_ratio, Some(0.5));
    assert!(p.intraday);
    q.session = Some(SessionState::PostClose);
    assert!(!position_summary(&q, &history).intraday);
    assert_eq!(position_summary(&q, &history[..4]).volume_ratio, None);
    q.volume = Some(0);
    assert_eq!(position_summary(&q, &history).volume_ratio, Some(0.));
    assert_eq!(position_summary(&q, &candles(&[0; 6])).volume_ratio, None);
    q.volume = None;
    assert_eq!(position_summary(&q, &history).volume_ratio, None);
    q.volume = Some(50);
    q.as_of = None;
    assert_eq!(position_summary(&q, &history).volume_ratio, None);
}
#[test]
fn p95_is_nearest_rank_including_zeros_and_small_samples() {
    assert_eq!(volume_p95(&[]), None);
    assert_eq!(volume_p95(&[42]), Some(42));
    assert_eq!(volume_p95(&[0, 0, 100]), Some(100));
    assert_eq!(volume_p95(&[0; 20]), Some(0));
    let mut v = vec![100; 19];
    v.push(10000);
    assert_eq!(volume_p95(&v), Some(100));
    assert_eq!(volume_p95(&v[..19]), Some(100));
    assert_eq!(volume_p95(&(1..=100).rev().collect::<Vec<_>>()), Some(95));
    assert_eq!(volume_p95(&(1..=19).rev().collect::<Vec<_>>()), Some(19));
}
#[test]
fn robust_ceiling_only_activates_strictly_above_two_point_five() {
    let cap = nice_ticks(0, 120, 3).max as u64;
    for (max, clipped) in [
        (cap * 5 / 2 - 1, false),
        (cap * 5 / 2, false),
        (cap * 5 / 2 + 1, true),
    ] {
        let mut v = vec![100; 19];
        v.push(max);
        assert_eq!(volume_clip_limit(&v), clipped.then_some(cap));
    }
    for v in [
        vec![],
        vec![100000],
        vec![100; 100],
        vec![0; 100],
        vec![0, 100000],
        vec![100; 19],
    ] {
        assert_eq!(volume_clip_limit(&v), None);
    }
    let mut v = vec![0; 19];
    v.push(100000);
    assert_eq!(volume_clip_limit(&v), Some(nice_ticks(0, 0, 3).max as u64));
}
#[test]
fn clipped_geometry_caps_pixels_and_only_annotates_clipped_bars() {
    use gpui_kit::{Bounds, point, px, size};
    use twstock_gpui::design::chart::{ChartGeometry, volume_title};
    let mut rows = candles(&[100; 20]);
    rows[10].volume = 10000;
    let series = ChartSeries {
        averages: [5, 10, 20].map(|p| moving_average(&rows, p)),
        candles: rows.clone(),
        lot_size: 1,
    };
    let bounds = Bounds::new(point(px(0.), px(0.)), size(px(600.), px(440.)));
    let g = ChartGeometry::new(bounds, px(14.), &series, None);
    let limit = g.volume_clip.unwrap();
    assert_eq!(
        g.volume_bar(10, rows[10].volume).top(),
        g.layout.volume.top()
    );
    assert!(
        (g.volume_bar(10, rows[10].volume).size.height - g.layout.volume.size.height).abs()
            < px(0.001)
    );
    assert!((g.volume_bar(10, limit).size.height - g.layout.volume.size.height).abs() < px(0.001));
    assert_eq!(g.clipped_volume(rows[10].volume), Some(10000));
    assert_eq!(g.clipped_volume(100), None);
    assert_eq!(g.clipped_volume(limit), None);
    assert!(volume_title(g.volume_clip, 1).contains("截斷於"));
    assert!(!volume_title(None, 1).contains("截斷於"));
    let ordinary = ChartSeries {
        candles: candles(&[100; 20]),
        ..series
    };
    let g = ChartGeometry::new(bounds, px(14.), &ordinary, None);
    assert_eq!(g.volume_clip, None);
    assert_eq!(g.clipped_volume(100), None);
}
#[test]
fn watchlist_uses_only_native_dock_menu() {
    let parts = include_str!("../src/design/parts.rs");
    let panel = include_str!("../src/panels/watchlist.rs");
    assert!(
        !parts.contains("Button::new(\"watchlist-actions\")"),
        "a second button duplicates the dock menu"
    );
    assert!(panel.contains("fn dropdown_menu("));
    assert!(parts.contains("移除選取（Delete / Backspace）"));
}
