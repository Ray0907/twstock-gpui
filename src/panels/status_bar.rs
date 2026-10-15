use crate::{
    design::parts,
    market::{MarketStore, local_time, now},
};
use gpui_kit::{App, IntoElement};
pub fn render(store: &MarketStore, cx: &App) -> impl IntoElement {
    let markets = store
        .indices
        .iter()
        .map(|i| {
            (
                i.name.clone(),
                format!("{:.2}", i.last.0 as f64 / 10000.),
                i.change_percent_units as f64 / 10000.,
            )
        })
        .collect();
    let source = store
        .snapshots()
        .find_map(|q| q.source.as_deref())
        .unwrap_or("MIS");
    let mut status = format!(
        "{} · {}",
        if source == "Mock" {
            "範例資料"
        } else {
            source
        },
        store.status_text(now())
    );
    if source != "Mock"
        && let Some((first, last)) = store.data_time_range()
    {
        status.push_str(&format!(
            " · 來源時間 {}",
            local_time(first).format("%Y-%m-%d %H:%M:%S")
        ));
        if first != last {
            status.push_str(&format!(
                "–{}",
                local_time(last).format("%Y-%m-%d %H:%M:%S")
            ));
        }
    }
    parts::status_bar(markets, status, cx)
}
