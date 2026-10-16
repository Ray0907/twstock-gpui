use std::time::Duration;
use twstock_gpui::market::HistoryClock;

#[derive(Default)]
pub struct FakeClock(Duration);
impl HistoryClock for FakeClock {
    fn elapsed(&self) -> Duration { self.0 }
    fn sleep(&mut self, duration: Duration) { self.0 += duration; }
}
