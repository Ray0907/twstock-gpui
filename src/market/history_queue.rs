use super::Symbol;
use std::{
    collections::{HashMap, HashSet, VecDeque},
    time::{Duration, Instant},
};

struct Failed {
    delay: Duration,
    retry_at: Instant,
}

/// The front stays queued while in flight; selection can replace it between month steps.
#[derive(Default)]
pub struct HistoryQueue {
    pending: VecDeque<Symbol>,
    finished: HashSet<Symbol>,
    failed: HashMap<Symbol, Failed>,
}
impl HistoryQueue {
    pub fn enqueue(&mut self, symbols: &[Symbol], selected: Option<&Symbol>) {
        self.enqueue_at(symbols, selected, Instant::now());
    }
    /// Explicit monotonic time keeps cooldown tests independent of wall time and sleeping.
    pub fn enqueue_at(&mut self, symbols: &[Symbol], selected: Option<&Symbol>, at: Instant) {
        for symbol in symbols {
            if !self.finished.contains(symbol)
                && !self.pending.contains(symbol)
                && self
                    .failed
                    .get(symbol)
                    .is_none_or(|failed| at >= failed.retry_at)
            {
                self.pending.push_back(symbol.clone());
            }
        }
        // Merely retaining selection during a watchlist update is not an explicit retry.
        if let Some(selected) = selected {
            self.promote(selected);
        }
    }
    pub fn select(&mut self, symbol: &Symbol) {
        if self.failed.contains_key(symbol) && !self.pending.contains(symbol) {
            self.pending.push_front(symbol.clone());
        }
        self.promote(symbol);
    }
    fn promote(&mut self, symbol: &Symbol) {
        if let Some(index) = self.pending.iter().position(|s| s == symbol) {
            self.pending.remove(index);
            self.pending.push_front(symbol.clone());
        }
    }
    pub fn next(&self) -> Option<Symbol> {
        self.pending.front().cloned()
    }
    pub fn finish(&mut self, symbol: &Symbol) {
        if let Some(index) = self.pending.iter().position(|s| s == symbol) {
            self.pending.remove(index);
            self.failed.remove(symbol);
            self.finished.insert(symbol.clone());
        }
    }
    pub fn fail(&mut self, symbol: &Symbol, at: Instant) {
        if let Some(index) = self.pending.iter().position(|s| s == symbol) {
            self.pending.remove(index);
            let delay = self
                .failed
                .get(symbol)
                .map_or(Duration::from_secs(60), |failed| {
                    (failed.delay * 2).min(Duration::from_secs(600))
                });
            self.failed.insert(
                symbol.clone(),
                Failed {
                    delay,
                    retry_at: at + delay,
                },
            );
        }
    }
    pub fn retain(&mut self, symbols: &[Symbol]) {
        self.pending.retain(|s| symbols.contains(s));
        self.finished.retain(|s| symbols.contains(s));
        self.failed.retain(|s, _| symbols.contains(s));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    #[test]
    fn success_clears_consecutive_failure_backoff() {
        let symbol = Symbol::new(super::super::Market::Listed, "AAAA").unwrap();
        let mut queue = HistoryQueue::default();
        let at = Instant::now();
        queue.enqueue(std::slice::from_ref(&symbol), None);
        queue.fail(&symbol, at);
        queue.select(&symbol);
        queue.fail(&symbol, at);
        assert_eq!(queue.failed[&symbol].delay.as_secs(), 120);
        queue.select(&symbol);
        queue.finish(&symbol);
        assert!(!queue.failed.contains_key(&symbol));
        assert!(queue.finished.contains(&symbol));
    }
}
