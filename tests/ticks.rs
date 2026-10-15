use twstock_gpui::market::nice_ticks;
fn check(low: i128, high: i128, intervals: usize) {
    let axis = nice_ticks(low, high, intervals);
    assert!(axis.min <= low.min(high) && axis.max >= low.max(high));
    assert!(axis.max > axis.min && axis.step > 0);
    assert!(axis.values.windows(2).all(|v| v[1] - v[0] == axis.step));
    assert_eq!(axis.values.first(), Some(&axis.min));
    assert_eq!(axis.values.last(), Some(&axis.max));
    let mut power = 1;
    while axis.step >= power * 10 {
        power *= 10;
    }
    assert!([2, 4, 5, 10].contains(&(axis.step * 2 / power)));
}
#[test]
fn nice_price_and_volume_ticks_cover_the_data() {
    // Invented inputs only; never copy live responses into fixtures.
    let price = nice_ticks(927_300, 1_169_900, 5);
    assert_eq!(
        (price.min, price.max, price.step),
        (900_000, 1_200_000, 50_000)
    );
    let volume = nice_ticks(0, 557_321_123, 3);
    assert_eq!(
        (volume.min, volume.max, volume.step),
        (0, 600_000_000, 200_000_000)
    );
}
#[test]
fn flat_tiny_etf_and_large_prices_are_safe() {
    for (low, high) in [
        (0, 0),
        (500_000, 500_000),
        (234_500, 234_500),
        (1, 2),
        (234_400, 234_900),
        (10_000_000, 12_610_000),
        (-15, 15),
        (200, 100),
        (i64::MIN as i128, i64::MAX as i128),
    ] {
        check(low, high, 5);
    }
    assert_eq!(nice_ticks(234_400, 234_900, 5).step, 100);
    assert_eq!(nice_ticks(10_000_000, 12_610_000, 5).step, 1_000_000);
    assert_eq!(nice_ticks(100, 111, 5).step, 5);
    assert_eq!(nice_ticks(1000, 1110, 5).step, 25);
}
