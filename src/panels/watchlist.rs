use crate::{
    design::{parts, space},
    quotes::{self, Quote},
};
use gpui_kit::component::{
    ElementExt,
    dock::{BasePanel, Panel, PanelEvent},
    table::{Column, TableDelegate, TableState},
};
use gpui_kit::*;

pub struct WatchlistDelegate {
    store: Entity<crate::market::MarketStore>,
    columns: Vec<Column>,
    // Presentation cache; TableState is still the selection source.
    highlighted_row: Option<usize>,
}
impl WatchlistDelegate {
    fn new(store: Entity<crate::market::MarketStore>, cx: &App) -> Self {
        Self {
            store,
            columns: parts::watch_columns(cx),
            highlighted_row: None,
        }
    }
    pub fn chart_series(
        &self,
        row: usize,
        cx: &App,
    ) -> Option<(crate::market::ChartSeries, Option<crate::market::Price>)> {
        let store = self.store.read(cx);
        let symbol = store.order.get(row)?;
        Some((store.chart_series(symbol)?, store.snapshot(symbol)?.last))
    }
    pub fn quote(&self, row: usize, cx: &App) -> Option<Quote> {
        quotes::from_store(self.store.read(cx), row)
    }
}
impl TableDelegate for WatchlistDelegate {
    fn columns_count(&self, _: &App) -> usize {
        self.columns.len()
    }
    fn rows_count(&self, cx: &App) -> usize {
        self.store.read(cx).order.len()
    }
    fn column(&self, col: usize, _: &App) -> Column {
        self.columns[col].clone()
    }
    fn render_th(
        &mut self,
        col: usize,
        _: &mut Window,
        cx: &mut Context<TableState<Self>>,
    ) -> impl IntoElement {
        parts::watch_column_header(self.columns[col].name.clone(), col, cx)
    }
    fn render_td(
        &mut self,
        row: usize,
        col: usize,
        _: &mut Window,
        cx: &mut Context<TableState<Self>>,
    ) -> impl IntoElement {
        let Some(quote) = self.quote(row, cx) else {
            return parts::panel_header("尚無行情", cx).into_any_element();
        };
        match col {
            0 => {
                parts::watch_name(&quote, self.highlighted_row == Some(row), cx).into_any_element()
            }
            1 => parts::watch_price(&quote, row, cx).into_any_element(),
            _ => parts::watch_sparkline(&quote, row, cx).into_any_element(),
        }
    }
}
#[derive(Clone)]
pub struct RemoveSelected;
gpui_kit::actions!(watchlist, [RemoveWatch]);
pub struct WatchlistPanel {
    pub table: Entity<TableState<WatchlistDelegate>>,
    _highlight: Subscription,
    _market: Subscription,
}
impl WatchlistPanel {
    pub fn new(
        store: Entity<crate::market::MarketStore>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let table = cx.new(|cx| {
            let mut table = TableState::new(WatchlistDelegate::new(store.clone(), cx), window, cx)
                .col_selectable(false)
                .col_movable(false)
                .cell_selectable(false);
            if !store.read(cx).order.is_empty() {
                table.set_selected_row(0, cx);
            }
            table.delegate_mut().highlighted_row = table.selected_row();
            table
        });
        let highlight = cx.observe_in(&table, window, |_, table, _, cx| {
            let row = table.read(cx).selected_row();
            if table.read(cx).delegate().highlighted_row != row {
                table.update(cx, |table, cx| {
                    table.delegate_mut().highlighted_row = row;
                    table.refresh(cx);
                });
            }
        });
        let market = cx.observe_in(&store, window, |this, _, _, cx| {
            this.table.update(cx, |table, cx| table.refresh(cx))
        });
        Self {
            table,
            _market: market,
            _highlight: highlight,
        }
    }
}
impl Focusable for WatchlistPanel {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.table.focus_handle(cx)
    }
}
impl EventEmitter<PanelEvent> for WatchlistPanel {}
impl EventEmitter<RemoveSelected> for WatchlistPanel {}
impl BasePanel for WatchlistPanel {
    fn panel_name(&self) -> &'static str {
        "watchlist"
    }
    fn closable(&self, _: &App) -> bool {
        false
    }
}
impl Panel for WatchlistPanel {
    fn title(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        parts::panel_header("自選", cx)
    }
    fn dropdown_menu(
        &mut self,
        menu: gpui_kit::component::menu::PopupMenu,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) -> gpui_kit::component::menu::PopupMenu {
        let entity = cx.entity().downgrade();
        parts::watch_menu(
            menu,
            std::rc::Rc::new(move |_, _, cx| {
                let _ = entity.update(cx, |_, cx| cx.emit(RemoveSelected));
            }),
        )
    }
    fn tab_name(&self, _: &App) -> Option<SharedString> {
        Some("自選".into())
    }
}
impl Render for WatchlistPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        parts::pane("watchlist-panel")
            .key_context("Watchlist")
            .on_action(cx.listener(|_, _: &RemoveWatch, _, cx| cx.emit(RemoveSelected)))
            .on_prepaint({
                let table = self.table.clone();
                move |bounds, window, cx| {
                    let widths = space::watch_widths(bounds.size.width);
                    if table.read(cx).delegate().columns[0].width != widths[0] {
                        // TableState retains column geometry; refresh after layout settles.
                        let table = table.clone();
                        window.defer(cx, move |_, cx| {
                            table.update(cx, |table, cx| {
                                for (column, width) in
                                    table.delegate_mut().columns.iter_mut().zip(widths)
                                {
                                    column.width = width;
                                }
                                table.refresh(cx);
                                cx.notify();
                            })
                        });
                    }
                }
            })
            .child(parts::watch_table(&self.table, cx))
    }
}
