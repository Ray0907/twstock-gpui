use gpui_kit::component::ThemeMode;
use gpui_kit::*;
use twstock_gpui::Workspace;

fn main() {
    gpui_kit::application()
        .with_assets(gpui_kit::assets::Assets)
        .run(|cx| {
            gpui_kit::init(cx);
            cx.on_window_closed(|cx, _| {
                if cx.windows().is_empty() {
                    cx.quit();
                }
            })
            .detach();

            let options = twstock_gpui::design::space::window_options(cx);
            gpui_kit::open_window(options, cx, |window, cx| {
                window.set_window_title("台股看盤");
                window.activate_window();
                twstock_gpui::theme::apply(ThemeMode::Dark, Some(window), cx);
                cx.new(|cx| Workspace::configured(window, cx))
            })
            .expect("Failed to open watchlist window");
        });
}
