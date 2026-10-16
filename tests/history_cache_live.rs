//! Opt in explicitly: TWSTOCK_GPUI_LIVE=1 cargo test --test history_cache_live -- --nocapture
use std::time::Instant;
use twstock_gpui::market::*;

#[test]
fn cold_and_warm_two_instruments() {
    if std::env::var("TWSTOCK_GPUI_LIVE").as_deref() != Ok("1") {
        return;
    }
    let result = measure();
    let output = match &result {
        Ok(output) => output.clone(),
        Err(error) => format!("History cache timing FAILED: {error}\n"),
    };
    print!("{output}");
    std::fs::write("/tmp/history-cache-timing.txt", output).unwrap();
    result.unwrap();
}
fn measure() -> Result<String, String> {
    // The only private-path access is the explicitly requested read-only watchlist read.
    let instruments = load_watchlist()?.ok_or("private watchlist required")?;
    if instruments.len() < 2 {
        return Err("two watchlist instruments required".into());
    }
    let cache = tempfile::tempdir().map_err(|e| e.to_string())?;
    let at = now();
    let mut output =
        String::from("2 watchlist instruments, 100 bars each; isolated temporary cache\n");
    let mut cold = Vec::new();
    for label in ["cold", "warm"] {
        let started = Instant::now();
        let mut provider = HistoryProvider::new_with_cache(cache.path().join("history"))?;
        let mut histories = Vec::new();
        for instrument in instruments.iter().take(2) {
            let rows = provider.recent(&instrument.symbol, 100, at)?;
            if rows.len() != 100 {
                return Err("expected 100 bars".into());
            }
            histories.push(rows);
        }
        let requests = provider.request_count();
        output.push_str(&format!(
            "{label}: {:.3} s, {requests} STOCK_DAY requests\n",
            started.elapsed().as_secs_f64()
        ));
        if label == "cold" {
            cold = histories;
        } else {
            if histories != cold {
                return Err("warm history differs from cold history".into());
            }
            if requests != 2 {
                return Err(format!("warm expected 2 requests, got {requests}"));
            }
        }
    }
    Ok(output)
}
