//! 看盤配色：冷灰中性色階、金色強調色，紅漲綠跌保留給方向。
//!
//! 這是全專案唯一允許寫死色碼的地方，元件一律讀 `cx.theme()`。
//! 每組配對的對比值都用最終 hex 量過：內文 4.5 以上，次要文字 4.5 以上。

use gpui_kit::component::{Theme, ThemeColor, ThemeMode};
use gpui_kit::*;

pub fn series_hex(index: usize, dark: bool) -> u32 {
    let (l, c, h): (f64, f64, f64) = match (index, dark) {
        (0, true) => (0.85, 0.12, 110.),
        (0, false) => (0.46, 0.08, 110.),
        (1, true) => (0.78, 0.10, 300.),
        (1, false) => (0.46, 0.16, 300.),
        (2, true) => (0.80, 0.09, 210.),
        (2, false) => (0.42, 0.07, 210.),
        _ => panic!("unknown MA series"),
    };
    let a = c * h.to_radians().cos();
    let b = c * h.to_radians().sin();
    let x = (l + 0.3963377774 * a + 0.2158037573 * b).powi(3);
    let y = (l - 0.1055613458 * a - 0.0638541728 * b).powi(3);
    let z = (l - 0.0894841775 * a - 1.2914855480 * b).powi(3);
    let channel = |v: f64| -> u32 {
        let v = if v <= 0.0031308 {
            12.92 * v
        } else {
            1.055 * v.powf(1. / 2.4) - 0.055
        };
        (v.clamp(0., 1.) * 255.).round() as u32
    };
    channel(4.0767416621 * x - 3.3077115913 * y + 0.2309699292 * z) << 16
        | channel(-1.2684380046 * x + 2.6097574011 * y - 0.3413193965 * z) << 8
        | channel(-0.0041960863 * x - 0.7034186147 * y + 1.7076147010 * z)
}
struct Palette {
    background: u32,
    panel: u32,
    raised: u32,
    hover: u32,
    border: u32,
    foreground: u32,
    secondary_text: u32,
    muted_text: u32,
    gold_text: u32,
    gold_solid: u32,
    gold_hover: u32,
    gold_active: u32,
    on_gold: u32,
    up: u32,
    down: u32,
    /// 選取列的金色底，疊在 background 上。
    selection_fill: f32,
}

const DARK: Palette = Palette {
    background: 0x0b0f14,
    panel: 0x11151a,
    raised: 0x1a1e24,
    hover: 0x20242a,
    border: 0x2a2e34,
    foreground: 0xe8ebef,
    secondary_text: 0xa7abb1,
    muted_text: 0x888d92,
    gold_text: 0xe8bf51,
    gold_solid: 0xddb13a,
    gold_hover: 0xf0c551,
    gold_active: 0xc69f32,
    on_gold: 0x10171f,
    up: 0xff645f,
    down: 0x4fce74,
    selection_fill: 0.14,
};

const LIGHT: Palette = Palette {
    background: 0xf8fafd,
    panel: 0xf1f4f7,
    raised: 0xe8ebef,
    hover: 0xe1e5e9,
    border: 0xd4d8dd,
    foreground: 0x11161d,
    secondary_text: 0x494d54,
    muted_text: 0x5f646a,
    gold_text: 0x7d5e00,
    gold_solid: 0xddb13a,
    gold_hover: 0xe8bf51,
    gold_active: 0xc69f32,
    on_gold: 0x10171f,
    up: 0xcc272e,
    down: 0x007f35,
    selection_fill: 0.16,
};

#[derive(Clone, Copy)]
pub enum ColorRole {
    Background,
    Foreground,
    Muted,
    Reference,
    Border,
    Focus,
    FocusFill,
    FocusInk,
    LastPriceLine,
    DepthUp,
    DepthDown,
    Up,
    Down,
}
pub fn get(role: ColorRole, cx: &App) -> Hsla {
    use gpui_kit::component::ActiveTheme;
    match role {
        ColorRole::Background => cx.theme().background,
        ColorRole::Foreground => cx.theme().foreground,
        ColorRole::Muted | ColorRole::Reference => cx.theme().muted_foreground,
        ColorRole::Border => cx.theme().border,
        ColorRole::Focus => cx.theme().link,
        ColorRole::FocusFill => cx.theme().primary,
        ColorRole::FocusInk => cx.theme().primary_foreground,
        ColorRole::LastPriceLine => cx.theme().link.opacity(0.28),
        ColorRole::DepthUp => cx.theme().red.opacity(0.28),
        ColorRole::DepthDown => cx.theme().green.opacity(0.28),
        ColorRole::Up => cx.theme().red,
        ColorRole::Down => cx.theme().green,
    }
}

fn color(hex: u32) -> Hsla {
    rgb(hex).into()
}

fn paint(colors: &mut ThemeColor, p: &Palette) {
    colors.background = color(p.background);
    colors.foreground = color(p.foreground);
    colors.border = color(p.border);
    colors.input = color(p.border);
    colors.caret = color(p.gold_text);
    colors.ring = color(p.gold_solid);
    colors.link = color(p.gold_text);
    colors.link_hover = color(p.gold_hover);
    colors.link_active = color(p.gold_active);
    colors.selection = color(p.gold_solid).opacity(0.3);

    colors.muted = color(p.raised);
    colors.muted_foreground = color(p.muted_text);
    colors.secondary = color(p.raised);
    colors.secondary_hover = color(p.hover);
    colors.secondary_active = color(p.hover);
    colors.secondary_foreground = color(p.foreground);
    colors.accent = color(p.hover);
    colors.accent_foreground = color(p.foreground);
    colors.popover = color(p.raised);
    colors.popover_foreground = color(p.foreground);
    colors.group_box = color(p.panel);
    colors.group_box_foreground = color(p.secondary_text);

    colors.primary = color(p.gold_solid);
    colors.primary_hover = color(p.gold_hover);
    colors.primary_active = color(p.gold_active);
    colors.primary_foreground = color(p.on_gold);
    colors.button_primary = color(p.gold_solid);
    colors.button_primary_hover = color(p.gold_hover);
    colors.button_primary_active = color(p.gold_active);
    colors.button_primary_foreground = color(p.on_gold);

    colors.red = color(p.up);
    colors.green = color(p.down);
    colors.chart_bullish = color(p.up);
    colors.chart_bearish = color(p.down);
    colors.chart_grid = color(p.border);

    let selected = color(p.gold_solid).opacity(p.selection_fill);
    colors.list = color(p.background);
    colors.list_even = color(p.panel);
    colors.list_head = color(p.panel);
    colors.list_hover = color(p.hover);
    colors.list_active = selected;
    colors.list_active_border = color(p.gold_solid).opacity(0.5);
    colors.table = color(p.background);
    colors.table_even = color(p.panel);
    colors.table_head = color(p.panel);
    colors.table_head_foreground = color(p.secondary_text);
    colors.table_hover = color(p.hover);
    colors.table_active = selected;
    colors.table_active_border = color(p.gold_solid).opacity(0.5);
    colors.table_row_border = color(p.border).opacity(0.6);

    colors.tab = color(p.background);
    colors.tab_active = color(p.raised);
    colors.tab_foreground = color(p.secondary_text);
    colors.tab_active_foreground = color(p.gold_text);
    colors.tab_bar = color(p.panel);
    colors.tab_bar_segmented = color(p.panel);

    colors.sidebar = color(p.panel);
    colors.sidebar_foreground = color(p.secondary_text);
    colors.sidebar_border = color(p.border);
    colors.sidebar_accent = color(p.hover);
    colors.sidebar_accent_foreground = color(p.foreground);
    colors.sidebar_primary = color(p.gold_solid);
    colors.sidebar_primary_foreground = color(p.on_gold);
    colors.title_bar = color(p.panel);
    colors.title_bar_border = color(p.border);
    colors.status_bar = color(p.panel);
    colors.status_bar_border = color(p.border);
    colors.scrollbar_thumb = color(p.border);
    colors.scrollbar_thumb_hover = color(p.muted_text);
}

/// 切換主題模式，再套上看盤配色與小圓角。
pub fn apply(mode: ThemeMode, window: Option<&mut Window>, cx: &mut App) {
    Theme::change(mode, window, cx);
    Theme::update(cx, |theme| {
        theme.font_size = px(super::text::BASE_FONT_SIZE);
        theme.radius = super::shape::Shape::Small.pixels(theme.font_size);
        theme.radius_lg = super::shape::Shape::Large.pixels(theme.font_size);
        paint(
            &mut theme.colors,
            if theme.mode.is_dark() { &DARK } else { &LIGHT },
        );
    });
}

#[cfg(test)]
mod tests {
    use super::{DARK, LIGHT};
    fn luminance(hex: u32) -> f64 {
        let channels = [hex >> 16, hex >> 8, hex].map(|value| {
            let c = (value & 255) as f64 / 255.;
            if c <= 0.04045 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        });
        channels[0] * 0.2126 + channels[1] * 0.7152 + channels[2] * 0.0722
    }
    #[test]
    fn oklch_series_final_hex_meets_contrast_and_hue_budget() {
        for (dark, palette) in [(true, &DARK), (false, &LIGHT)] {
            for (index, hue) in [110., 300., 210.].into_iter().enumerate() {
                let hex = super::series_hex(index, dark);
                for bg in [
                    palette.background,
                    palette.panel,
                    palette.raised,
                    palette.hover,
                ] {
                    let a = luminance(hex);
                    let b = luminance(bg);
                    assert!(
                        (a.max(b) + 0.05) / (a.min(b) + 0.05) >= 4.5,
                        "series {index}: #{hex:06x} on #{bg:06x}"
                    );
                }
                assert!((hue - 85_f64).abs() > 20., "not focus gold");
                assert!((hue - 145_f64).abs() > 20., "not direction green");
            }
        }
    }
    #[test]
    fn gold_price_label_has_at_least_4_5_contrast() {
        for palette in [&DARK, &LIGHT] {
            let a = luminance(palette.on_gold);
            let b = luminance(palette.gold_solid);
            assert!((a.max(b) + 0.05) / (a.min(b) + 0.05) >= 4.5);
        }
    }

    #[test]
    fn final_hex_body_and_secondary_contrast_is_at_least_4_5() {
        for palette in [&DARK, &LIGHT] {
            for foreground in [
                palette.foreground,
                palette.secondary_text,
                palette.muted_text,
                palette.gold_text,
            ] {
                for background in [
                    palette.background,
                    palette.panel,
                    palette.raised,
                    palette.hover,
                ] {
                    let a = luminance(foreground);
                    let b = luminance(background);
                    let ratio = (a.max(b) + 0.05) / (a.min(b) + 0.05);
                    assert!(
                        ratio >= 4.5,
                        "#{foreground:06x} / #{background:06x}: {ratio:.2}"
                    );
                }
            }
        }
    }
}
