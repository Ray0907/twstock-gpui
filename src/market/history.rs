use super::*;
use chrono::{Datelike, NaiveDate};
use serde_json::Value;
use std::{thread, time::Duration};
pub fn parse_history(body: &str) -> Result<Vec<Candle>, String> {
    let value: Value = serde_json::from_str(body).map_err(|_| "invalid history JSON")?;
    if value["stat"].as_str() != Some("OK") {
        return Err(value["stat"]
            .as_str()
            .unwrap_or("history request failed")
            .into());
    }
    value["data"]
        .as_array()
        .ok_or("missing history data")?
        .iter()
        .map(|row| {
            let fields = row.as_array().ok_or("invalid history row")?;
            let text = |index: usize| {
                fields
                    .get(index)
                    .and_then(Value::as_str)
                    .ok_or("missing history field")
            };
            let date = text(0)?
                .split('/')
                .map(str::parse::<i32>)
                .collect::<Result<Vec<_>, _>>()
                .map_err(|_| "invalid ROC date")?;
            if date.len() != 3 {
                return Err("invalid ROC date".into());
            }
            let year = date[0].checked_add(1911).ok_or("ROC year overflow")?;
            let time = NaiveDate::from_ymd_opt(year, date[1] as u32, date[2] as u32)
                .ok_or("invalid history date")?;
            let volume = text(1)?
                .replace(',', "")
                .parse::<u64>()
                .map_err(|_| "invalid history volume")?;
            let price = |i| -> Result<Price, String> {
                let p = text(i)?.parse::<Price>()?;
                if p.0 < 0 {
                    return Err("negative history price".into());
                }
                Ok(p)
            };
            let candle = Candle {
                time,
                open: price(3)?,
                high: price(4)?,
                low: price(5)?,
                close: price(6)?,
                volume,
            };
            if candle.low > candle.high
                || candle.open < candle.low
                || candle.close < candle.low
                || candle.open > candle.high
                || candle.close > candle.high
            {
                return Err("inconsistent history OHLC".into());
            }
            Ok(candle)
        })
        .collect()
}
fn monthly_rows(body: &str) -> Result<Vec<Candle>, String> {
    let value: Value = serde_json::from_str(body).map_err(|_| "invalid history JSON")?;
    if value["stat"]
        .as_str()
        .is_some_and(|s| s.contains("沒有符合條件"))
    {
        return Ok(vec![]);
    }
    parse_history(body)
}
pub struct HistoryProvider<T: HttpTransport = CurlSession> {
    transport: T,
}
impl HistoryProvider<CurlSession> {
    pub fn new() -> Result<Self, String> {
        Ok(Self::with_transport(CurlSession::new(
            "https://www.twse.com.tw/",
        )?))
    }
}
impl<T: HttpTransport> HistoryProvider<T> {
    pub fn with_transport(transport: T) -> Self {
        Self { transport }
    }
    pub fn recent(
        &mut self,
        symbol: &Symbol,
        count: usize,
        now: i64,
    ) -> Result<Vec<Candle>, String> {
        if symbol.market != Market::Listed {
            return Err("上櫃歷史尚未接入".into());
        }
        let end = local_time(now).date_naive();
        let mut month = NaiveDate::from_ymd_opt(end.year(), end.month(), 1).unwrap();
        let mut rows = Vec::new();
        if count == 0 {
            return Ok(rows);
        }
        for _ in 0..12 {
            let body=self.transport.get(&format!("https://www.twse.com.tw/rwd/zh/afterTrading/STOCK_DAY?date={}&stockNo={}&response=json",month.format("%Y%m%d"),symbol.code))?;
            let data = monthly_rows(&body)?;
            rows.extend(data.into_iter().filter(|c| c.time <= end));
            rows.sort_by_key(|c| c.time);
            rows.dedup_by_key(|c| c.time);
            if rows.len() >= count {
                return Ok(rows.split_off(rows.len() - count));
            }
            month = month
                .pred_opt()
                .ok_or("history date underflow")?
                .with_day(1)
                .unwrap();
            thread::sleep(Duration::from_secs(2));
        }
        Err(format!("只有 {} 根歷史，未滿 {count}", rows.len()))
    }
}
impl<T: HttpTransport> History for HistoryProvider<T> {
    fn candles(
        &mut self,
        symbol: &Symbol,
        timeframe: Timeframe,
        range: DateRange,
    ) -> Result<Vec<Candle>, String> {
        if timeframe != Timeframe::Day || symbol.market != Market::Listed {
            return Err("unsupported history".into());
        }
        let mut month = range.end.with_day(1).ok_or("invalid range")?;
        let mut rows = Vec::new();
        while month <= range.end && month.with_day(1).unwrap() >= range.start.with_day(1).unwrap() {
            let body=self.transport.get(&format!("https://www.twse.com.tw/rwd/zh/afterTrading/STOCK_DAY?date={}&stockNo={}&response=json",month.format("%Y%m%d"),symbol.code))?;
            rows.extend(
                monthly_rows(&body)?
                    .into_iter()
                    .filter(|c| c.time >= range.start && c.time <= range.end),
            );
            month = month
                .pred_opt()
                .ok_or("history date underflow")?
                .with_day(1)
                .unwrap();
            if month >= range.start.with_day(1).unwrap() {
                thread::sleep(Duration::from_secs(2));
            }
        }
        rows.sort_by_key(|c| c.time);
        rows.dedup_by_key(|c| c.time);
        Ok(rows)
    }
}
