use super::watchlist::WatchlistDelegate;
use crate::{design::parts, quotes::Quote};
use gpui_kit::component::{
    dock::{BasePanel, Panel, PanelEvent},
    table::TableState,
};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

pub struct QuotePanel {
    table: Entity<TableState<WatchlistDelegate>>,
    focus: FocusHandle,
    trades_open: bool,
    _selection: Subscription,
}
impl QuotePanel {
    pub fn new(
        table: Entity<TableState<WatchlistDelegate>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let selection = cx.observe_in(&table, window, |_, _, _, cx| cx.notify());
        Self {
            table,
            focus: cx.focus_handle(),
            trades_open: parts::TRADES_DEFAULT_OPEN,
            _selection: selection,
        }
    }
}
fn bid_share(quote: &Quote) -> f32 {
    let bid: u128 = quote
        .bids
        .iter()
        .map(|(_, volume)| u128::from(*volume))
        .sum();
    let ask: u128 = quote
        .asks
        .iter()
        .map(|(_, volume)| u128::from(*volume))
        .sum();
    if bid + ask == 0 {
        f32::NAN
    } else {
        bid as f32 / (bid + ask) as f32
    }
}
impl Focusable for QuotePanel {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}
impl EventEmitter<PanelEvent> for QuotePanel {}
impl BasePanel for QuotePanel {
    fn panel_name(&self) -> &'static str {
        "quote"
    }
    fn closable(&self, _: &App) -> bool {
        false
    }
}
impl Panel for QuotePanel {
    fn title(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        parts::panel_header("行情", cx)
    }
    fn tab_name(&self, _: &App) -> Option<SharedString> {
        Some("行情".into())
    }
}
impl Render for QuotePanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let table = self.table.read(cx);
        let Some(quote) = table
            .delegate()
            .quote(table.selected_row().unwrap_or(0), cx)
        else {
            return crate::design::state::panel(
                crate::design::state::PanelState::Empty("尚無行情 · 請加入自選商品".into()),
                cx,
            )
            .into_any_element();
        };
        let mut pane = parts::quote_pane()
            .track_focus(&self.focus)
            .child(parts::quote_summary(&quote, bid_share(&quote), cx))
            .when(quote.has_trades, |pane| {
                pane.child(
                    parts::trade_toggle(self.trades_open, cx).on_click(cx.listener(
                        |this, _, _, cx| {
                            this.trades_open = !this.trades_open;
                            cx.notify();
                        },
                    )),
                )
            });
        if self.trades_open && quote.has_trades {
            pane = pane.child(parts::trade_feed(&quote, cx));
        }
        pane.into_any_element()
    }
}
#[cfg(test)]
mod tests {
    use super::bid_share;
    #[test]
    fn depth_ratio_matches_quantities_and_handles_empty_depth() {
        let mut quote = crate::quotes::sample().remove(0);
        let bid: u64 = quote.bids.iter().map(|(_, qty)| qty).sum();
        let ask: u64 = quote.asks.iter().map(|(_, qty)| qty).sum();
        assert!((bid_share(&quote) - bid as f32 / (bid + ask) as f32).abs() < 1e-6);
        quote.bids = vec![(0., 0); 5];
        quote.asks = vec![(0., 0); 5];
        assert!(bid_share(&quote).is_nan());
    }
}
