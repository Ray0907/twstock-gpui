use super::*;
use std::cell::Cell;

#[derive(Clone, Default)]
struct DragState(Rc<Cell<Option<(Pixels, VisibleRange)>>>);
impl DragState {
    fn get(&self) -> Option<(Pixels, VisibleRange)> {
        self.0.get()
    }
    fn start(&self, x: Pixels, range: VisibleRange) {
        self.0.set(Some((x, range)));
    }
    fn cancel(&self) {
        self.0.set(None);
    }
}
impl Drop for DragState {
    fn drop(&mut self) {
        self.cancel();
    }
}

/// Owned by the panel; plots only share this state across element rebuilds.
pub struct ChartViewport {
    pub range: Cell<VisibleRange>,
    drag: DragState,
    zoom_remainder: Cell<f64>,
    pan_remainder: Cell<f64>,
}
impl ChartViewport {
    pub fn new(total: usize) -> Self {
        Self {
            range: Cell::new(VisibleRange::reset(total)),
            drag: DragState::default(),
            zoom_remainder: Cell::new(0.),
            pan_remainder: Cell::new(0.),
        }
    }
    pub fn reset(&self, total: usize) {
        self.range.set(VisibleRange::reset(total));
        self.drag.cancel();
        self.zoom_remainder.set(0.);
        self.pan_remainder.set(0.);
    }
    pub(crate) fn cancel_drag(&self) {
        self.drag.cancel();
    }
    fn rebase_drag(&self, position: Pixels) {
        if self.drag.get().is_some() {
            self.drag.start(position, self.range.get());
        }
    }
    fn zoom(&self, total: usize, anchor: f64, delta: f64) {
        if !delta.is_finite() || delta == 0. {
            return;
        }
        let old = self.range.get();
        let delta = self.zoom_remainder.get() + delta;
        let next = old.zoom_at(total, anchor, delta);
        let clamped =
            (delta > 0. && next.count == total.min(20)) || (delta < 0. && next.count == total);
        self.zoom_remainder
            .set(if next == old && !clamped { delta } else { 0. });
        self.range.set(next);
        self.pan_remainder.set(0.);
    }
    fn zoom_cursor(&self, total: usize, cursor: f64, width: f64, delta: f64) {
        let old = self.range.get();
        self.zoom(total, (cursor / width).clamp(0., 1.), delta);
        let next = self.range.get();
        if next.count != old.count {
            self.range.set(ChartGeometry::anchor_zoom(
                old,
                next,
                total,
                cursor.clamp(0., width),
                width,
            ));
        }
    }
    fn pan(&self, total: usize, bars: f64) {
        if !bars.is_finite() || bars == 0. {
            return;
        }
        let old = self.range.get();
        let bars = self.pan_remainder.get() + bars;
        let next = old.pan_by(total, bars);
        let clamped =
            (bars < 0. && next.start == 0) || (bars > 0. && next.start + next.count == total);
        self.pan_remainder.set(if !clamped {
            bars - (next.start as f64 - old.start as f64)
        } else {
            0.
        });
        self.range.set(next);
    }
    pub(super) fn mount(
        self: &Rc<Self>,
        bounds: Bounds<Pixels>,
        g: &ChartGeometry,
        total: usize,
        hitbox: &Hitbox,
        window: &mut Window,
    ) {
        // GPUI drops this clone when the plot is no longer painted, ending any capture.
        window.with_global_id("chart-drag-liveness".into(), |id, window| {
            window.with_element_state(id, |previous: Option<DragState>, _| {
                let drag = previous
                    .filter(|drag| Rc::ptr_eq(&drag.0, &self.drag.0))
                    .unwrap_or_else(|| self.drag.clone());
                ((), drag)
            });
        });
        let view = window.current_view();
        let width = g.layout.price.size.width.as_f32() as f64;
        let step = g.x.step() as f64;
        let count = self.range.get().count;
        let current_step = move |count_now| {
            if count_now == count {
                step
            } else {
                ChartGeometry::candle_scale(count_now, width as f32).step() as f64
            }
        };
        let state = self.clone();
        let scroll_hitbox = hitbox.clone();
        window.on_mouse_event(move |e: &ScrollWheelEvent, phase, window, cx| {
            if !phase.bubble()
                || window.default_prevented()
                || !scroll_hitbox.should_handle_scroll(window)
            {
                return;
            }
            let delta = e.delta.pixel_delta(px(16.));
            let old = state.range.get();
            if delta.x.abs() > delta.y.abs() {
                state.pan(
                    total,
                    -delta.x.as_f32() as f64 / current_step(state.range.get().count),
                );
            } else {
                state.zoom_cursor(
                    total,
                    (e.position.x - bounds.left()).as_f32() as f64,
                    width,
                    delta.y.as_f32() as f64 / 300.,
                );
            }
            if old != state.range.get() {
                state.rebase_drag(e.position.x);
                window.prevent_default();
                cx.notify(view);
            }
        });
        let state = self.clone();
        let pinch_hitbox = hitbox.clone();
        window.on_mouse_event(move |e: &PinchEvent, phase, window, cx| {
            if !phase.bubble() || window.default_prevented() || !pinch_hitbox.is_hovered(window) {
                return;
            }
            let old = state.range.get();
            state.zoom_cursor(
                total,
                (e.position.x - bounds.left()).as_f32() as f64,
                width,
                e.delta as f64,
            );
            if old != state.range.get() {
                state.rebase_drag(e.position.x);
                window.prevent_default();
                cx.notify(view);
            }
        });
        let state = self.clone();
        let down_hitbox = hitbox.clone();
        window.on_mouse_event(move |e: &MouseDownEvent, phase, window, cx| {
            if !phase.bubble()
                || window.default_prevented()
                || e.button != MouseButton::Left
                || !down_hitbox.is_hovered(window)
            {
                return;
            }
            if e.click_count == 2 {
                state.reset(total);
                cx.notify(view);
            } else {
                state.drag.start(e.position.x, state.range.get());
            }
            window.prevent_default();
        });
        let state = self.clone();
        let move_hitbox = hitbox.clone();
        window.on_mouse_event(move |e: &MouseMoveEvent, phase, window, cx| {
            if phase.capture() {
                // Probe inside the plot even while captured outside it; overlays still occlude it.
                let probe = point(
                    e.position
                        .x
                        .clamp(bounds.left() + px(0.5), bounds.right() - px(0.5)),
                    e.position
                        .y
                        .clamp(bounds.top() + px(0.5), bounds.bottom() - px(0.5)),
                );
                if e.pressed_button != Some(MouseButton::Left)
                    || (state.drag.get().is_some() && !move_hitbox.is_hovered_at(probe, window))
                {
                    state.drag.cancel();
                }
                return;
            }
            if let Some((origin, range)) = state.drag.get() {
                let delta = (origin - e.position.x).as_f32() as f64 / current_step(range.count);
                let next = range.pan_by(total, delta);
                if state.range.replace(next) != next {
                    cx.notify(view);
                }
            }
        });
        let state = self.clone();
        window.on_mouse_event(move |e: &MouseUpEvent, phase, _, _| {
            if phase.capture() && e.button == MouseButton::Left {
                state.drag.cancel();
            }
        });
    }
}
#[cfg(test)]
mod tests {
    use super::ChartViewport;
    use crate::design::chart::range::VisibleRange;
    #[test]
    fn tiny_trackpad_deltas_accumulate_at_boundaries() {
        let viewport = ChartViewport::new(100);
        for _ in 0..100 {
            viewport.zoom(100, 0.5, 0.001);
        }
        assert!(viewport.range.get().count < 100);
        viewport.range.set(VisibleRange {
            start: 0,
            count: 40,
        });
        for _ in 0..10 {
            viewport.pan(100, 0.1);
        }
        assert_eq!(viewport.range.get().start, 1);
    }
    #[test]
    fn reset_clears_gesture_residue() {
        let viewport = ChartViewport::new(100);
        viewport.zoom(100, 0.5, 1.);
        viewport.pan(100, 0.1);
        viewport.reset(100);
        assert_eq!(viewport.range.get(), VisibleRange::reset(100));
        assert_eq!(viewport.zoom_remainder.get(), 0.);
        assert_eq!(viewport.pan_remainder.get(), 0.);
        assert!(viewport.drag.get().is_none());
    }
}
