//! Shared, neutral panel states. The parent supplies the available region.
use super::{
    color::{self, ColorRole},
    space::Space,
    text::{self, TextRole},
};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{
    base::ObservedElement,
    component::{Sizable, spinner::Spinner, v_flex},
    *,
};

#[derive(Clone)]
pub enum PanelState {
    Loading(SharedString),
    Failed(SharedString),
    Empty(SharedString),
    InsufficientSpace,
}
impl PanelState {
    pub fn label(&self) -> SharedString {
        match self {
            Self::Loading(s) | Self::Empty(s) => s.clone(),
            Self::Failed(reason) => format!("載入失敗：{reason}").into(),
            Self::InsufficientSpace => "圖表空間不足".into(),
        }
    }
}
pub fn content(state: PanelState, cx: &App) -> ObservedElement<Stateful<Div>> {
    let label = state.label();
    v_flex()
        .id("panel-state-content")
        .test_support()
        .items_center()
        .gap(Space::Small.rem())
        .when(matches!(state, PanelState::Loading(_)), |el| {
            el.child(
                Spinner::new()
                    .small()
                    .color(color::get(ColorRole::Muted, cx)),
            )
        })
        .child(
            text::text(TextRole::Caption, label.clone(), cx)
                .id("panel-state-text")
                .aria_label(label)
                .test_support()
                .text_center(),
        )
}
pub fn panel(state: PanelState, cx: &App) -> Div {
    v_flex()
        .size_full()
        .flex_1()
        .min_w_0()
        .min_h_0()
        .items_center()
        .justify_center()
        .child(content(state, cx))
}
