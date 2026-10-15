use super::watchlist::WatchlistDelegate;
use crate::design::{chart, parts};
use gpui_kit::component::{
    dock::{BasePanel, Panel, PanelEvent},
    table::TableState,
};
use gpui_kit::*;

pub struct ChartPanel {
    table: Entity<TableState<WatchlistDelegate>>,
    focus: FocusHandle,
    _selection: Subscription,
    period: usize,
}
impl ChartPanel {
    pub fn new(
        table: Entity<TableState<WatchlistDelegate>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let selection = cx.observe_in(&table, window, |_, _, _, cx| cx.notify());
        Self {
            table,
            focus: cx.focus_handle(),
            _selection: selection,
            period: 0,
        }
    }
}
impl Focusable for ChartPanel {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}
impl EventEmitter<PanelEvent> for ChartPanel {}
impl BasePanel for ChartPanel {
    fn panel_name(&self) -> &'static str {
        "chart"
    }
    fn closable(&self, _: &App) -> bool {
        false
    }
}
impl Panel for ChartPanel {
    fn title(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        parts::panel_header("走勢", cx)
    }
    fn tab_name(&self, _: &App) -> Option<SharedString> {
        Some("走勢".into())
    }
}
impl Render for ChartPanel {
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
        parts::pane("chart-panel")
            .track_focus(&self.focus)
            .child(parts::chart_header(&quote, cx))
            .child(parts::periods(
                self.period,
                cx.listener(|this, index, _, cx| {
                    this.period = *index;
                    cx.notify();
                }),
                cx,
            ))
            .child(chart::history_view(
                table
                    .delegate()
                    .chart_series(table.selected_row().unwrap_or(0), cx),
                quote.history_error.as_deref(),
                [
                    crate::market::Timeframe::Day,
                    crate::market::Timeframe::Week,
                    crate::market::Timeframe::Month,
                ][self.period],
                cx,
            ))
            .into_any_element()
    }
}
