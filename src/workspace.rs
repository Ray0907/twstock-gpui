use crate::{
    design::parts,
    panels::{chart::ChartPanel, quote::QuotePanel, status_bar, watchlist::WatchlistPanel},
};
use gpui_kit::component::dock::{DockArea, DockSkin, panel_handle};
use gpui_kit::*;
use std::rc::Rc;

pub struct Workspace {
    dock: Entity<DockArea>,
    watchlist: Entity<WatchlistPanel>,
    chart: Entity<ChartPanel>,
    quote: Entity<QuotePanel>,
    _skin: Rc<DockSkin>,
    store: Entity<crate::market::MarketStore>,
    _market: Subscription,
    _feed: Option<Task<()>>,
    _history: Option<Task<()>>,
    _freshness: Option<Task<()>>,
}
impl Workspace {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self::with_store(crate::market::MockProvider::new().store(), window, cx)
    }
    pub fn configured(window: &mut Window, cx: &mut Context<Self>) -> Self {
        use crate::market::*;
        use futures::StreamExt;
        let instruments = match load_watchlist() {
            Ok(None) => return Self::new(window, cx),
            Ok(Some(instruments)) => instruments,
            Err(reason) => {
                let mut store = MarketStore::new(Capabilities::mis());
                store.link = LinkState::Failed {
                    reason,
                    retry_in: 0,
                };
                return Self::with_store(store, window, cx);
            }
        };
        let mut store = MarketStore::new(Capabilities::mis());
        for instrument in instruments {
            store.add_instrument(instrument);
        }
        let symbols = store.subscriptions(&[]);
        let mut workspace = Self::with_store(store, window, cx);
        match MisProvider::new() {
            Ok(mut provider) => {
                let mut events = provider.subscribe(symbols.clone());
                workspace._feed = Some(cx.spawn(async move |this, cx| {
                    while let Some(event) = events.next().await {
                        if this
                            .update(cx, |this, cx| {
                                this.store.update(cx, |store, cx| {
                                    if store.apply(event, now()) {
                                        cx.notify();
                                    }
                                })
                            })
                            .is_err()
                        {
                            break;
                        }
                    }
                }));
            }
            Err(reason) => workspace.store.update(cx, |store, cx| {
                store.link = LinkState::Failed {
                    reason,
                    retry_in: 0,
                };
                cx.notify();
            }),
        }
        let (sender, mut receiver) = futures::channel::mpsc::unbounded();
        std::thread::spawn(move || {
            let mut provider = HistoryProvider::new();
            for symbol in symbols {
                if sender.is_closed() {
                    break;
                }
                let rows = match &mut provider {
                    Ok(p) => p.recent(&symbol, 100, now()),
                    Err(e) => Err(e.clone()),
                };
                if sender.unbounded_send((symbol, rows)).is_err() {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_secs(2));
            }
        });
        workspace._history = Some(cx.spawn(async move |this, cx| {
            while let Some((symbol, rows)) = receiver.next().await {
                if this
                    .update(cx, |this, cx| {
                        this.store.update(cx, |store, cx| {
                            store.set_history(symbol, rows);
                            cx.notify();
                        })
                    })
                    .is_err()
                {
                    break;
                }
            }
        }));
        workspace._freshness = Some(cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(std::time::Duration::from_secs(1))
                    .await;
                if this
                    .update(cx, |this, cx| {
                        this.store.update(cx, |store, cx| {
                            if store.tick(now()) || matches!(store.link, LinkState::Stale { .. }) {
                                cx.notify();
                            }
                        })
                    })
                    .is_err()
                {
                    break;
                }
            }
        }));
        workspace
    }
    /// Display supplied data without starting providers or freshness tasks.
    pub fn with_store(
        store: crate::market::MarketStore,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let store = cx.new(|_| store);
        let market = cx.observe(&store, |_, _, cx| cx.notify());
        let watchlist = cx.new(|cx| WatchlistPanel::new(store.clone(), window, cx));
        let table = watchlist.read(cx).table.clone();
        let chart = cx.new(|cx| ChartPanel::new(table.clone(), window, cx));
        let quote = cx.new(|cx| QuotePanel::new(table, window, cx));
        let (dock, skin) = parts::dock(window, cx);
        let workspace = Self {
            dock,
            watchlist,
            chart,
            quote,
            _skin: skin,
            store,
            _market: market,
            _feed: None,
            _history: None,
            _freshness: None,
        };
        workspace.reset_layout(window, cx);
        workspace
    }
    fn reset_layout(&self, window: &mut Window, cx: &mut Context<Self>) {
        let layout = parts::default_dock_layout(
            [
                panel_handle(self.watchlist.clone()),
                panel_handle(self.chart.clone()),
                panel_handle(self.quote.clone()),
            ],
            window,
            cx,
        );
        self.dock
            .update(cx, |dock, cx| dock.set_center(layout, window, cx));
    }
}
impl Render for Workspace {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        parts::workspace(cx)
            .child(
                parts::title_bar(cx)
                    .child(
                        parts::watch_button().on_click(cx.listener(|this, _, window, cx| {
                            let panel = panel_handle(this.watchlist.clone()).panel_id(cx);
                            this.dock
                                .update(cx, |dock, cx| dock.select_panel(panel, window, cx));
                            window.focus(&this.watchlist.focus_handle(cx), cx);
                        })),
                    )
                    .child(parts::reset_button().on_click(
                        cx.listener(|this, _, window, cx| this.reset_layout(window, cx)),
                    )),
            )
            .child(parts::workspace_body().child(self.dock.clone()))
            .child(status_bar::render(self.store.read(cx), cx))
    }
}
