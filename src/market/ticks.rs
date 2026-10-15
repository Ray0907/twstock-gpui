//! Display-axis ticks in canonical integer units; no UI or floating-point money math.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NiceTicks {
    pub min: i128,
    pub max: i128,
    pub step: i128,
    pub values: Vec<i128>,
}
/// Inputs are raw Price units or share counts. Both fit within i64/u64.
pub fn nice_ticks(low: i128, high: i128, target_intervals: usize) -> NiceTicks {
    assert!(target_intervals > 0, "tick interval count must be positive");
    let (mut low, mut high) = (low.min(high), low.max(high));
    if low == high {
        let pad = (low.abs() / 100).max(1);
        low -= pad;
        high += pad;
    }
    let target = ((high - low + target_intervals as i128 - 1) / target_intervals as i128).max(1);
    let mut power = 1;
    while power * 10 <= target {
        power *= 10;
    }
    // Skip fractional raw units: Price already has a fixed 0.0001 quantum.
    let step = [2, 4, 5, 10, 20]
        .into_iter()
        .map(|factor| factor * power)
        .filter(|v| v % 2 == 0)
        .map(|v| v / 2)
        .find(|step| *step >= target)
        .unwrap();
    let min = low.div_euclid(step) * step;
    let max = -(-high).div_euclid(step) * step;
    let values = (0..=(max - min) / step)
        .map(|index| min + index * step)
        .collect();
    NiceTicks {
        min,
        max,
        step,
        values,
    }
}
