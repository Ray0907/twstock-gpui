use chrono::NaiveDate;
use twstock_gpui::market::*;
fn candle(date: &str, n: i64) -> Candle {
    Candle {
        time: NaiveDate::parse_from_str(date, "%Y-%m-%d").unwrap(),
        open: Price(n * 10000),
        high: Price((n + 3) * 10000),
        low: Price((n - 2) * 10000),
        close: Price((n + 1) * 10000),
        volume: n as u64 * 1000,
    }
}
#[test]
fn aggregation_uses_iso_weeks_months_and_real_observed_days() {
    let rows = vec![
        candle("2020-12-31", 10),
        candle("2021-01-01", 12),
        candle("2021-01-04", 14),
        candle("2021-01-08", 16),
        candle("2021-02-01", 18),
    ];
    let w = aggregate_candles(&rows, Timeframe::Week).unwrap();
    assert_eq!(w.len(), 3);
    assert_eq!(
        w[0],
        Candle {
            time: rows[1].time,
            open: Price(100000),
            high: Price(150000),
            low: Price(80000),
            close: Price(130000),
            volume: 22000
        }
    );
    assert_eq!(w[1].volume, 30000);
    assert_eq!(w[2], rows[4]);
    let m = aggregate_candles(&rows, Timeframe::Month).unwrap();
    assert_eq!(m.len(), 3);
    assert_eq!(m[1].open, rows[1].open);
    assert_eq!(m[1].close, rows[3].close);
    assert_eq!(m[1].volume, 42000);
    assert_eq!(aggregate_candles(&rows, Timeframe::Day).unwrap(), rows);
    assert!(aggregate_candles(&[], Timeframe::Month).unwrap().is_empty());
    let mut unordered = rows.clone();
    unordered.swap(0, 1);
    assert!(aggregate_candles(&unordered, Timeframe::Week).is_err());
    let mut bad = rows[..2].to_vec();
    bad[0].volume = u64::MAX;
    assert!(aggregate_candles(&bad, Timeframe::Week).is_err());
}
#[test]
fn current_day_is_in_unfinished_period_and_ma_is_recomputed() {
    let mut store = MockProvider::new().store();
    let symbol = store.order[0].clone();
    let at = chrono::FixedOffset::east_opt(8 * 3600).unwrap();
    use chrono::TimeZone;
    let time = at
        .with_ymd_and_hms(2026, 10, 15, 11, 0, 0)
        .unwrap()
        .timestamp();
    store.apply(
        MarketEvent::Quote(QuoteUpdate {
            symbol: symbol.clone(),
            as_of: Some(time),
            last: Some(Price(2000000)),
            open: Some(Price(1990000)),
            high: Some(Price(2010000)),
            low: Some(Price(1980000)),
            volume: Some(17000),
            ..Default::default()
        }),
        time,
    );
    let day = store.chart_series(&symbol).unwrap();
    for period in [Timeframe::Week, Timeframe::Month] {
        let grouped = day.clone().for_period(period).unwrap();
        assert_eq!(grouped.candles.last().unwrap().close, Price(2000000));
        assert_eq!(
            grouped.averages,
            [5, 10, 20].map(|n| moving_average(&grouped.candles, n))
        );
        assert!(grouped.candles.len() < day.candles.len());
    }
}
