use gpui_kit::component::ActiveTheme;
use gpui_kit::*;
#[derive(Clone, Copy)]
pub enum Space {
    Micro,
    Tight,
    Small,
    Medium,
    Large,
}
impl Space {
    pub fn rem(self) -> Rems {
        rems(match self {
            Self::Micro => 0.125,
            Self::Tight => 0.25,
            Self::Small => 0.5,
            Self::Medium => 0.75,
            Self::Large => 1.,
        })
    }
}
#[derive(Clone, Copy)]
pub(super) enum Dimension {
    Watchlist,
    WatchRow,
    WatchHeader,
    Axis,
    DataRow,
    Quantity,
    Marker,
    StatusLane,
}
impl Dimension {
    pub fn rem(self) -> Rems {
        rems(match self {
            Self::Watchlist => 14.5,
            Self::WatchRow => 2.75,
            Self::WatchHeader => 1.75,
            Self::Axis => 4.,
            Self::DataRow => 1.5,
            Self::Quantity => 2.,
            Self::Marker => 1.25,
            Self::StatusLane => 14.,
        })
    }
}

pub fn window_options(cx: &App) -> WindowOptions {
    let font = cx.theme().font_size;
    WindowOptions {
        window_bounds: Some(WindowBounds::centered(
            size(rems(67.5).to_pixels(font), rems(45.).to_pixels(font)),
            cx,
        )),
        window_min_size: Some(size(
            rems(56.25).to_pixels(font),
            rems(37.5).to_pixels(font),
        )),
        ..gpui_kit::component::TitleBar::window_options()
    }
}
pub fn dock_sizes(window: &Window, cx: &App) -> [Pixels; 3] {
    let left = rems(14.5).to_pixels(cx.theme().font_size);
    let right = rems(17.5).to_pixels(cx.theme().font_size);
    [left, window.viewport_size().width - left - right, right]
}
pub fn watch_widths(width: Pixels) -> [Pixels; 3] {
    [
        width * (6. / 14.5),
        width * (6. / 14.5),
        width * (2.5 / 14.5),
    ]
}
