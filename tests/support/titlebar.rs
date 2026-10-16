use super::*;

pub fn run() {
    let mut cx = HeadlessAppContext::with_platform(
        gpui_kit::platform::current_platform(true).text_system(),
        Arc::new(Assets),
        gpui_kit::platform::current_headless_renderer,
    );
    cx.update(gpui_kit::init);
    let (handle, _) = cx
        .update(|cx| {
            gpui_kit::open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds::new(
                        Default::default(),
                        size(px(1080.), px(720.)),
                    ))),
                    focus: false,
                    show: false,
                    ..TitleBar::window_options()
                },
                cx,
                |w, cx| {
                    twstock_gpui::theme::apply(ThemeMode::Dark, Some(w), cx);
                    cx.new(|cx| Workspace::new(w, cx))
                },
            )
        })
        .unwrap();
    for _ in 0..3 {
        cx.update_window(handle, |_, w, cx| w.render_frame(cx))
            .unwrap();
        cx.run_until_parked();
    }
    let mut failures = Vec::new();
    let original = cx
        .update_window(handle, |_, w, cx| {
            if w.try_find("nav-watchlist").is_some() {
                failures.push("redundant watchlist button remains");
            }
            if w.try_find("reset-layout").is_some() {
                failures.push("reset layout button remains");
            }
            let titlebar = w.find("title-bar").bounds();
            assert_eq!(titlebar.bottom(), px(34.));
            assert!(titlebar.contains(&w.find("open-palette").bounds().center()));
            let original = w.find("watchlist-panel").bounds().size.width;
            let before = w.find("watchlist-panel").bounds();
            let divider = point(before.right() + px(0.5), before.center().y);
            w.drag(divider, divider + point(px(32.), px(0.)), cx);
            assert!(w.find("watchlist-panel").bounds().size.width > original + px(10.));
            original
        })
        .unwrap();
    cx.update_window(handle, |_, w, cx| w.click("open-palette", cx))
        .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, w, cx| {
        w.render_frame(cx);
        assert!(w.try_find("directory-palette").is_some());
        let resized = w.find("watchlist-panel").bounds().size.width;
        w.press("cmd-shift-r", cx);
        assert!(w.try_find("directory-palette").is_some());
        if w.find("watchlist-panel").bounds().size.width != resized {
            failures.push("reset shortcut fired beneath modal palette");
        }
        w.press("escape", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, w, cx| {
        w.render_frame(cx);
        assert!(w.try_find("directory-palette").is_none());
        w.press("cmd-shift-r", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, w, cx| {
        w.render_frame(cx);
        if (w.find("watchlist-panel").bounds().size.width - original).abs() > px(1.) {
            failures.push("Cmd+Shift+R did not restore the default layout");
        }
    })
    .unwrap();
    assert!(
        failures.is_empty(),
        "title bar regressions: {}",
        failures.join("; ")
    );
    println!(
        "title bar: only search remains; Cmd+Shift+R resets layout and is blocked by modal palette; band height 34 px"
    );
}
