mod config;
mod history;
mod mis;
mod mock;
mod model;
mod store;
mod ticks;
pub use config::*;
pub use history::*;
pub use mis::*;
pub use mock::*;
pub use model::*;
pub use store::*;
pub use ticks::*;

pub type EventStream = futures::stream::BoxStream<'static, MarketEvent>;
pub trait QuoteFeed {
    fn capabilities(&self) -> Capabilities;
    fn subscribe(&mut self, symbols: Vec<Symbol>) -> EventStream;
}
pub trait History {
    fn candles(
        &mut self,
        symbol: &Symbol,
        timeframe: Timeframe,
        range: DateRange,
    ) -> Result<Vec<Candle>, String>;
}
pub fn now() -> i64 {
    chrono::Utc::now().timestamp()
}
