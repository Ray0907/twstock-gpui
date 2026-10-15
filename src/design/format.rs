/// Actual quantities in lots, not abbreviated axis ticks. Never changes source data.
pub fn volume(lots: f64) -> String {
    if !lots.is_finite() {
        return "—".into();
    }
    let digits = format!("{:.0}", lots.max(0.).round());
    let mut result = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            result.push(',');
        }
        result.push(digit);
    }
    result
}
#[cfg(test)]
mod tests {
    #[test]
    fn lots_round_and_group_without_negative_quantities() {
        for (input, expected) in [
            (469130.535, "469,131"),
            (0., "0"),
            (999., "999"),
            (1000., "1,000"),
            (1234567., "1,234,567"),
            (-1., "0"),
            (-0.5, "0"),
            (0.5, "1"),
            (999.5, "1,000"),
            (f64::NAN, "—"),
            (f64::INFINITY, "—"),
        ] {
            assert_eq!(super::volume(input), expected);
        }
    }
}
