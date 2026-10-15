use super::{Candle, Price, SessionState, Snapshot, local_time, nice_ticks};

#[derive(Clone, Debug)]
pub struct PositionSummary {
    pub recent_count: usize,
    pub recent_low: Option<Price>,
    pub recent_high: Option<Price>,
    pub recent_percentile: Option<f64>,
    pub volume_ratio: Option<f64>,
    pub intraday: bool,
}

/// A flat range sits at the midpoint; out-of-range prices pin to the nearest end.
pub fn range_percentile(low: Price, high: Price, current: Option<Price>) -> Option<f64> {
    let current = current?;
    if low > high {
        return None;
    }
    if low == high {
        return Some(match current.cmp(&low) {
            std::cmp::Ordering::Less => 0.,
            std::cmp::Ordering::Equal => 50.,
            std::cmp::Ordering::Greater => 100.,
        });
    }
    Some(
        ((current.0 as f64 - low.0 as f64) / (high.0 as f64 - low.0 as f64) * 100.).clamp(0., 100.),
    )
}

pub fn position_summary(q: &Snapshot, history: &[Candle]) -> PositionSummary {
    let recent_low = history.iter().map(|c| c.low).min();
    let recent_high = history.iter().map(|c| c.high).max();
    let recent_percentile = recent_low
        .zip(recent_high)
        .and_then(|(lo, hi)| range_percentile(lo, hi, q.last));
    let volume_ratio = q.as_of.zip(q.volume).and_then(|(at, volume)| {
        let date = local_time(at).date_naive();
        let previous: Vec<_> = history
            .iter()
            .filter(|c| c.time < date)
            .rev()
            .take(5)
            .collect();
        if previous.len() < 5 {
            return None;
        }
        let sum: u128 = previous.iter().map(|c| u128::from(c.volume)).sum();
        (sum > 0).then(|| volume as f64 * 5. / sum as f64)
    });
    PositionSummary {
        recent_count: history.len(),
        recent_low,
        recent_high,
        recent_percentile,
        volume_ratio,
        intraday: !matches!(
            q.session,
            Some(SessionState::PostClose | SessionState::Closed)
        ),
    }
}

/// Nearest-rank P95: fewer than 20 samples retain their maximum.
pub fn volume_p95(volumes: &[u64]) -> Option<u64> {
    if volumes.is_empty() {
        return None;
    }
    let mut sorted = volumes.to_vec();
    sorted.sort_unstable();
    Some(sorted[(sorted.len() * 95).div_ceil(100) - 1])
}

pub fn volume_clip_limit(volumes: &[u64]) -> Option<u64> {
    let p95 = volume_p95(volumes)?;
    let limit = nice_ticks(0, (i128::from(p95) * 6 + 4) / 5, 3).max;
    let max = i128::from(*volumes.iter().max()?);
    (max * 2 > limit * 5).then_some(limit as u64)
}
