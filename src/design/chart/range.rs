//! Index ranges over the full, already-aggregated candle series. No UI dependencies.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct VisibleRange {
    pub start: usize,
    pub count: usize,
}
impl VisibleRange {
    pub fn reset(total: usize) -> Self {
        Self {
            start: 0,
            count: total,
        }
    }
    pub fn clamp(self, total: usize) -> Self {
        let count = self.count.clamp(total.min(20), total);
        Self {
            start: self.start.min(total - count),
            count,
        }
    }
    /// Positive logarithmic deltas zoom in. Integer indices round the anchor by at most half a bar.
    pub fn zoom_at(self, total: usize, anchor: f64, delta: f64) -> Self {
        if !delta.is_finite() || delta == 0. || !anchor.is_finite() {
            return self;
        }
        let old = self.clamp(total);
        let count = (old.count as f64 * (-delta).exp())
            .round()
            .clamp(total.min(20) as f64, total as f64) as usize;
        let start = (old.start as f64 + anchor.clamp(0., 1.) * (old.count as f64 - count as f64))
            .round()
            .clamp(0., (total - count) as f64) as usize;
        Self { start, count }
    }
    pub fn pan_by(self, total: usize, bars: f64) -> Self {
        if !bars.is_finite() || bars == 0. {
            return self;
        }
        let old = self.clamp(total);
        Self {
            start: (old.start as f64 + bars)
                .round()
                .clamp(0., (total - old.count) as f64) as usize,
            ..old
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn anchor_stays_fixed_to_nearest_bar() {
        let full = VisibleRange::reset(100);
        let zoom = full.zoom_at(100, 0.4, 2f64.ln());
        assert_eq!(
            zoom,
            VisibleRange {
                start: 20,
                count: 50
            }
        );
        assert_eq!(
            full.start as f64 + 0.4 * full.count as f64,
            zoom.start as f64 + 0.4 * zoom.count as f64
        );
    }
    #[test]
    fn clamps_zoom_and_pan() {
        let zoom = VisibleRange::reset(100).zoom_at(100, 0.5, 100.);
        assert_eq!(zoom.count, 20);
        assert_eq!(zoom.pan_by(100, -1000.).start, 0);
        assert_eq!(zoom.pan_by(100, 1000.).start, 80);
        assert_eq!(zoom.zoom_at(100, 0.5, -100.), VisibleRange::reset(100));
        assert_eq!(
            VisibleRange {
                start: 200,
                count: 1
            }
            .clamp(100),
            VisibleRange {
                start: 80,
                count: 20
            }
        );
    }
    #[test]
    fn repeated_zoom_returns_to_full() {
        let mut range = VisibleRange::reset(100);
        for _ in 0..50 {
            range = range.zoom_at(100, 0.65, 0.15);
        }
        assert_eq!(range.count, 20);
        for _ in 0..50 {
            range = range.zoom_at(100, 0.65, -0.15);
        }
        assert_eq!(range, VisibleRange::reset(100));
    }
    #[test]
    fn tiny_totals_and_invalid_deltas() {
        for total in [0, 1, 5, 20] {
            let full = VisibleRange::reset(total);
            assert_eq!(full.zoom_at(total, 0.5, 5.), full);
            assert_eq!(full.pan_by(total, 3.), full);
        }
        let range = VisibleRange {
            start: 30,
            count: 40,
        };
        for delta in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, 0.] {
            assert_eq!(range.zoom_at(100, 0.5, delta), range);
            assert_eq!(range.pan_by(100, delta), range);
        }
        assert_eq!(range.zoom_at(100, f64::NAN, 1.), range);
    }
}
