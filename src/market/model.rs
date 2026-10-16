use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use std::{fmt, str::FromStr};

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, Eq, PartialEq, Ord, PartialOrd)]
pub struct Price(pub i64);
impl FromStr for Price {
    type Err = String;
    fn from_str(input: &str) -> Result<Self, String> {
        let input = input.replace(',', "");
        let (sign, input) = if let Some(p) = input.strip_prefix('-') {
            (-1, p)
        } else {
            (1, input.as_str())
        };
        let parts = input.split('.').collect::<Vec<_>>();
        if parts.len() > 2
            || parts[0].is_empty()
            || !parts.iter().all(|p| p.bytes().all(|b| b.is_ascii_digit()))
        {
            return Err("invalid fixed-point price".into());
        }
        let whole = parts[0].parse::<i64>().map_err(|_| "price overflow")?;
        let fraction = parts.get(1).copied().unwrap_or("");
        if fraction.len() > 4 {
            return Err("price exceeds four decimal places".into());
        }
        let fraction = format!("{fraction:0<4}")
            .parse::<i64>()
            .map_err(|_| "invalid fraction")?;
        let value = whole
            .checked_mul(10000)
            .and_then(|p| p.checked_add(fraction))
            .and_then(|p| p.checked_mul(sign))
            .ok_or("price overflow")?;
        Ok(Self(value))
    }
}
impl fmt::Display for Price {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = i128::from(self.0).abs();
        write!(
            f,
            "{}{}.{:04}",
            if self.0 < 0 { "-" } else { "" },
            value / 10000,
            value % 10000
        )
    }
}
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, Eq, PartialEq, Hash)]
pub enum Market {
    #[default]
    #[serde(alias = "tse", alias = "listed")]
    Listed,
    #[serde(alias = "otc", alias = "tpex")]
    Otc,
}
impl Market {
    pub fn exchange(self) -> &'static str {
        match self {
            Self::Listed => "tse",
            Self::Otc => "otc",
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Listed => "上市",
            Self::Otc => "上櫃",
        }
    }
}
#[derive(Clone, Debug, Default, Deserialize, Serialize, Eq, PartialEq, Hash)]
pub struct Symbol {
    pub market: Market,
    pub code: String,
}
impl Symbol {
    pub fn new(market: Market, code: &str) -> Result<Self, String> {
        if !(4..=8).contains(&code.len())
            || !code
                .bytes()
                .all(|b| b.is_ascii_digit() || b.is_ascii_uppercase())
        {
            return Err("instrument code must have 4–8 uppercase letters/digits".into());
        }
        Ok(Self {
            market,
            code: code.into(),
        })
    }
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, PartialEq)]
pub enum TickRule {
    Stock,
    Etf,
}
impl TickRule {
    pub fn tick(self, price: Price) -> Price {
        let p = price.0;
        Price(match self {
            Self::Etf => {
                if p < 500_000 {
                    100
                } else {
                    500
                }
            }
            Self::Stock => match p {
                p if p < 100_000 => 100,
                p if p < 500_000 => 500,
                p if p < 1_000_000 => 1000,
                p if p < 5_000_000 => 5000,
                p if p < 10_000_000 => 10000,
                _ => 50000,
            },
        })
    }
    pub fn previous_price(self, price: Price) -> Price {
        Price(
            price
                .0
                .saturating_sub(self.tick(Price(price.0.saturating_sub(1))).0),
        )
    }
    pub fn next_price(self, price: Price) -> Price {
        Price(price.0.saturating_add(self.tick(price).0))
    }
    pub fn round(self, price: Price) -> Price {
        let t = self.tick(price).0;
        Price((price.0 + t / 2) / t * t)
    }
}
#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
pub enum LimitRule {
    Source,
    Percent(u32),
}
#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
pub struct Instrument {
    pub symbol: Symbol,
    pub name: String,
    pub tick_rule: TickRule,
    pub lot_size: u64,
    pub limit_rule: LimitRule,
}
impl Instrument {
    pub fn new(symbol: Symbol) -> Self {
        let tick_rule = TickRule::Stock;
        Self {
            name: symbol.code.clone(),
            symbol,
            tick_rule,
            lot_size: 1000,
            limit_rule: LimitRule::Source,
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SessionState {
    PreOpen,
    Open,
    PostClose,
    Closed,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Timeframe {
    Day,
    Week,
    Month,
}
#[derive(Clone, Debug)]
pub struct DateRange {
    pub start: NaiveDate,
    pub end: NaiveDate,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct Candle {
    pub time: NaiveDate,
    pub open: Price,
    pub high: Price,
    pub low: Price,
    pub close: Price,
    pub volume: u64,
}
impl Candle {
    /// Domain quantities are shares, never rounded fractional lots.
    pub fn volume_lots(&self, lot_size: u64) -> f64 {
        self.volume as f64 / lot_size.max(1) as f64
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct Depth {
    pub price: Price,
    pub quantity: u64,
    pub as_of: i64,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Side {
    Buy,
    Sell,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Trade {
    pub symbol: Symbol,
    pub time: i64,
    pub price: Price,
    pub quantity: u64,
    pub side: Option<Side>,
}
#[derive(Clone, Debug, Default, PartialEq)]
pub struct QuoteUpdate {
    pub symbol: Symbol,
    pub instrument: Option<Box<Instrument>>,
    pub last: Option<Price>,
    pub volume: Option<u64>,
    pub open: Option<Price>,
    pub high: Option<Price>,
    pub low: Option<Price>,
    pub prev_close: Option<Price>,
    pub limit_up: Option<Price>,
    pub limit_down: Option<Price>,
    pub bids: Option<Vec<Depth>>,
    pub asks: Option<Vec<Depth>>,
    pub as_of: Option<i64>,
    pub source: Option<String>,
    pub session: Option<SessionState>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LinkState {
    Connecting,
    Live,
    Stale { since: i64 },
    Failed { reason: String, retry_in: u64 },
}
#[derive(Clone, Debug, PartialEq)]
pub enum MarketEvent {
    Polled { at: i64 },
    Quote(QuoteUpdate),
    Trade(Trade),
    Link(LinkState),
}
#[derive(Clone, Debug)]
pub struct ChartSeries {
    pub candles: Vec<Candle>,
    pub averages: [Vec<Option<Price>>; 3],
    pub lot_size: u64,
}
#[derive(Clone, Debug)]
pub struct IndexSnapshot {
    pub name: String,
    pub last: Price,
    pub change_percent_units: i64,
}
#[derive(Clone, Debug)]
pub struct Capabilities {
    pub depth_levels: usize,
    pub max_subscriptions: usize,
    pub min_interval: u64,
    pub has_trades: bool,
    pub timeframes: Vec<Timeframe>,
    pub realtime: bool,
    pub delay_seconds: Option<u64>,
}
impl Capabilities {
    pub fn mis() -> Self {
        Self {
            depth_levels: 5,
            max_subscriptions: 100,
            min_interval: 5,
            has_trades: false,
            timeframes: vec![Timeframe::Day],
            realtime: false,
            delay_seconds: None,
        }
    }
}
pub fn moving_average(candles: &[Candle], period: usize) -> Vec<Option<Price>> {
    assert!(period > 0, "MA period must be positive");
    let mut sum = 0_i128;
    candles
        .iter()
        .enumerate()
        .map(|(ix, c)| {
            sum += i128::from(c.close.0);
            if ix >= period {
                sum -= i128::from(candles[ix - period].close.0);
            }
            if ix + 1 < period {
                None
            } else {
                Some(Price((sum / period as i128) as i64))
            }
        })
        .collect()
}
