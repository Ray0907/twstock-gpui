use crate::{design::palette as design, market::*};
use gpui_kit::{
    component::{
        IndexPath, WindowExt,
        list::{ListDelegate, ListEvent, ListItem, ListState},
    },
    *,
};
use std::rc::Rc;
pub type AddHandler = Rc<dyn Fn(Instrument, &mut Window, &mut App) -> Result<(), String>>;
pub struct PaletteDelegate {
    items: Vec<Instrument>,
    matches: Vec<Instrument>,
    query: String,
    pub watching: Vec<Symbol>,
    pub status: Option<crate::design::state::PanelState>,
}
impl PaletteDelegate {
    fn search(&mut self, query: &str) {
        self.query = query.into();
        self.matches = search_directory(&self.items, query)
            .into_iter()
            .cloned()
            .collect();
    }
}
impl ListDelegate for PaletteDelegate {
    type Item = ListItem;
    fn items_count(&self, _: usize, _: &App) -> usize {
        self.matches.len()
    }
    fn render_item(
        &mut self,
        ix: IndexPath,
        _: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) -> Option<ListItem> {
        let item = self.matches.get(ix.row)?;
        Some(design::item(item, self.watching.contains(&item.symbol), cx))
    }
    fn set_selected_index(
        &mut self,
        _: Option<IndexPath>,
        _: &mut Window,
        _: &mut Context<ListState<Self>>,
    ) {
    }
    fn perform_search(
        &mut self,
        query: &str,
        _: &mut Window,
        _: &mut Context<ListState<Self>>,
    ) -> Task<()> {
        self.search(query);
        Task::ready(())
    }
    fn render_empty(
        &mut self,
        _: &mut Window,
        cx: &mut Context<ListState<Self>>,
    ) -> impl IntoElement {
        design::empty(self.status.clone(), cx)
    }
}
pub struct Palette {
    pub list: Entity<ListState<PaletteDelegate>>,
    message: String,
    mock: bool,
    on_add: AddHandler,
    _observe: Subscription,
    _events: Subscription,
}
impl Palette {
    pub fn new(
        directory: Option<Result<Vec<Instrument>, String>>,
        watching: Vec<Symbol>,
        mock: bool,
        on_add: AddHandler,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let status = match &directory {
            None => Some(crate::design::state::PanelState::Loading(
                "商品清單載入中".into(),
            )),
            Some(Err(reason)) => Some(crate::design::state::PanelState::Failed(
                format!("商品清單：{reason}").into(),
            )),
            Some(Ok(_)) => None,
        };
        let (items, message) = match directory {
            Some(Ok(items)) => (
                items,
                if mock {
                    "目前為範例模式：加入第一檔會建立設定檔並切換至真實 MIS 行情".into()
                } else {
                    "Enter 加入 · Esc 關閉".into()
                },
            ),
            Some(Err(e)) => (vec![], format!("商品清單失敗：{e}")),
            None => (vec![], "商品清單載入中…".into()),
        };
        let list = cx.new(|cx| {
            let mut delegate = PaletteDelegate {
                items,
                matches: vec![],
                query: String::new(),
                watching,
                status,
            };
            delegate.search("");
            ListState::new(delegate, window, cx).searchable(true)
        });
        list.update(cx, |list, cx| {
            list.set_selected_index(Some(IndexPath::new(0)), window, cx)
        });
        let observe = cx.observe(&list, |_, _, cx| cx.notify());
        let events = cx.subscribe_in(&list, window, |this, _, event, window, cx| match event {
            ListEvent::Cancel => window.close_dialog(cx),
            ListEvent::Confirm(ix) => {
                let item = this.list.read(cx).delegate().matches.get(ix.row).cloned();
                if let Some(item) = item {
                    let symbol = item.symbol.clone();
                    match (this.on_add)(item, window, cx) {
                        Ok(()) => {
                            this.message = if this.mock {
                                "已建立設定檔並切換為真實 MIS 行情".into()
                            } else {
                                "已加入自選".into()
                            };
                            let was_mock = this.mock;
                            this.mock = false;
                            this.list.update(cx, |list, cx| {
                                if was_mock {
                                    list.delegate_mut().watching.clear();
                                }
                                if !list.delegate().watching.contains(&symbol) {
                                    list.delegate_mut().watching.push(symbol);
                                }
                                cx.notify();
                            });
                        }
                        Err(e) => this.message = e,
                    }
                    cx.notify();
                }
            }
            _ => {}
        });
        Self {
            list,
            message,
            on_add,
            mock,
            _observe: observe,
            _events: events,
        }
    }
    pub fn set_directory(
        &mut self,
        result: Result<Vec<Instrument>, String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match result {
            Ok(items) => {
                self.message = if self.mock {
                    "目前為範例模式：加入第一檔會建立設定檔並切換至真實 MIS 行情".into()
                } else {
                    "Enter 加入 · Esc 關閉".into()
                };
                self.list.update(cx, |list, cx| {
                    let query = list.delegate().query.clone();
                    list.delegate_mut().items = items;
                    list.delegate_mut().status = None;
                    list.delegate_mut().search(&query);
                    list.set_selected_index(Some(IndexPath::new(0)), window, cx);
                    cx.notify();
                });
            }
            Err(e) => {
                self.message = format!("商品清單失敗：{e}");
                self.list.update(cx, |list, cx| {
                    list.delegate_mut().status = Some(crate::design::state::PanelState::Failed(
                        format!("商品清單：{e}").into(),
                    ));
                    cx.notify();
                });
            }
        };
        cx.notify();
    }
    pub fn focus(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.list.update(cx, |list, cx| list.focus(window, cx));
    }
}
impl Render for Palette {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        design::body(&self.list, self.message.clone(), cx)
    }
}
