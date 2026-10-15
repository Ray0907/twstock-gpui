use super::{
    parts,
    space::Space,
    text::{self, TextRole},
};
use crate::{
    market::Instrument,
    palette::{Palette, PaletteDelegate},
};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{
    component::{
        Sizable,
        button::Button,
        dialog::Dialog,
        list::{List, ListItem, ListState},
    },
    *,
};
pub fn item(item: &Instrument, watched: bool, cx: &App) -> ListItem {
    let code = item.symbol.code.clone();
    ListItem::new(format!("palette-result-{code}"))
        .accessibility_label(format!(
            "{} {} {}{}",
            code,
            item.name,
            item.symbol.market.label(),
            if watched { " 已在自選" } else { "" }
        ))
        .child(
            component::h_flex()
                .id(format!("palette-row-{code}"))
                .test_support()
                .w_full()
                .min_w_0()
                .items_center()
                .gap(Space::Small.rem())
                .child(text::text(TextRole::Value, code.clone(), cx))
                .child(
                    text::text(TextRole::Body, item.name.clone(), cx)
                        .flex_1()
                        .truncate(),
                )
                .child(parts::panel_header(item.symbol.market.label(), cx))
                .when(watched, |this| {
                    this.child(
                        parts::panel_header("已在自選", cx)
                            .id(format!("palette-watched-{code}"))
                            .test_support(),
                    )
                }),
        )
}
pub fn empty(status: Option<super::state::PanelState>, cx: &App) -> impl IntoElement {
    let state =
        status.unwrap_or_else(|| super::state::PanelState::Empty("沒有符合的股票或 ETF".into()));
    let label = state.label();
    super::state::panel(state, cx)
        .id("palette-message")
        .aria_label(label)
        .test_support()
}
pub fn body(
    list: &Entity<ListState<PaletteDelegate>>,
    message: String,
    cx: &App,
) -> impl IntoElement {
    component::v_flex()
        .id("directory-palette")
        .test_support()
        .gap(Space::Small.rem())
        .child(
            List::new(list)
                .search_placeholder("輸入代號前綴或名稱")
                .h(rems(23.)),
        )
        .when(list.read(cx).delegate().status.is_none(), |el| {
            el.child(
                text::text(TextRole::Caption, message.clone(), cx)
                    .id("palette-message")
                    .aria_label(message)
                    .test_support(),
            )
        })
}
pub fn dialog(dialog: Dialog, palette: Entity<Palette>, cx: &App) -> Dialog {
    dialog
        .title(text::text(TextRole::Title, "搜尋並加入自選", cx))
        .width(px(600.))
        .child(palette)
}
pub fn search_button() -> Button {
    Button::new("open-palette").label("搜尋商品 ⌘K").small()
}
