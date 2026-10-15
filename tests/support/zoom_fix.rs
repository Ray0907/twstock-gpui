use super::*;

pub(super) fn run() {
    let mut failures = Vec::new();
    overlay_gestures(&mut failures);
    zoom_during_drag(&mut failures);
    blur_cancels_drag(&mut failures);
    assert!(
        failures.is_empty(),
        "zoom regressions:\n{}",
        failures.join("\n")
    );
    println!(
        "zoom regressions: overlay hitboxes, wheel/pinch during drag (with/without redraw), window blur passed"
    );
}
pub(super) fn chart_label(cx: &mut HeadlessAppContext, handle: AnyWindowHandle) -> String {
    cx.update_window(handle, |_, w, _| {
        w.find("candle-region").label().unwrap().to_owned()
    })
    .unwrap()
}
pub(super) fn chart_image(
    cx: &mut HeadlessAppContext,
    handle: AnyWindowHandle,
) -> image::RgbaImage {
    cx.update_window(handle, |_, w, cx| {
        w.dispatch_event(
            MouseMoveEvent {
                position: point(px(0.), px(0.)),
                ..Default::default()
            }
            .to_platform_input(),
            cx,
        );
    })
    .unwrap();
    frames(cx, handle);
    let (b, s) = cx
        .update_window(handle, |_, w, _| {
            (w.find("candle-region").bounds(), w.scale_factor())
        })
        .unwrap();
    let image = cx.capture_screenshot(handle).unwrap();
    image::imageops::crop_imm(
        &image,
        (b.left().as_f32() * s) as u32,
        (b.top().as_f32() * s) as u32,
        (b.size.width.as_f32() * s) as u32,
        (b.size.height.as_f32() * s) as u32,
    )
    .to_image()
}
fn overlay_gestures(failures: &mut Vec<String>) {
    for gesture in ["wheel", "pinch", "drag", "reset"] {
        let mut cx = context();
        let (handle, _) = cx
            .update(|cx| {
                gpui_kit::open_window(options(), cx, |w, cx| {
                    twstock_gpui::theme::apply(ThemeMode::Dark, Some(w), cx);
                    cx.new(|cx| Workspace::new(w, cx))
                })
            })
            .unwrap();
        frames(&mut cx, handle);
        let position = cx
            .update_window(handle, |_, w, cx| {
                let bounds = w.find("candle-region").bounds();
                let p = point(bounds.center().x, bounds.bottom() - px(12.));
                // Start zoomed so reset and drag are observable too.
                w.dispatch_event(scroll(p, 0., 275.).to_platform_input(), cx);
                p
            })
            .unwrap();
        frames(&mut cx, handle);
        let before_label = chart_label(&mut cx, handle);
        let before_image = chart_image(&mut cx, handle);
        cx.update_window(handle, |_, w, cx| w.click("open-palette", cx))
            .unwrap();
        frames(&mut cx, handle);
        cx.update_window(handle, |_, w, cx| {
            assert!(
                !w.find("directory-palette").bounds().contains(&position),
                "test must hit the backdrop, not palette contents"
            );
            gesture_event(gesture, position, w, cx);
        })
        .unwrap();
        frames(&mut cx, handle);
        if chart_label(&mut cx, handle) != before_label {
            failures.push(format!(
                "overlay {gesture}: chart count changed beneath palette"
            ));
        }
        cx.update_window(handle, |_, w, cx| {
            if w.try_find("directory-palette").is_some() {
                w.dispatch_event(
                    gpui_kit::KeyDownEvent {
                        keystroke: gpui_kit::Keystroke::parse("escape").unwrap(),
                        is_held: false,
                        prefer_character_input: false,
                    }
                    .to_platform_input(),
                    cx,
                );
            }
        })
        .unwrap();
        frames(&mut cx, handle);
        if chart_image(&mut cx, handle) != before_image {
            failures.push(format!(
                "overlay {gesture}: visible chart changed beneath palette"
            ));
        }
        let closed_before = chart_image(&mut cx, handle);
        cx.update_window(handle, |_, w, cx| gesture_event(gesture, position, w, cx))
            .unwrap();
        frames(&mut cx, handle);
        if chart_image(&mut cx, handle) == closed_before {
            failures.push(format!("closed palette {gesture}: gesture stopped working"));
        }
    }
}
fn gesture_event(
    gesture: &str,
    position: gpui_kit::Point<gpui_kit::Pixels>,
    w: &mut Window,
    cx: &mut gpui_kit::App,
) {
    match gesture {
        "wheel" => {
            w.dispatch_event(scroll(position, 0., 275.).to_platform_input(), cx);
        }
        "pinch" => {
            w.dispatch_event(
                PinchEvent {
                    position,
                    delta: 0.8,
                    ..Default::default()
                }
                .to_platform_input(),
                cx,
            );
        }
        "drag" => {
            w.dispatch_event(
                MouseDownEvent {
                    position,
                    button: MouseButton::Left,
                    click_count: 1,
                    ..Default::default()
                }
                .to_platform_input(),
                cx,
            );
            w.dispatch_event(
                MouseMoveEvent {
                    position: position - point(px(80.), px(0.)),
                    pressed_button: Some(MouseButton::Left),
                    ..Default::default()
                }
                .to_platform_input(),
                cx,
            );
            w.dispatch_event(
                MouseUpEvent {
                    position,
                    button: MouseButton::Left,
                    ..Default::default()
                }
                .to_platform_input(),
                cx,
            );
        }
        "reset" => {
            w.dispatch_event(
                MouseDownEvent {
                    position,
                    button: MouseButton::Left,
                    click_count: 2,
                    ..Default::default()
                }
                .to_platform_input(),
                cx,
            );
        }
        _ => unreachable!(),
    }
}
fn blur_cancels_drag(failures: &mut Vec<String>) {
    let mut cx = context();
    let (handle, _) = cx
        .update(|cx| {
            gpui_kit::open_window(options(), cx, |w, cx| {
                twstock_gpui::theme::apply(ThemeMode::Dark, Some(w), cx);
                cx.new(|cx| Workspace::new(w, cx))
            })
        })
        .unwrap();
    frames(&mut cx, handle);
    cx.update_window(handle, |_, w, _| w.activate_window())
        .unwrap();
    frames(&mut cx, handle);
    assert!(
        cx.update_window(handle, |_, w, _| w.is_window_active())
            .unwrap()
    );
    let position = cx
        .update_window(handle, |_, w, cx| {
            let position = w.find("candle-region").bounds().center();
            w.dispatch_event(scroll(position, 0., 275.).to_platform_input(), cx);
            position
        })
        .unwrap();
    frames(&mut cx, handle);
    let before = chart_image(&mut cx, handle);
    cx.update_window(handle, |_, w, cx| {
        w.dispatch_event(
            MouseDownEvent {
                position,
                button: MouseButton::Left,
                click_count: 1,
                ..Default::default()
            }
            .to_platform_input(),
            cx,
        );
    })
    .unwrap();
    let (other, _) = cx
        .update(|cx| gpui_kit::open_window(options(), cx, |_, cx| cx.new(|_| FontWeights)))
        .unwrap();
    cx.update_window(other, |_, w, _| w.activate_window())
        .unwrap();
    frames(&mut cx, handle);
    assert!(
        !cx.update_window(handle, |_, w, _| w.is_window_active())
            .unwrap()
    );
    cx.update_window(handle, |_, w, _| w.activate_window())
        .unwrap();
    frames(&mut cx, handle);
    cx.update_window(handle, |_, w, cx| {
        w.dispatch_event(
            MouseMoveEvent {
                position: position - point(px(100.), px(0.)),
                pressed_button: Some(MouseButton::Left),
                ..Default::default()
            }
            .to_platform_input(),
            cx,
        );
    })
    .unwrap();
    frames(&mut cx, handle);
    if chart_image(&mut cx, handle) != before {
        failures.push("blur: stale drag continued after cancellation".into());
    }
}

fn zoom_during_drag(failures: &mut Vec<String>) {
    for pinch in [false, true] {
        for render_between in [false, true] {
            let store = MockProvider::new().store();
            let data = Rc::new(store.chart_series(&store.order[0]).unwrap());
            let viewport = Rc::new(ChartViewport::new(data.candles.len()));
            let mut cx = context();
            let (handle, _) = cx
                .update(|cx| {
                    gpui_kit::open_window(options(), cx, |w, cx| {
                        twstock_gpui::theme::apply(ThemeMode::Dark, Some(w), cx);
                        cx.new(|_| ZoomChart {
                            data: data.clone(),
                            viewport: viewport.clone(),
                        })
                    })
                })
                .unwrap();
            frames(&mut cx, handle);
            let (bounds, font) = cx
                .update_window(handle, |_, w, cx| {
                    (w.find("candle-region").bounds(), cx.theme().font_size)
                })
                .unwrap();
            let position = bounds.center();
            cx.update_window(handle, |_, w, cx| {
                w.dispatch_event(scroll(position, 0., 275.).to_platform_input(), cx);
            })
            .unwrap();
            frames(&mut cx, handle);
            assert_eq!(viewport.range.get().count, 40);
            cx.update_window(handle, |_, w, cx| {
                w.dispatch_event(
                    MouseDownEvent {
                        position,
                        button: MouseButton::Left,
                        click_count: 1,
                        ..Default::default()
                    }
                    .to_platform_input(),
                    cx,
                );
                if pinch {
                    w.dispatch_event(
                        PinchEvent {
                            position,
                            delta: 1.,
                            ..Default::default()
                        }
                        .to_platform_input(),
                        cx,
                    );
                } else {
                    w.dispatch_event(scroll(position, 0., 275.).to_platform_input(), cx);
                }
            })
            .unwrap();
            let zoomed = viewport.range.get();
            assert_eq!(zoomed.count, 20);
            if render_between {
                frames(&mut cx, handle);
            }
            let g = ChartGeometry::for_range(bounds, font, &data, None, zoomed);
            cx.update_window(handle, |_, w, cx| {
                w.dispatch_event(
                    MouseMoveEvent {
                        position: position - point(px(g.x.step() * 5.), px(0.)),
                        pressed_button: Some(MouseButton::Left),
                        ..Default::default()
                    }
                    .to_platform_input(),
                    cx,
                );
            })
            .unwrap();
            let expected = zoomed.pan_by(data.candles.len(), 5.);
            if viewport.range.get() != expected {
                failures.push(format!("zoom during drag (pinch={pinch}, redraw={render_between}): got {:?}, expected {expected:?}", viewport.range.get()));
            }
        }
    }
}
