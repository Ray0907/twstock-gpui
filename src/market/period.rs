use super::*;
use chrono::Datelike;

/// Dates label the last observed trading day, including an unfinished period.
pub fn aggregate_candles(rows: &[Candle], period: Timeframe) -> Result<Vec<Candle>, String> {
    if rows.windows(2).any(|pair| pair[0].time >= pair[1].time) {
        return Err("日線日期需由舊至新且不可重複".into());
    }
    let key = |c: &Candle| match period {
        Timeframe::Day => (c.time.year(), c.time.ordinal()),
        Timeframe::Week => {
            let week = c.time.iso_week();
            (week.year(), week.week())
        }
        Timeframe::Month => (c.time.year(), c.time.month()),
    };
    let mut out: Vec<Candle> = Vec::new();
    for row in rows {
        if let Some(last) = out.last_mut()
            && key(last) == key(row)
        {
            last.high = last.high.max(row.high);
            last.low = last.low.min(row.low);
            last.close = row.close;
            last.time = row.time;
            last.volume = last
                .volume
                .checked_add(row.volume)
                .ok_or("期間成交量加總溢位")?;
        } else {
            out.push(row.clone());
        }
    }
    Ok(out)
}
impl ChartSeries {
    pub fn for_period(self, period: Timeframe) -> Result<Self, String> {
        let candles = aggregate_candles(&self.candles, period)?;
        let averages = [5, 10, 20].map(|n| moving_average(&candles, n));
        Ok(Self {
            candles,
            averages,
            lot_size: self.lot_size,
        })
    }
}
