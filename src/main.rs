use gpui_kit::component::ThemeMode;
use gpui_kit::*;
use twstock_gpui::Workspace;

// `cargo run` has no .app bundle, so the Dock would show a generic icon.
#[cfg(target_os = "macos")]
#[allow(unexpected_cfgs)] // objc 0.2's macros probe a `cargo-clippy` feature
fn set_dock_icon() {
    use objc::{class, msg_send, runtime::Object, sel, sel_impl};
    static ICON: &[u8] = include_bytes!("../assets/icon/icon-1024.png");
    // SAFETY: called from the GPUI startup callback, which runs on the main thread.
    unsafe {
        let data: *mut Object =
            msg_send![class!(NSData), dataWithBytes: ICON.as_ptr() length: ICON.len()];
        let image: *mut Object = msg_send![class!(NSImage), alloc];
        let image: *mut Object = msg_send![image, initWithData: data];
        if !image.is_null() {
            let app: *mut Object = msg_send![class!(NSApplication), sharedApplication];
            let _: () = msg_send![app, setApplicationIconImage: image];
        }
    }
}

fn main() {
    gpui_kit::application()
        .with_assets(gpui_kit::assets::Assets)
        .run(|cx| {
            gpui_kit::init(cx);
            #[cfg(target_os = "macos")]
            set_dock_icon();
            cx.on_window_closed(|cx, _| {
                if cx.windows().is_empty() {
                    cx.quit();
                }
            })
            .detach();

            let options = twstock_gpui::design::space::window_options(cx);
            gpui_kit::open_window(options, cx, |window, cx| {
                window.set_window_title("twstock-gpui");
                window.activate_window();
                twstock_gpui::theme::apply(ThemeMode::Dark, Some(window), cx);
                cx.new(|cx| Workspace::configured(window, cx))
            })
            .expect("Failed to open watchlist window");
        });
}
