use super::{
    color::{self, ColorRole},
    text::{TextRole, text},
};
use gpui_kit::{App, Div, Hsla, Styled};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Up,
    Down,
    Flat,
    Unknown,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CandleBody {
    Solid,
    Hollow,
    Doji,
}

impl Direction {
    pub fn of(change: f64) -> Self {
        if !change.is_finite() {
            Self::Unknown
        } else if change > 0. {
            Self::Up
        } else if change < 0. {
            Self::Down
        } else {
            Self::Flat
        }
    }
    pub fn color(self, cx: &App) -> Hsla {
        color::get(
            match self {
                Self::Up => ColorRole::Up,
                Self::Down => ColorRole::Down,
                Self::Flat | Self::Unknown => ColorRole::Muted,
            },
            cx,
        )
    }
    pub fn arrow(self) -> &'static str {
        match self {
            Self::Up => "▲",
            Self::Down => "▼",
            Self::Flat => "—",
            Self::Unknown => "…",
        }
    }
    pub fn candle_body(self) -> CandleBody {
        match self {
            Self::Up => CandleBody::Solid,
            Self::Down => CandleBody::Hollow,
            Self::Flat | Self::Unknown => CandleBody::Doji,
        }
    }
    pub fn signed(self, value: f64) -> String {
        if self == Self::Unknown {
            return "—".into();
        }
        match self {
            Self::Up => format!("+{:.2}", value.abs()),
            Self::Down => format!("−{:.2}", value.abs()),
            Self::Flat => "0.00".into(),
            Self::Unknown => "—".into(),
        }
    }
    pub fn change(self, value: f64, percent: bool, cx: &App) -> Div {
        if self == Self::Unknown {
            return text(TextRole::Caption, "尚無成交", cx);
        }
        text(
            TextRole::Body,
            format!(
                "{} {}{}",
                self.arrow(),
                self.signed(value),
                if percent { "%" } else { "" }
            ),
            cx,
        )
        .text_color(self.color(cx))
    }
    pub fn marker(self, cx: &App) -> Div {
        text(TextRole::Body, self.arrow(), cx).text_color(self.color(cx))
    }
}
#[cfg(test)]
mod tests {
    use super::Direction;
    use gpui_kit::TestAppContext;
    use gpui_kit::component::{ActiveTheme, ThemeMode};
    #[gpui_kit::test]
    fn taiwan_direction_is_the_theme_contract(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_kit::init(cx);
            for mode in [ThemeMode::Dark, ThemeMode::Light] {
                crate::theme::apply(mode, None, cx);
                assert_eq!(Direction::of(1.).color(cx), cx.theme().red);
                assert_eq!(Direction::of(-1.).color(cx), cx.theme().green);
                assert_eq!(Direction::of(0.).color(cx), cx.theme().muted_foreground);
            }
        });
    }
    #[test]
    fn candle_direction_has_a_non_color_encoding() {
        use super::CandleBody;
        assert_eq!(Direction::of(1.).candle_body(), CandleBody::Solid);
        assert_eq!(Direction::of(-1.).candle_body(), CandleBody::Hollow);
        assert_eq!(Direction::of(0.).candle_body(), CandleBody::Doji);
        assert_ne!(Direction::Up.candle_body(), Direction::Down.candle_body());
    }
    #[test]
    fn signs_and_arrows_have_one_source() {
        assert_eq!(Direction::Up.signed(1.5), "+1.50");
        assert_eq!(Direction::Down.signed(-1.5), "−1.50");
        assert_eq!(Direction::Flat.signed(0.), "0.00");
        assert_eq!(
            (
                Direction::Up.arrow(),
                Direction::Down.arrow(),
                Direction::Flat.arrow()
            ),
            ("▲", "▼", "—")
        );
    }
}
