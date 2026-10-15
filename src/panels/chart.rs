use super::watchlist::WatchlistDelegate;
use crate::design::{chart, parts};
use crate::market::{ChartSeries, Market, Price, Timeframe};
use gpui_kit::component::{
    dock::{BasePanel, Panel, PanelEvent},
    table::TableState,
};
use gpui_kit::*;
use std::rc::Rc;

pub struct ChartPanel {
    table: Entity<TableState<WatchlistDelegate>>,
    focus: FocusHandle,
    _selection: Subscription,
    _activation: Subscription,
    period: usize,
    viewport: Rc<chart::ChartViewport>,
    instrument: Option<(Market, SharedString)>,
    history_dirty: bool,
    history: Option<(Rc<ChartSeries>, Option<Price>)>,
    history_error: Option<String>,
}
impl ChartPanel {
    pub fn new(
        table: Entity<TableState<WatchlistDelegate>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let selection = cx.observe_in(&table, window, |this, _, _, cx| {
            this.history_dirty = true;
            cx.notify();
        });
        let activation = cx.observe_window_activation(window, |this, window, _| {
            if !window.is_window_active() {
                this.viewport.cancel_drag();
            }
        });
        Self {
            table,
            focus: cx.focus_handle(),
            _selection: selection,
            _activation: activation,
            period: 0,
            viewport: Rc::new(chart::ChartViewport::new(0)),
            instrument: None,
            history_dirty: true,
            history: None,
            history_error: None,
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
            self.instrument = None;
            self.history = None;
            self.viewport.reset(0);
            return crate::design::state::panel(
                crate::design::state::PanelState::Empty("尚無行情 · 請加入自選商品".into()),
                cx,
            )
            .into_any_element();
        };
        let instrument = (quote.market, quote.code.clone());
        let changed = self.instrument.as_ref() != Some(&instrument);
        let period = [Timeframe::Day, Timeframe::Week, Timeframe::Month][self.period];
        if changed || self.history_dirty {
            let old_total = self.history.as_ref().map_or(0, |(s, _)| s.candles.len());
            self.history_error = None;
            self.history = table
                .delegate()
                .chart_series(table.selected_row().unwrap_or(0), cx)
                .and_then(|(s, current)| match s.for_period(period) {
                    Ok(s) => Some((Rc::new(s), current)),
                    Err(reason) => {
                        self.history_error = Some(reason);
                        None
                    }
                });
            let total = self.history.as_ref().map_or(0, |(s, _)| s.candles.len());
            if changed || self.viewport.range.get().count == old_total {
                self.viewport.reset(total);
            } else {
                self.viewport
                    .range
                    .set(self.viewport.range.get().clamp(total));
            }
            self.instrument = Some(instrument);
            self.history_dirty = false;
        }
        parts::pane("chart-panel")
            .track_focus(&self.focus)
            .child(parts::chart_header(&quote, cx))
            .child(parts::periods(
                self.period,
                cx.listener(|this, index, _, cx| {
                    if this.period != *index {
                        this.period = *index;
                        this.history_dirty = true;
                        this.history = None;
                        this.viewport.reset(0);
                        cx.notify();
                    }
                }),
                cx,
            ))
            .child(chart::history_view_with_viewport(
                self.history.clone(),
                self.history_error
                    .as_deref()
                    .or(quote.history_error.as_deref()),
                period,
                self.viewport.clone(),
                cx,
            ))
            .into_any_element()
    }
}
