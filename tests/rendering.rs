//! Real offscreen GPU rendering; AppKit must initialize on this main thread.
use std::{collections::HashSet, sync::Arc, time::Duration};

use gpui_kit::component::plot::scale::Scale;
use gpui_kit::{
    AppContext, Bounds, Context, FontFeatures, FontWeight, HeadlessAppContext, Hsla, IntoElement,
    ParentElement, Render, Rgba, SharedString, Styled, Window, WindowBounds, WindowOptions,
    assets::Assets,
    component::{ActiveTheme, ThemeMode, TitleBar, h_flex, v_flex},
    div, point, px, size,
    test::TestWindowExt,
};
use twstock_gpui::Workspace;

fn color_bytes(color: Hsla) -> [u8; 4] {
    let color: Rgba = color.into();
    [color.r, color.g, color.b, color.a].map(|v| (v * 255.).round() as u8)
}

struct FixtureDirectory(Result<Vec<twstock_gpui::market::Instrument>, String>);
impl twstock_gpui::market::Directory for FixtureDirectory {
    fn instruments(&mut self) -> Result<Vec<twstock_gpui::market::Instrument>, String> {
        self.0.clone()
    }
}

struct FontWeights;
impl Render for FontWeights {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .size_full()
            .p_4()
            .gap_3()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .font_family(cx.theme().font_family.clone())
            .font_features(FontFeatures(Arc::new(vec![("tnum".into(), 1)])))
            .children(
                [
                    ("Regular 400", FontWeight::NORMAL),
                    ("Medium 500", FontWeight::MEDIUM),
                    ("Semibold 600", FontWeight::SEMIBOLD),
                ]
                .map(|(label, weight)| {
                    h_flex()
                        .gap_3()
                        .child(div().w_32().text_xs().child(SharedString::from(label)))
                        .child(
                            div()
                                .text_base()
                                .font_weight(weight)
                                .child(SharedString::from("範例電子 124.00 0123456789")),
                        )
                }),
            )
    }
}

fn font_weight_probe() {
    let mut cx = HeadlessAppContext::with_platform(
        gpui_kit::platform::current_platform(true).text_system(),
        Arc::new(Assets),
        gpui_kit::platform::current_headless_renderer,
    );
    cx.update(gpui_kit::init);
    let (handle, _) = cx
        .update(|cx| {
            twstock_gpui::theme::apply(ThemeMode::Dark, None, cx);
            gpui_kit::open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds {
                        origin: Default::default(),
                        size: size(px(580.), px(170.)),
                    })),
                    show: false,
                    focus: false,
                    ..Default::default()
                },
                cx,
                |_, cx| cx.new(|_| FontWeights),
            )
        })
        .unwrap();
    cx.update_window(handle, |_, window, cx| window.render_frame(cx))
        .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| window.render_frame(cx))
        .unwrap();
    cx.capture_screenshot(handle)
        .unwrap()
        .save("/tmp/twstock-gpui-font-weights.png")
        .unwrap();
}

fn render(mode: ThemeMode, width: u32, height: u32, path: &str) -> Vec<u8> {
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
                    window_bounds: Some(WindowBounds::Windowed(Bounds {
                        origin: Default::default(),
                        size: size(px(width as f32), px(height as f32)),
                    })),
                    focus: false,
                    show: false,
                    ..TitleBar::window_options()
                },
                cx,
                |window, cx| {
                    twstock_gpui::theme::apply(mode, Some(window), cx);
                    cx.new(|cx| Workspace::new(window, cx))
                },
            )
        })
        .expect("headless workspace must open");

    // Use Kit's frame helper, drain asset/layout tasks, and finish chart entry motion.
    cx.update_window(handle, |_, window, cx| window.render_frame(cx))
        .unwrap();
    cx.run_until_parked();
    cx.advance_clock(Duration::from_secs(2));
    cx.update_window(handle, |_, window, cx| window.render_frame(cx))
        .unwrap();
    cx.run_until_parked();
    let scale = cx
        .update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            let viewport = window.viewport_size();
            for id in ["watchlist-panel", "chart-panel", "quote-panel"] {
                let bounds = window.find(id).bounds();
                assert!(bounds.size.width > px(0.) && bounds.size.height > px(0.));
                assert!(bounds.left() >= px(0.) && bounds.top() >= px(0.));
                assert!(
                    bounds.right() <= viewport.width + px(0.5),
                    "{id} overflows horizontally"
                );
                assert!(
                    bounds.bottom() <= viewport.height + px(0.5),
                    "{id} overflows vertically"
                );
            }
            assert!(window.try_find("limits-section").is_none());
            let position = window.find("position-section").bounds();
            assert!(position.size.height > px(0.));
            assert!(position.bottom() <= window.find("quote-panel").bounds().bottom());
            assert!(
                window
                    .find("position-recent")
                    .label()
                    .unwrap()
                    .contains("根")
            );
            assert!(
                window
                    .find("position-volume")
                    .label()
                    .unwrap()
                    .contains("累計")
            );
            assert!(
                window
                    .find("candle-region")
                    .label()
                    .unwrap()
                    .contains("截斷於")
            );
            assert!(window.try_find("watchlist-actions").is_none());
            assert!(
                window.try_find("trade-feed").is_some(),
                "trades start expanded"
            );
            let feed = window.find("trade-feed").bounds();
            assert!(
                (feed.bottom() - window.find("quote-panel").bounds().bottom()).abs() < px(1.),
                "feed fills remaining pane height"
            );
            let header_before = window.find("quote-last").bounds();
            let trade_before = window.find(("trade-price", 0_usize)).bounds();
            window.scroll(
                "trade-feed",
                gpui_kit::ScrollDelta::Pixels(point(px(0.), px(-300.))),
                cx,
            );
            assert_eq!(
                window.find("quote-last").bounds(),
                header_before,
                "feed scrolling must not move quote summary"
            );
            if height == 600 {
                assert!(
                    window.find(("trade-price", 0_usize)).bounds().top() < trade_before.top(),
                    "small feed actually scrolls"
                );
            }
            window.scroll(
                "trade-feed",
                gpui_kit::ScrollDelta::Pixels(point(px(0.), px(300.))),
                cx,
            );
            window.click("trades-toggle", cx);
            window.render_frame(cx);
            assert!(window.try_find("trade-feed").is_none());
            window.click("trades-toggle", cx);
            window.render_frame(cx);
            assert!(window.try_find("trade-feed").is_some());
            assert!(
                window.try_find("ohlc-row").is_none(),
                "redundant top OHLC row is removed"
            );
            let watchlist = window.find("watchlist-panel").bounds();
            for row in [0_usize, 2, 4] {
                let price = window.find(("watch-price", row)).bounds();
                assert!(
                    price.right() <= watchlist.right(),
                    "watchlist price column is clipped"
                );
            }
            let bounds = window.find("candle-region").bounds();
            let layout = twstock_gpui::design::chart::ChartLayout::new(bounds, cx.theme().font_size);
            assert!(layout.price.bottom() < layout.volume.top());
            assert!(layout.volume.bottom() < bounds.size.height);
            assert_eq!(window.find(("row", 0_usize)).selected(), Some(true));
            for row in 1_usize..7 {
                assert_eq!(window.find(("row", row)).selected(), Some(false));
            }
            window.click(("row", 2_usize), cx);
            assert_eq!(window.find(("row", 2_usize)).selected(), Some(true));
            window.press("down", cx);
            assert_eq!(window.find(("row", 3_usize)).selected(), Some(true));
            window.click(("row", 0_usize), cx);
            assert_eq!(window.find(("row", 0_usize)).selected(), Some(true));

            let before = window.find("watchlist-panel").bounds();
            let divider = point(before.right() + px(0.5), before.center().y);
            window.drag(divider, point(divider.x + px(24.), divider.y), cx);
            assert!(
                window.find("watchlist-panel").bounds().size.width > before.size.width + px(10.),
                "dock divider must resize the panel"
            );
            window.click("reset-layout", cx);
            window.scale_factor()
        })
        .unwrap();
    cx.run_until_parked();
    let chart_left = cx
        .update_window(handle, |_, window, cx| {
            window.render_frame(cx);
            let chart = window.find("chart-panel").bounds();
            let quote = window.find("quote-panel").bounds();
            window.drag(
                point(chart.center().x, chart.top() - px(15.)),
                quote.center(),
                cx,
            );
            chart.left()
        })
        .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        let moved = window.find("chart-panel").bounds();
        assert!(
            moved.left() > chart_left + px(20.),
            "chart tab must move to the quote group: {chart_left:?} → {moved:?}"
        );
        window.click("reset-layout", cx);
    })
    .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| window.render_frame(cx))
        .unwrap();
    cx.advance_clock(Duration::from_secs(2));
    cx.run_until_parked();
    cx.update_window(handle, |_, window, cx| {
        window.render_frame(cx);
        assert!(
            window.find("watchlist-panel").bounds().left()
                < window.find("chart-panel").bounds().left()
        );
        assert!(
            window.find("chart-panel").bounds().left() < window.find("quote-panel").bounds().left()
        );
        assert_eq!(window.find(("row", 0_usize)).selected(), Some(true));
    })
    .unwrap();

    let (price_bounds, spark_bounds, foreground, direction_colors) = cx
        .update_window(handle, |_, window, cx| {
            let mut bounds = vec![
                window.find("quote-last").bounds(),
                window.find("chart-last").bounds(),
            ];
            for row in [0_usize, 2, 4] {
                bounds.push(window.find(("watch-last", row)).bounds());
            }
            for ix in 0_usize..6 {
                bounds.push(window.find(("quote-stat", ix)).bounds());
            }
            for ix in 0_usize..3 {
                bounds.push(window.find(("index-last", ix)).bounds());
                let trade = window.find(("trade-price", ix)).bounds();
                if trade.bottom() <= window.find("trade-feed").bounds().bottom() {
                    bounds.push(trade);
                }
            }
            (
                bounds,
                window.find(("watch-sparkline", 0_usize)).bounds(),
                color_bytes(cx.theme().foreground),
                [color_bytes(cx.theme().red), color_bytes(cx.theme().green)],
            )
        })
        .unwrap();
    let (
        plot_bounds,
        name_bounds,
        gold,
        faint_gold,
        focus_fill,
        focus_ink,
        volume_colors,
        ma_colors,
    ) = cx
        .update_window(handle, |_, window, cx| {
            (
                window.find("candle-region").bounds(),
                window.find("watch-name-991901").bounds(),
                color_bytes(cx.theme().link),
                {
                    let fg: Rgba = twstock_gpui::design::color::get(
                        twstock_gpui::design::color::ColorRole::LastPriceLine,
                        cx,
                    )
                    .into();
                    let bg: Rgba = cx.theme().background.into();
                    [
                        fg.r * fg.a + bg.r * (1. - fg.a),
                        fg.g * fg.a + bg.g * (1. - fg.a),
                        fg.b * fg.a + bg.b * (1. - fg.a),
                        1.,
                    ]
                    .map(|v| (v * 255.).round() as u8)
                },
                color_bytes(cx.theme().primary),
                color_bytes(cx.theme().primary_foreground),
                [cx.theme().red, cx.theme().green].map(|color| {
                    let c: Rgba = color.into();
                    let bg: Rgba = cx.theme().background.into();
                    [
                        c.r * 0.55 + bg.r * 0.45,
                        c.g * 0.55 + bg.g * 0.45,
                        c.b * 0.55 + bg.b * 0.45,
                        1.,
                    ]
                    .map(|v| (v * 255.).round() as u8)
                }),
                [0, 1, 2].map(|ix| {
                    color_bytes(
                        gpui_kit::rgb(twstock_gpui::design::color::series_hex(
                            ix,
                            cx.theme().mode.is_dark(),
                        ))
                        .into(),
                    )
                }),
            )
        })
        .unwrap();
    let image = cx
        .capture_screenshot(handle)
        .expect("real offscreen renderer must be available");
    if width == 1080 && mode == ThemeMode::Dark {
        image.save("/tmp/twstock-gpui-position.png").unwrap();
        image.save("/tmp/twstock-gpui-volume-clip.png").unwrap();
        snapshot::assert_snapshot("position", &image);
        snapshot::assert_snapshot("volume-clip", &image);
    }
    let pixels = |bounds: Bounds<gpui_kit::Pixels>| {
        let image = &image;
        let left = (bounds.left().as_f32() * scale).ceil() as u32;
        let right = (bounds.right().as_f32() * scale).floor() as u32;
        let top = (bounds.top().as_f32() * scale).ceil() as u32;
        let bottom = (bounds.bottom().as_f32() * scale).floor() as u32;
        (top..bottom).flat_map(move |y| (left..right).map(move |x| (y, image.get_pixel(x, y).0)))
    };
    for bounds in price_bounds {
        let values: Vec<_> = pixels(bounds).collect();
        assert!(
            values.iter().any(|(_, pixel)| *pixel == foreground),
            "neutral price glyphs missing at {bounds:?}"
        );
        assert!(
            !values
                .iter()
                .any(|(_, pixel)| direction_colors.contains(pixel)),
            "price text must never be red/green at {bounds:?}"
        );
    }
    assert!(
        pixels(name_bounds).any(|(_, pixel)| pixel == gold),
        "selected stock name uses focus gold"
    );
    let store = twstock_gpui::market::MockProvider::new().store();
    let symbol = &store.order[0];
    let snapshot = store.snapshot(symbol).unwrap();
    let data = store.chart_series(symbol).unwrap();
    let geometry =
        twstock_gpui::design::chart::ChartGeometry::new(plot_bounds, px(13.), &data, snapshot.last);
    let spike = data
        .candles
        .iter()
        .position(|c| c.volume == 600_000_000)
        .unwrap();
    let cap_bar = geometry.volume_bar(spike, data.candles[spike].volume);
    let marker = Bounds::new(
        plot_bounds.origin
            + point(
                px(geometry.candle(spike, &data.candles[spike]).x - 3.),
                cap_bar.top(),
            ),
        size(px(6.), px(5.)),
    );
    assert!(
        pixels(marker)
            .filter(|(_, pixel)| *pixel == foreground)
            .count()
            >= 2,
        "capped spike needs an explicit contrasting break marker"
    );
    let line_y = plot_bounds.top().as_f32() + geometry.price_y(snapshot.last.unwrap()).as_f32();
    let band = Bounds {
        origin: point(plot_bounds.left(), px(line_y - 1.5)),
        size: size(geometry.layout.price.size.width, px(3.)),
    };
    assert!(
        pixels(band).any(|(_, pixel)| pixel
            .iter()
            .zip(faint_gold)
            .all(|(a, b)| a.abs_diff(b) <= 5)),
        "gold line must align with the last candle's close"
    );
    let price_area = Bounds::new(
        plot_bounds.origin + geometry.layout.price.origin,
        geometry.layout.price.size,
    );
    let volume_area = Bounds::new(
        plot_bounds.origin + geometry.layout.volume.origin,
        geometry.layout.volume.size,
    );
    for color in ma_colors {
        assert!(
            pixels(price_area)
                .filter(|(_, p)| p.iter().zip(color).all(|(a, b)| a.abs_diff(b) <= 2))
                .count()
                > 10,
            "MA must paint a line, not just a legend"
        );
    }
    for (period, color) in [5, 10, 20].into_iter().zip(ma_colors) {
        let prefix = Bounds::new(
            price_area.origin,
            size(
                px(geometry.candle(period - 2, &data.candles[period - 2]).x),
                price_area.size.height,
            ),
        );
        assert!(
            !pixels(prefix).any(|(_, p)| p.iter().zip(color).all(|(a, b)| a.abs_diff(b) <= 2)),
            "MA prefix must stay empty, never zero-padded"
        );
    }
    for color in volume_colors {
        assert!(
            pixels(volume_area)
                .filter(|(_, p)| p.iter().zip(color).all(|(a, b)| a.abs_diff(b) <= 3))
                .count()
                > 10,
            "volume bars must be painted"
        );
    }
    let tag_box = geometry.price_tag(geometry.price_y(snapshot.last.unwrap()));
    let tag_box = Bounds::new(plot_bounds.origin + tag_box.origin, tag_box.size);
    assert!(pixels(tag_box).any(|(_, p)| p == focus_fill));
    assert!(pixels(tag_box).any(|(_, p)| p == focus_ink));
    let mut solid = 0;
    let mut hollow = 0;
    for (ix, c) in data.candles.iter().enumerate() {
        let c = geometry.candle(ix, c);
        if c.body.size.height < px(5.) || c.body.size.width < px(2.5) {
            continue;
        }
        let center = Bounds::new(
            plot_bounds.origin + c.body.center() - point(px(0.5), px(0.5)),
            size(px(1.), px(1.)),
        );
        match c.style {
            twstock_gpui::design::direction::CandleBody::Solid => {
                if pixels(center).any(|(_, p)| p == direction_colors[0]) {
                    solid += 1;
                }
            }
            twstock_gpui::design::direction::CandleBody::Hollow => {
                assert!(
                    !pixels(center).any(|(_, p)| p == direction_colors[1]),
                    "hollow candle interior cannot contain its wick or fill"
                );
                hollow += 1;
            }
            _ => {}
        }
    }
    assert!(
        solid > 0 && hollow > 0,
        "both candle encodings must actually render"
    );
    let ys: Vec<_> = pixels(spark_bounds)
        .filter(|(_, pixel)| direction_colors.contains(pixel))
        .map(|(y, _)| y)
        .collect();
    assert!(
        ys.iter().max().unwrap() - ys.iter().min().unwrap() > 4,
        "sparkline must visibly preserve price variation, not a zero-based flat line"
    );
    assert_eq!(
        image.dimensions(),
        (
            (width as f32 * scale).round() as u32,
            (height as f32 * scale).round() as u32
        ),
        "capture must match the scale-adjusted viewport",
    );
    let colors: HashSet<_> = image.pixels().map(|pixel| pixel.0).collect();
    assert!(colors.len() > 128, "UI capture must not be blank");
    image.save(path).expect("PNG capture must be saved");
    match (mode.is_dark(), width, height) {
        (true, 1080, 720) => snapshot::assert_snapshot("dark", &image),
        (false, 1080, 720) => snapshot::assert_snapshot("light", &image),
        (true, 900, 600) => snapshot::assert_snapshot("dark-small", &image),
        _ => {}
    }
    println!(
        "{mode:?}: {}×{}, scale {scale}, {} colors → {path}",
        image.width(),
        image.height(),
        colors.len()
    );
    if mode.is_dark() && width == 1080 && height == 720 {
        cx.update_window(handle, |_, window, cx| {
            window.hover("candle-region", cx);
            window.render_frame(cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.advance_clock(Duration::from_secs(2));
        cx.update_window(handle, |_, window, cx| window.render_frame(cx))
            .unwrap();
        cx.run_until_parked();
        let cross = cx.capture_screenshot(handle).unwrap();
        cross.save("/tmp/twstock-gpui-crosshair.png").unwrap();
        let cursor = point(plot_bounds.size.width / 2., plot_bounds.size.height / 2.);
        let ix = geometry
            .x
            .nearest_index(cursor.x.as_f32() - geometry.x.band_width() / 2.);
        let x = geometry.candle(ix, &data.candles[ix]).x;
        let y = cursor.y;
        let has_gold = |bounds: Bounds<gpui_kit::Pixels>, expected: [u8; 4]| {
            let left = (bounds.left().as_f32() * scale).ceil() as u32;
            let top = (bounds.top().as_f32() * scale).ceil() as u32;
            let right = (bounds.right().as_f32() * scale).floor() as u32;
            let bottom = (bounds.bottom().as_f32() * scale).floor() as u32;
            (top..bottom).any(|y| (left..right).any(|x| cross.get_pixel(x, y).0 == expected))
        };
        let vertical = Bounds::new(
            plot_bounds.origin + point(px(x - 0.75), geometry.layout.volume.top()),
            size(px(1.5), geometry.layout.volume.size.height),
        );
        assert!(
            has_gold(vertical, gold),
            "native plot hover paints a synchronized vertical crosshair in volume"
        );
        let horizontal = Bounds::new(
            plot_bounds.origin + point(px(0.), y - px(0.75)),
            size(geometry.layout.price.size.width, px(1.5)),
        );
        assert!(
            has_gold(horizontal, gold),
            "cursor price line must use the same scale as its axis tag"
        );
        let info_area = Bounds::new(
            plot_bounds.origin,
            size(plot_bounds.size.width, geometry.layout.caption),
        );
        assert!(
            has_gold(info_area, foreground),
            "hover OHLC and volume must use neutral foreground"
        );
        assert!(
            !has_gold(info_area, direction_colors[0]) && !has_gold(info_area, direction_colors[1])
        );
        let price_tag = geometry.price_tag(y);
        assert!(
            has_gold(
                Bounds::new(plot_bounds.origin + price_tag.origin, price_tag.size),
                focus_fill
            ),
            "gold cursor price label missing"
        );
        let date_area = Bounds::new(
            plot_bounds.origin + point(px(x - 25.), geometry.layout.volume.bottom() + px(2.)),
            size(px(50.), px(16.)),
        );
        assert!(
            has_gold(date_area, focus_fill),
            "gold cursor date label missing"
        );
        assert!((geometry.price_y(geometry.cursor_price(y)) - y).abs() < px(0.02));
        cross.save("/tmp/twstock-gpui-crosshair.png").unwrap();
        snapshot::assert_snapshot("crosshair", &cross);
        volume::check(&mut cx, handle, plot_bounds, &data, snapshot.last);
    }
    image.into_raw()
}

fn watchlist_edge_cases(count: usize) {
    let mut store = twstock_gpui::market::MockProvider::new().store();
    store.order.truncate(count);
    let symbols = store.order.clone();
    let full_name = SharedString::from("六個中文字名");
    store.instruments.get_mut(&symbols[0]).unwrap().name = full_name.to_string();
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
                |window, cx| {
                    twstock_gpui::theme::apply(ThemeMode::Dark, Some(window), cx);
                    cx.new(|cx| Workspace::with_store(store, window, cx))
                },
            )
        })
        .unwrap();
    for _ in 0..3 {
        cx.update_window(handle, |_, window, cx| window.render_frame(cx))
            .unwrap();
        cx.run_until_parked();
    }
    let (last, panel) = cx
        .update_window(handle, |_, window, _| {
            for symbol in &symbols {
                assert!(window.find(format!("watch-name-{}", symbol.code)).visible());
            }
            assert!(window.try_find("watch-name-991904").is_none());
            for row in 0usize..count {
                assert!(window.find(("row", row)).visible());
            }
            assert!(
                window.try_find(("row", count)).is_none(),
                "exactly N rows, not just N names"
            );
            (
                window.find(("row", count - 1)).bounds(),
                window.find("watchlist-panel").bounds(),
            )
        })
        .unwrap();
    let image = cx.capture_screenshot(handle).unwrap();
    image.save("/tmp/twstock-twstock-gpui-edge.png").unwrap();
    let scale = image.width() as f32 / 1080.;
    let x = ((panel.left() + px(20.)).as_f32() * scale) as u32;
    let first = ((last.bottom() + px(4.)).as_f32() * scale) as u32;
    let end = ((panel.bottom() - px(4.)).as_f32() * scale) as u32;
    let background = image.get_pixel(x, end).0;
    assert!(
        (first..end).all(|y| image.get_pixel(x, y).0 == background),
        "N={count} must render only N rows, no filler stripes or row borders"
    );
    cx.update_window(handle, |_, window, cx| {
        window.hover(format!("watch-name-{}", symbols[0].code), cx)
    })
    .unwrap();
    cx.run_until_parked();
    cx.advance_clock(Duration::from_secs(2));
    cx.update_window(handle, |_, window, cx| window.render_frame(cx))
        .unwrap();
    cx.run_until_parked();
    cx.update_window(handle, |_, window, _| {
        let name = window.find(format!("watch-name-{}", symbols[0].code));
        let cell = window.find(format!("watch-name-cell-{}", symbols[0].code));
        assert!(name.bounds().right() <= cell.bounds().right());
        let tooltip = window.find("watch-full-name");
        assert_eq!(tooltip.label(), Some(full_name.as_ref()));
        assert!(
            tooltip.bounds().size.width > name.bounds().size.width,
            "six Chinese characters must be fully available on hover"
        );
    })
    .unwrap();
}
fn reference_price_rendering(mode: ThemeMode, height: f32) {
    use twstock_gpui::market::*;
    let mock = MockProvider::new().store();
    let symbol = mock.order[0].clone();
    let mut store = MarketStore::new(Capabilities::mis());
    store.add_instrument(mock.instruments[&symbol].clone());
    store.set_history(symbol.clone(), Ok(mock.histories[&symbol].clone()));
    let at = mock.snapshot(&symbol).unwrap().as_of.unwrap();
    store.apply(
        MarketEvent::Quote(QuoteUpdate {
            symbol: symbol.clone(),
            as_of: Some(at),
            prev_close: Some(Price(1230000)),
            open: Some(Price(1230000)),
            high: Some(Price(1260000)),
            low: Some(Price(1220000)),
            volume: Some(7000),
            bids: Some(vec![Depth {
                price: Price(1240000),
                quantity: 1000,
                as_of: at,
            }]),
            asks: Some(vec![Depth {
                price: Price(1242000),
                quantity: 1000,
                as_of: at,
            }]),
            ..Default::default()
        }),
        at,
    );
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
                        size(px(1080.), px(height)),
                    ))),
                    focus: false,
                    show: false,
                    ..TitleBar::window_options()
                },
                cx,
                |window, cx| {
                    twstock_gpui::theme::apply(mode, Some(window), cx);
                    cx.new(|cx| Workspace::with_store(store, window, cx))
                },
            )
        })
        .unwrap();
    for _ in 0..3 {
        cx.update_window(handle, |_, window, cx| window.render_frame(cx))
            .unwrap();
        cx.run_until_parked();
    }
    let (regions, muted, red, green) = cx
        .update_window(handle, |_, window, cx| {
            for id in ["quote-reference", "chart-reference"] {
                assert!(window.find(id).visible());
            }
            assert!(window.find(("watch-reference", 0usize)).visible());
            let watch = window.find(("watch-price", 0usize)).bounds();
            assert!(
                window.find(("watch-reference", 0usize)).bounds().right() <= watch.right(),
                "reference label must stay in its cell"
            );
            (
                [
                    window.find("quote-last").bounds(),
                    window.find("chart-last").bounds(),
                    watch,
                ],
                color_bytes(cx.theme().muted_foreground),
                color_bytes(cx.theme().red),
                color_bytes(cx.theme().green),
            )
        })
        .unwrap();
    cx.update_window(handle, |_, window, _| {
        assert!(window.try_find("trade-feed").is_none());
        assert!(window.try_find("trades-toggle").is_none());
        assert!(
            window
                .find("position-today")
                .label()
                .unwrap()
                .contains("不標現價")
        );
    })
    .unwrap();
    let image = cx.capture_screenshot(handle).unwrap();
    let scale = image.width() as f32 / 1080.;
    for region in regions {
        let mut grey = 0;
        for y in (region.top().as_f32() * scale).ceil() as u32
            ..(region.bottom().as_f32() * scale).floor() as u32
        {
            for x in (region.left().as_f32() * scale).ceil() as u32
                ..(region.right().as_f32() * scale).floor() as u32
            {
                let p = image.get_pixel(x, y).0;
                assert!(
                    p != red && p != green,
                    "reference price/change must not use trade direction colors"
                );
                if p == muted {
                    grey += 1;
                }
            }
        }
        assert!(grey > 5, "reference prices must be visibly muted");
    }
    image
        .save(if mode.is_dark() {
            "/tmp/twstock-gpui-reference-dark.png"
        } else {
            "/tmp/twstock-gpui-reference-light.png"
        })
        .unwrap();
    if height < 600. {
        cx.update_window(handle, |_, window, cx| {
            window.scroll(
                "quote-panel",
                gpui_kit::ScrollDelta::Pixels(point(px(0.), px(-300.))),
                cx,
            );
            window.render_frame(cx);
            assert!(
                window.find("position-section").bounds().bottom()
                    <= window.find("quote-panel").bounds().bottom(),
                "position remains reachable at the minimum window height"
            );
        })
        .unwrap();
    }
}
fn period_rendering() {
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
    for (id, path) in [
        ("period-week", "/tmp/twstock-gpui-weekly.png"),
        ("period-month", "/tmp/twstock-gpui-monthly.png"),
    ] {
        for _ in 0..3 {
            cx.update_window(handle, |_, w, cx| w.render_frame(cx))
                .unwrap();
            cx.run_until_parked();
        }
        cx.update_window(handle, |_, w, cx| w.click(id, cx))
            .unwrap();
        for _ in 0..3 {
            cx.update_window(handle, |_, w, cx| w.render_frame(cx))
                .unwrap();
            cx.run_until_parked();
        }
        cx.update_window(handle, |_, w, cx| {
            let prefix = if id == "period-week" {
                "週線"
            } else {
                "月線"
            };
            assert!(w.find("candle-region").label().unwrap().starts_with(prefix));
            w.click(("row", 1usize), cx);
            w.render_frame(cx);
            assert!(
                w.find("candle-region").label().unwrap().starts_with(prefix),
                "stock selection must preserve the period"
            );
            w.click(("row", 0usize), cx);
            w.render_frame(cx);
        })
        .unwrap();
        let image = cx.capture_screenshot(handle).unwrap();
        image.save(path).unwrap();
        snapshot::assert_snapshot(
            if id == "period-week" {
                "weekly"
            } else {
                "monthly"
            },
            &image,
        );
    }
}

fn palette_rendering() {
    use twstock_gpui::market::*;
    for failed in [false, true] {
        let mut store = MockProvider::new().store();
        store.capabilities = Capabilities::mis();
        let symbol = store.order[0].clone();
        let item = store.instruments[&symbol].clone();
        let directory = FixtureDirectory(if failed {
            Err("編造的 Directory 網路失敗".into())
        } else {
            Ok(vec![item])
        })
        .instruments();
        let temp = tempfile::tempdir().unwrap();
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
                        cx.new(|cx| {
                            Workspace::with_services(
                                store,
                                directory,
                                temp.path().join("watchlist.toml"),
                                w,
                                cx,
                            )
                        })
                    },
                )
            })
            .unwrap();
        for _ in 0..3 {
            cx.update_window(handle, |_, w, cx| w.render_frame(cx))
                .unwrap();
            cx.run_until_parked();
        }
        cx.update_window(handle, |_, w, cx| {
            w.click(("row", 0usize), cx);
            w.press("cmd-k", cx);
        })
        .unwrap();
        for _ in 0..3 {
            cx.update_window(handle, |_, w, cx| w.render_frame(cx))
                .unwrap();
            cx.run_until_parked();
        }
        cx.update_window(handle, |_, w, _| {
            assert!(w.find("directory-palette").visible());
            if failed {
                assert!(w.find("palette-message").label().unwrap().contains("失敗"));
            } else {
                assert!(w.find("palette-watched-991901").visible());
            }
        })
        .unwrap();
        if !failed {
            cx.capture_screenshot(handle)
                .unwrap()
                .save("/tmp/twstock-gpui-palette.png")
                .unwrap();
        }
        cx.update_window(handle, |_, w, cx| w.press("escape", cx))
            .unwrap();
        for _ in 0..3 {
            cx.update_window(handle, |_, w, cx| w.render_frame(cx))
                .unwrap();
            cx.run_until_parked();
        }
        cx.update_window(handle, |_, w, _| {
            assert!(w.try_find("directory-palette").is_none())
        })
        .unwrap();
    }
}
fn palette_workflow() {
    use twstock_gpui::market::*;
    for mock in [false, true] {
        let mut store = MockProvider::new().store();
        let keep = store.order[0].clone();
        for symbol in store.order.clone().into_iter().skip(1) {
            store.remove_instrument(&symbol);
        }
        if !mock {
            let at = store.snapshot(&keep).unwrap().as_of.unwrap() + 1;
            store.apply(
                MarketEvent::Quote(QuoteUpdate {
                    symbol: keep.clone(),
                    as_of: Some(at),
                    source: Some("Fixture".into()),
                    ..Default::default()
                }),
                at,
            );
            store.capabilities.max_subscriptions = 2;
        }
        let mut a = Instrument::new(Symbol::new(Market::Otc, "889908").unwrap());
        a.name = "編造候選甲".into();
        let mut b = Instrument::new(Symbol::new(Market::Listed, "889909").unwrap());
        b.name = "編造候選乙".into();
        b.tick_rule = TickRule::Etf;
        let directory = vec![store.instruments[&keep].clone(), a, b];
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("watchlist.toml");
        if !mock {
            save_watchlist(
                &path,
                WatchlistChange::Add(store.instruments[&keep].clone()),
                2,
            )
            .unwrap();
        }
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
                        cx.new(|cx| {
                            Workspace::with_services(
                                store,
                                FixtureDirectory(Ok(directory)).instruments(),
                                path.clone(),
                                w,
                                cx,
                            )
                        })
                    },
                )
            })
            .unwrap();
        for _ in 0..3 {
            cx.update_window(handle, |_, w, cx| w.render_frame(cx))
                .unwrap();
            cx.run_until_parked();
        }
        if mock {
            for _ in 0..1 {
                cx.update_window(handle, |_, w, cx| {
                    w.click(("row", 0usize), cx);
                    w.press("delete", cx);
                })
                .unwrap();
                cx.run_until_parked();
                cx.update_window(handle, |_, w, cx| w.render_frame(cx))
                    .unwrap();
                cx.run_until_parked();
            }
            cx.update_window(handle, |_, w, _| {
                assert!(
                    w.try_find(("row", 0usize)).is_none(),
                    "Delete must remove selected rows"
                )
            })
            .unwrap();
        }
        cx.update_window(handle, |_, w, cx| {
            if !mock {
                w.click(("row", 0usize), cx);
            }
            w.press("cmd-k", cx);
        })
        .unwrap();
        for _ in 0..3 {
            cx.update_window(handle, |_, w, cx| w.render_frame(cx))
                .unwrap();
            cx.run_until_parked();
        }
        if !mock {
            cx.capture_screenshot(handle)
                .unwrap()
                .save("/tmp/twstock-gpui-palette.png")
                .unwrap();
        }
        cx.update_window(handle, |_, w, cx| w.input("8899", cx))
            .unwrap();
        cx.advance_clock(Duration::from_secs(1));
        cx.run_until_parked();
        for _ in 0..3 {
            cx.update_window(handle, |_, w, cx| w.render_frame(cx))
                .unwrap();
            cx.run_until_parked();
        }
        cx.update_window(handle, |_, w, cx| {
            w.press("down", cx);
            w.press("enter", cx);
        })
        .unwrap();
        cx.run_until_parked();
        let items = parse_watchlist(
            &std::fs::read_to_string(&path).expect("Enter must persist the selected result"),
        )
        .unwrap();
        assert_eq!(items.len(), if mock { 1 } else { 2 });
        assert!(
            items
                .iter()
                .any(|i| i.symbol.code == "889909" && i.tick_rule == TickRule::Etf),
            "down key must select the second filtered result"
        );
        for _ in 0..3 {
            cx.update_window(handle, |_, w, cx| w.render_frame(cx))
                .unwrap();
            cx.run_until_parked();
        }
        if mock {
            cx.update_window(handle, |_, w, _| {
                assert!(w.find("palette-message").label().unwrap().contains("切換"))
            })
            .unwrap();
        }
        let before = std::fs::read_to_string(&path).unwrap();
        cx.update_window(handle, |_, w, cx| w.press("enter", cx))
            .unwrap();
        cx.run_until_parked();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), before);
        if !mock {
            cx.update_window(handle, |_, w, cx| {
                w.press("cmd-a", cx);
                w.input("889908", cx);
            })
            .unwrap();
            cx.advance_clock(Duration::from_secs(1));
            cx.run_until_parked();
            for _ in 0..3 {
                cx.update_window(handle, |_, w, cx| w.render_frame(cx))
                    .unwrap();
                cx.run_until_parked();
            }
            cx.update_window(handle, |_, w, cx| w.press("enter", cx))
                .unwrap();
            cx.run_until_parked();
            cx.update_window(handle, |_, w, cx| {
                w.render_frame(cx);
                assert!(w.find("palette-message").label().unwrap().contains("上限"));
            })
            .unwrap();
            assert_eq!(std::fs::read_to_string(&path).unwrap(), before);
        }
        cx.update_window(handle, |_, w, cx| w.press("escape", cx))
            .unwrap();
        for _ in 0..3 {
            cx.update_window(handle, |_, w, cx| w.render_frame(cx))
                .unwrap();
            cx.run_until_parked();
        }
        if !mock {
            cx.update_window(handle, |_, w, cx| {
                w.click(("row", 1usize), cx);
                w.press("backspace", cx);
            })
            .unwrap();
            cx.run_until_parked();
            assert_eq!(
                parse_watchlist(&std::fs::read_to_string(&path).unwrap())
                    .unwrap()
                    .len(),
                1
            );
            for _ in 0..3 {
                cx.update_window(handle, |_, w, cx| w.render_frame(cx))
                    .unwrap();
                cx.run_until_parked();
            }
            cx.update_window(handle, |_, w, cx| {
                w.click(("row", 0usize), cx);
                w.within(("resizable-panel", 0usize)).click("menu", cx);
                assert!(w.find("popup-menu").visible());
                w.press("down", cx);
                w.press("enter", cx);
            })
            .unwrap();
            cx.run_until_parked();
            assert!(
                parse_watchlist(&std::fs::read_to_string(&path).unwrap())
                    .unwrap()
                    .is_empty(),
                "visible remove menu must persist an empty list"
            );
        }
    }
}
fn position_layout_cases() {
    use twstock_gpui::{design::position::PositionLayout, market::*};
    let mut store = MockProvider::new().store();
    let symbol = store.order[0].clone();
    store.instruments.get_mut(&symbol).unwrap().tick_rule = TickRule::Etf;
    let at = store.snapshot(&symbol).unwrap().as_of.unwrap() + 1;
    let history = store.histories.get_mut(&symbol).unwrap();
    let offset = 865000 - history.last().unwrap().close.0;
    for c in history {
        for p in [&mut c.open, &mut c.high, &mut c.low, &mut c.close] {
            *p = TickRule::Etf.round(Price(p.0 + offset));
        }
    }
    store.apply(
        MarketEvent::Quote(QuoteUpdate {
            symbol,
            as_of: Some(at),
            last: Some(Price(827000)),
            low: Some(Price(823500)),
            high: Some(Price(836500)),
            open: Some(Price(836500)),
            prev_close: Some(Price(865000)),
            limit_down: Some(Price(779000)),
            limit_up: Some(Price(951000)),
            ..Default::default()
        }),
        at,
    );
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
                |window, cx| {
                    twstock_gpui::theme::apply(ThemeMode::Dark, Some(window), cx);
                    cx.new(|cx| Workspace::with_store(store, window, cx))
                },
            )
        })
        .unwrap();
    for _ in 0..3 {
        cx.update_window(handle, |_, w, cx| w.render_frame(cx))
            .unwrap();
        cx.run_until_parked();
    }
    cx.update_window(handle, |_, w, cx| {
        let plot_bounds = w.find("position-plot").bounds();
        assert!(
            w.find("position-limit-down")
                .label()
                .unwrap()
                .starts_with("跌停 ")
        );
        assert!(
            w.find("position-limit-up")
                .label()
                .unwrap()
                .starts_with("漲停 ")
        );
        for (low, high, prev, last, open, reference) in [
            (82.35, 83.65, 86.50, 82.70, 83.65, false), // previous close outside today's high/low
            (84., 84., 84., 84., 84., false),           // flat, all labels coincide
            (82.35, 83.65, 83., 82.70, 82.35, false),   // open at low
            (82.35, 83.65, 83., 82.70, 83.65, false),   // open at high
            (82.35, 83.65, 82.70, 82.70, 83., false),   // current at previous close
            (82.35, 83.65, 86.50, 1000., 83.65, true),  // reference must not affect domain
            (82.35, 82.45, 103.50, 82.40, 82.35, false), // very close labels
        ] {
            let mut markers = vec![("低", low), ("高", high), ("昨收", prev), ("開", open)];
            if !reference {
                markers.push(("現", last));
            }
            for rule in [TickRule::Etf, TickRule::Stock] {
                for width in [120., plot_bounds.size.width.as_f32(), 260.] {
                    let frame = size(px(width), plot_bounds.size.height);
                    let layout =
                        PositionLayout::new(&markers, rule, frame, cx.theme().font_size, w)
                            .unwrap();
                    let mut prices = vec![low, high, prev, open];
                    if !reference {
                        prices.push(last);
                    }
                    let min = prices.iter().copied().fold(f64::INFINITY, f64::min);
                    let max = prices.iter().copied().fold(f64::NEG_INFINITY, f64::max);
                    let tick = [min, max]
                        .map(|v| rule.tick(Price((v * 10000.).round() as i64)).0)
                        .into_iter()
                        .max()
                        .unwrap() as f64
                        / 10000.;
                    let pad = ((max - min) * 0.08).max(tick);
                    assert!((layout.domain[0] - (min - pad)).abs() < 1e-9);
                    assert!((layout.domain[1] - (max + pad)).abs() < 1e-9);
                    assert_eq!(layout.labels.iter().any(|l| l.name == "現"), !reference);
                    for (i, label) in layout.labels.iter().enumerate() {
                        let b = label.bounds;
                        assert!(
                            b.left() >= px(0.)
                                && b.right() <= frame.width
                                && b.top() >= px(0.)
                                && b.bottom() <= frame.height,
                            "{} outside frame {frame:?}: {b:?}",
                            label.name
                        );
                        for other in &layout.labels[i + 1..] {
                            assert!(
                                b.right() <= other.bounds.left()
                                    || other.bounds.right() <= b.left()
                                    || b.bottom() <= other.bounds.top()
                                    || other.bounds.bottom() <= b.top(),
                                "{} overlaps {}: {b:?}, {:?}",
                                label.name,
                                other.name,
                                other.bounds
                            );
                        }
                        let expected_x = layout.axis.left().as_f32()
                            + ((label.value - layout.domain[0])
                                / (layout.domain[1] - layout.domain[0]))
                                as f32
                                * layout.axis.size.width.as_f32();
                        assert!(
                            (label.x.as_f32() - expected_x).abs() < 0.001,
                            "{} must share the axis scale",
                            label.name
                        );
                    }
                }
            }
        }
    })
    .unwrap();
    cx.capture_screenshot(handle)
        .unwrap()
        .save("/tmp/twstock-gpui-position-outside.png")
        .unwrap();
}
#[path = "support/snapshot.rs"]
mod snapshot;
#[path = "support/states.rs"]
mod states;
#[path = "support/volume.rs"]
mod volume;
fn main() {
    states::run();
    position_layout_cases();
    palette_rendering();
    palette_workflow();
    period_rendering();
    reference_price_rendering(ThemeMode::Dark, 488.);
    reference_price_rendering(ThemeMode::Dark, 720.);
    reference_price_rendering(ThemeMode::Light, 720.);
    for count in [1, 3] {
        watchlist_edge_cases(count);
    }
    font_weight_probe();
    for (width, height, suffix) in [(1080, 720, ""), (900, 600, "-small")] {
        let dark_path = format!("/tmp/twstock-gpui-dark{suffix}.png");
        let light_path = format!("/tmp/twstock-gpui-light{suffix}.png");
        let dark = render(ThemeMode::Dark, width, height, &dark_path);
        assert!(
            dark == render(ThemeMode::Dark, width, height, &dark_path),
            "repeated dark renders must be identical"
        );
        let light = render(ThemeMode::Light, width, height, &light_path);
        assert!(
            light == render(ThemeMode::Light, width, height, &light_path),
            "repeated light renders must be identical"
        );
        assert!(dark != light, "dark and light images must differ");
    }
    println!("rendering: pixel, bounds, selection, dock resize/drag/reset assertions passed");
}
