use crate::{
    design::{palette as palette_design, parts},
    market::*,
    palette::Palette,
    panels::{
        chart::ChartPanel,
        quote::QuotePanel,
        status_bar,
        watchlist::{RemoveSelected, RemoveWatch, WatchlistPanel},
    },
};
use futures::StreamExt;
use gpui_kit::{
    component::{
        WindowExt,
        dock::{DockArea, DockSkin, panel_handle},
    },
    *,
};
use std::{path::PathBuf, rc::Rc};
gpui_kit::actions!(workspace, [OpenPalette]);
pub struct Workspace {
    dock: Entity<DockArea>,
    watchlist: Entity<WatchlistPanel>,
    chart: Entity<ChartPanel>,
    quote: Entity<QuotePanel>,
    _skin: Rc<DockSkin>,
    store: Entity<MarketStore>,
    focus: FocusHandle,
    _market: Subscription,
    _remove: Subscription,
    _feed: Option<Task<()>>,
    _history: Option<Task<()>>,
    _freshness: Option<Task<()>>,
    _directory: Option<Task<()>>,
    provider: Option<MisProvider>,
    directory: Option<Result<Vec<Instrument>, String>>,
    palette: Option<Entity<Palette>>,
    path: Option<PathBuf>,
    live: bool,
    mock: bool,
    directory_loading: bool,
    directory_fetched_at: Option<i64>,
}
impl Workspace {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self::with_store(MockProvider::new().store(), window, cx)
    }
    pub fn configured(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let (path, loaded) = match watchlist_path() {
            Ok(path) => {
                let loaded = load_watchlist_at(&path);
                (Some(path), loaded)
            }
            Err(reason) => (None, Err(reason)),
        };
        let store = match loaded {
            Ok(None) => MockProvider::new().store(),
            Ok(Some(items)) => {
                let mut store = MarketStore::new(Capabilities::mis());
                for item in items {
                    store.add_instrument(item);
                }
                store
            }
            Err(reason) => {
                let mut store = MarketStore::new(Capabilities::mis());
                store.link = LinkState::Failed {
                    reason,
                    retry_in: 0,
                };
                store
            }
        };
        let mut this = Self::with_store(store, window, cx);
        this.path = path;
        this.live = true;
        this.load_directory(window, cx);
        this
    }
    /// Frozen services for embedders and deterministic tests; never starts network workers.
    pub fn with_services(
        store: MarketStore,
        directory: Result<Vec<Instrument>, String>,
        path: PathBuf,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut this = Self::with_store(store, window, cx);
        this.directory = Some(directory);
        this.path = Some(path);
        this
    }
    /// Display supplied data without starting providers or freshness tasks.
    pub fn with_store(store: MarketStore, window: &mut Window, cx: &mut Context<Self>) -> Self {
        cx.bind_keys([
            KeyBinding::new("cmd-k", OpenPalette, Some("WatchWorkspace")),
            KeyBinding::new("delete", RemoveWatch, Some("Watchlist")),
            KeyBinding::new("backspace", RemoveWatch, Some("Watchlist")),
        ]);
        let mock = store
            .snapshots()
            .any(|q| q.source.as_deref() == Some("Mock"));
        let store = cx.new(|_| store);
        let market = cx.observe(&store, |_, _, cx| cx.notify());
        let watchlist = cx.new(|cx| WatchlistPanel::new(store.clone(), window, cx));
        let table = watchlist.read(cx).table.clone();
        let chart = cx.new(|cx| ChartPanel::new(table.clone(), window, cx));
        let quote = cx.new(|cx| QuotePanel::new(table, window, cx));
        let (dock, skin) = parts::dock(window, cx);
        let remove = cx.subscribe_in(
            &watchlist,
            window,
            |this, _, _: &RemoveSelected, window, cx| {
                if let Err(error) = this.remove_selected(cx) {
                    this.show_error(error, window, cx);
                }
            },
        );
        let this = Self {
            dock,
            watchlist,
            chart,
            quote,
            _skin: skin,
            store,
            focus: cx.focus_handle(),
            _market: market,
            _remove: remove,
            _feed: None,
            _history: None,
            _freshness: None,
            _directory: None,
            provider: None,
            directory: None,
            palette: None,
            path: None,
            live: false,
            mock,
            directory_loading: false,
            directory_fetched_at: None,
        };
        this.reset_layout(window, cx);
        window.focus(&this.focus, cx);
        this
    }
    fn is_mock(&self, _: &App) -> bool {
        self.mock
    }
    fn load_directory(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.directory_loading = true;
        self.directory = None;
        let (tx, rx) = futures::channel::oneshot::channel();
        std::thread::spawn(move || {
            let (result, fetched_at) = match IsinDirectory::new() {
                Ok(mut source) => {
                    let result = source.instruments();
                    (result, source.fetched_at())
                }
                Err(e) => (Err(e), None),
            };
            let _ = tx.send((result, fetched_at));
        });
        self._directory = Some(cx.spawn_in(window, async move |this, cx| {
            let (result, fetched_at) = rx.await.unwrap_or_else(|e| (Err(e.to_string()), None));
            let _ = this.update_in(cx, |this, window, cx| {
                this.directory_loading = false;
                this.directory_fetched_at = fetched_at;
                this.directory = Some(result.clone());
                if let Some(palette) = &this.palette {
                    palette.update(cx, |palette, cx| {
                        palette.set_directory(result.clone(), window, cx)
                    });
                }
                match result {
                    Ok(items) => {
                        let symbols = this.store.read(cx).order.clone();
                        let missing = symbols
                            .iter()
                            .find(|symbol| !items.iter().any(|i| &i.symbol == *symbol))
                            .cloned();
                        if !this.is_mock(cx) {
                            if let Some(symbol) = missing {
                                this.store.update(cx, |store, cx| {
                                    store.link = LinkState::Failed {
                                        reason: format!(
                                            "商品 {} 不在支援的股票／ETF 清單",
                                            symbol.code
                                        ),
                                        retry_in: 0,
                                    };
                                    cx.notify();
                                });
                                return;
                            }
                            this.store.update(cx, |store, cx| {
                                for item in items {
                                    if store.instruments.contains_key(&item.symbol) {
                                        store.add_instrument(item);
                                    }
                                }
                                cx.notify();
                            });
                            this.start_live(cx);
                        }
                    }
                    Err(reason) => {
                        if !this.is_mock(cx) {
                            this.store.update(cx, |store, cx| {
                                store.link = LinkState::Failed {
                                    reason: format!("商品清單失敗：{reason}"),
                                    retry_in: 0,
                                };
                                cx.notify();
                            });
                        }
                    }
                }
            });
        }));
    }
    fn open_palette(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.live
            && !self.directory_loading
            && !directory_fresh(self.directory_fetched_at, now())
        {
            self.load_directory(window, cx);
        }
        let weak = cx.entity().downgrade();
        let add = Rc::new(move |item, window: &mut Window, cx: &mut App| {
            weak.update(cx, |this, cx| this.add(item, window, cx))
                .unwrap_or_else(|e| Err(e.to_string()))
        });
        let watching = self.store.read(cx).order.clone();
        let mock = self.is_mock(cx);
        let directory = self.directory.clone();
        let palette = cx.new(|cx| Palette::new(directory, watching, mock, add, window, cx));
        self.palette = Some(palette.clone());
        window.open_dialog(cx, move |dialog, _, cx| {
            palette_design::dialog(dialog, palette.clone(), cx)
        });
        let palette = self.palette.as_ref().unwrap().clone();
        window.defer(cx, move |window, cx| {
            palette.update(cx, |p, cx| p.focus(window, cx))
        });
    }
    fn show_error(&mut self, error: String, window: &mut Window, cx: &mut Context<Self>) {
        let palette = cx.new(|cx| {
            Palette::new(
                Some(Err(error)),
                self.store.read(cx).order.clone(),
                false,
                Rc::new(|_, _, _| Err("無法加入".into())),
                window,
                cx,
            )
        });
        window.open_dialog(cx, move |dialog, _, cx| {
            palette_design::dialog(dialog, palette.clone(), cx)
        });
    }
    fn add(
        &mut self,
        item: Instrument,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        if self.store.read(cx).instruments.contains_key(&item.symbol) {
            return Err("已在自選，不重複加入".into());
        }
        let max = if self.is_mock(cx) {
            Capabilities::mis().max_subscriptions
        } else {
            self.store.read(cx).capabilities.max_subscriptions
        };
        if !self.is_mock(cx) && self.store.read(cx).order.len() >= max {
            return Err(format!("已達來源訂閱上限（{max} 檔），請先移除商品"));
        }
        let path = self.path.clone().ok_or("尚未設定自選檔案路徑")?;
        let entries = save_watchlist(&path, WatchlistChange::Add(item), max)?;
        if self.is_mock(cx) {
            self.store.update(cx, |store, _| {
                *store = MarketStore::new(Capabilities::mis())
            });
        }
        self.mock = false;
        self.apply_watchlist(entries, cx);
        Ok(())
    }
    fn remove_selected(&mut self, cx: &mut Context<Self>) -> Result<(), String> {
        let table = self.watchlist.read(cx).table.clone();
        let row = table.read(cx).selected_row().ok_or("請先選取自選列")?;
        let symbol = self
            .store
            .read(cx)
            .order
            .get(row)
            .cloned()
            .ok_or("請先選取自選列")?;
        if self.is_mock(cx) {
            self.store.update(cx, |store, cx| {
                store.remove_instrument(&symbol);
                cx.notify();
            });
        } else {
            let path = self.path.clone().ok_or("尚未設定自選檔案路徑")?;
            let entries = save_watchlist(
                &path,
                WatchlistChange::Remove(symbol),
                Capabilities::mis().max_subscriptions,
            )?;
            self.apply_watchlist(entries, cx);
        }
        let count = self.store.read(cx).order.len();
        table.update(cx, |table, cx| {
            if count == 0 {
                table.clear_selection(cx);
            } else {
                table.set_selected_row(row.min(count - 1), cx);
            }
            cx.notify();
        });
        Ok(())
    }
    fn apply_watchlist(&mut self, items: Vec<Instrument>, cx: &mut Context<Self>) {
        let directory = self.directory.as_ref().and_then(|r| r.as_ref().ok());
        self.store.update(cx, |store, cx| {
            for symbol in store.order.clone() {
                if !items.iter().any(|i| i.symbol == symbol) {
                    store.remove_instrument(&symbol);
                }
            }
            for item in items {
                let item = directory
                    .and_then(|d| d.iter().find(|i| i.symbol == item.symbol))
                    .cloned()
                    .unwrap_or(item);
                store.add_instrument(item);
            }
            cx.notify();
        });
        let table = self.watchlist.read(cx).table.clone();
        if table
            .read(cx)
            .selected_row()
            .is_none_or(|row| row >= self.store.read(cx).order.len())
            && !self.store.read(cx).order.is_empty()
        {
            table.update(cx, |t, cx| t.set_selected_row(0, cx));
        }
        if self.live {
            self.start_live(cx);
        }
    }
    fn start_live(&mut self, cx: &mut Context<Self>) {
        if !self.live {
            return;
        }
        let symbols = self.store.read(cx).subscriptions(&[]);
        if self.store.read(cx).order.len() > Capabilities::mis().max_subscriptions {
            self.store.update(cx, |s, cx| {
                s.link = LinkState::Failed {
                    reason: "自選超過來源訂閱上限".into(),
                    retry_in: 0,
                };
                cx.notify();
            });
            return;
        }
        if let Some(provider) = &self.provider {
            provider.resubscribe(symbols.clone());
        } else if !symbols.is_empty() {
            match MisProvider::new() {
                Ok(mut provider) => {
                    let mut events = provider.subscribe(symbols.clone());
                    self.provider = Some(provider);
                    self._feed = Some(cx.spawn(async move |this, cx| {
                        while let Some(event) = events.next().await {
                            if this
                                .update(cx, |this, cx| {
                                    this.store.update(cx, |store, cx| {
                                        if let MarketEvent::Quote(q) = &event
                                            && !store.instruments.contains_key(&q.symbol)
                                        {
                                            return;
                                        }
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
                Err(reason) => self.store.update(cx, |store, cx| {
                    store.link = LinkState::Failed {
                        reason,
                        retry_in: 0,
                    };
                    cx.notify();
                }),
            }
        }
        if symbols.is_empty() {
            self.store.update(cx, |s, cx| {
                s.link = LinkState::Live;
                cx.notify();
            });
        }
        let missing = symbols
            .into_iter()
            .filter(|s| !self.store.read(cx).histories.contains_key(s))
            .collect::<Vec<_>>();
        if !missing.is_empty() {
            let (tx, mut rx) = futures::channel::mpsc::unbounded();
            std::thread::spawn(move || {
                let mut provider = HistoryProvider::new();
                for symbol in missing {
                    if tx.is_closed() {
                        break;
                    }
                    let result = match &mut provider {
                        Ok(p) => p.recent(&symbol, 100, now()),
                        Err(e) => Err(e.clone()),
                    };
                    if tx.unbounded_send((symbol, result)).is_err() {
                        break;
                    }
                    std::thread::sleep(std::time::Duration::from_secs(2));
                }
            });
            self._history = Some(cx.spawn(async move |this, cx| {
                while let Some((symbol, rows)) = rx.next().await {
                    if this
                        .update(cx, |this, cx| {
                            this.store.update(cx, |store, cx| {
                                if store.instruments.contains_key(&symbol) {
                                    store.set_history(symbol, rows);
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
        if self._freshness.is_none() {
            self._freshness = Some(cx.spawn(async move |this, cx| {
                loop {
                    cx.background_executor()
                        .timer(std::time::Duration::from_secs(1))
                        .await;
                    if this
                        .update(cx, |this, cx| {
                            this.store.update(cx, |store, cx| {
                                if store.tick(now()) {
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
            .track_focus(&self.focus)
            .key_context("WatchWorkspace")
            .on_action(cx.listener(|this, _: &OpenPalette, w, cx| this.open_palette(w, cx)))
            .child(
                parts::title_bar(cx)
                    .child(
                        palette_design::search_button()
                            .on_click(cx.listener(|this, _, w, cx| this.open_palette(w, cx))),
                    )
                    .child(
                        parts::watch_button().on_click(cx.listener(|this, _, window, cx| {
                            let panel = panel_handle(this.watchlist.clone()).panel_id(cx);
                            this.dock
                                .update(cx, |dock, cx| dock.select_panel(panel, window, cx));
                            window.focus(&this.watchlist.focus_handle(cx), cx);
                        })),
                    )
                    .child(
                        parts::reset_button()
                            .on_click(cx.listener(|this, _, w, cx| this.reset_layout(w, cx))),
                    ),
            )
            .child(parts::workspace_body().child(self.dock.clone()))
            .child(status_bar::render(self.store.read(cx), cx))
    }
}
