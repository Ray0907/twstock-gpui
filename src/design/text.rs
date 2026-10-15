use super::color::{self, ColorRole};
use gpui_kit::component::ActiveTheme;
use gpui_kit::*;
use std::sync::Arc;

pub(super) const BASE_FONT_SIZE: f32 = 13.;
#[derive(Clone, Copy)]
pub enum TextRole {
    Hero,
    Title,
    Value,
    Body,
    Label,
    Caption,
}
pub(super) fn features() -> FontFeatures {
    FontFeatures(Arc::new(vec![("tnum".into(), 1)]))
}
fn settings(role: TextRole) -> (f32, FontWeight, ColorRole, bool) {
    match role {
        TextRole::Hero => (1.875, FontWeight::SEMIBOLD, ColorRole::Foreground, true),
        TextRole::Title => (1., FontWeight::MEDIUM, ColorRole::Foreground, false),
        TextRole::Value => (1., FontWeight::MEDIUM, ColorRole::Foreground, true),
        TextRole::Body => (0.875, FontWeight::NORMAL, ColorRole::Foreground, true),
        TextRole::Label => (0.75, FontWeight::NORMAL, ColorRole::Muted, false),
        TextRole::Caption => (0.75, FontWeight::NORMAL, ColorRole::Muted, true),
    }
}
pub(super) fn pixels(role: TextRole, font: Pixels) -> Pixels {
    rems(settings(role).0).to_pixels(font)
}
pub(super) fn frame(role: TextRole, cx: &App) -> Div {
    let (size, weight, tone, tabular) = settings(role);
    div()
        .text_size(rems(size))
        .line_height(rems(size * 1.25))
        .font_weight(weight)
        .font_family(cx.theme().font_family.clone())
        .font_features(if tabular {
            features()
        } else {
            FontFeatures::default()
        })
        .text_color(color::get(tone, cx))
}
pub fn text(role: TextRole, content: impl Into<SharedString>, cx: &App) -> Div {
    frame(role, cx).child(content.into())
}
#[cfg(test)]
mod tests {
    use super::{TextRole, text};
    use gpui_kit::component::{ActiveTheme, ThemeMode};
    use gpui_kit::{FontWeight, Styled, TestAppContext, px};
    #[gpui_kit::test]
    fn price_roles_are_neutral_tabular_interface_text(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_kit::init(cx);
            for mode in [ThemeMode::Dark, ThemeMode::Light] {
                crate::theme::apply(mode, None, cx);
                for (role, weight) in [
                    (TextRole::Hero, FontWeight::SEMIBOLD),
                    (TextRole::Value, FontWeight::MEDIUM),
                    (TextRole::Body, FontWeight::NORMAL),
                ] {
                    let mut element = text(role, "124.00", cx);
                    let style = &element.style().text;
                    assert_eq!(style.color, Some(cx.theme().foreground));
                    assert_eq!(style.font_family, Some(cx.theme().font_family.clone()));
                    assert_eq!(
                        style.font_features.as_ref().unwrap().tag_value_list(),
                        &[("tnum".into(), 1)]
                    );
                    assert_eq!(style.font_weight, Some(weight));
                }
                assert_eq!(cx.theme().font_size, px(13.));
            }
        });
    }
}
