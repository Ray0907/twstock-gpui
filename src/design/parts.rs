use super::{
    color::{self, ColorRole},
    direction::Direction,
    space::{self, Dimension, Space},
    text::{self, TextRole},
};
use crate::quotes::{Quote, format_price};
use gpui_kit::base::ObservedElement;
use gpui_kit::component::{
    ActiveTheme, IconName, Sizable, Size, TitleBar,
    button::{Button, ButtonVariants},
    chart::LineChart,
    dock::{BasePanelView, DockArea, DockLayout, DockSkin},
    h_flex,
    status_bar::StatusBar,
    tab::{Tab, TabBar},
    table::{Column, DataTable, TableDelegate, TableState},
    tooltip::Tooltip,
    v_flex,
};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;

pub fn panel_header(label: impl Into<SharedString>, cx: &App) -> Div {
    text::text(TextRole::Label, label, cx)
}
pub fn dock(window: &mut Window, cx: &mut App) -> (Entity<DockArea>, std::rc::Rc<DockSkin>) {
    let (dock, skin) = DockSkin::dock_area("trading-dock", None, window, cx);
    skin.set_toggle_button_visible(false, cx);
    (dock, skin)
}
pub fn default_dock_layout(
    panels: [std::sync::Arc<dyn BasePanelView>; 3],
    window: &Window,
    cx: &App,
) -> DockLayout {
    panels.into_iter().zip(space::dock_sizes(window, cx)).fold(
        DockLayout::h_split(),
        |layout, (panel, width)| {
            layout.child(DockLayout::tabs().panel_view(panel, cx), Some(width))
        },
    )
}
pub fn workspace(cx: &App) -> Stateful<Div> {
    v_flex()
        .id("workspace")
        .size_full()
        .bg(color::get(ColorRole::Background, cx))
        .text_color(color::get(ColorRole::Foreground, cx))
}
pub fn workspace_body() -> Div {
    h_flex().flex_1().min_h_0().items_stretch()
}
pub fn title_bar(cx: &App) -> TitleBar {
    TitleBar::new()
        .child(text::text(TextRole::Title, "台股看盤", cx))
        .child(div().flex_1())
}
pub fn reset_button() -> Button {
    Button::new("reset-layout")
        .ghost()
        .small()
        .icon(IconName::LayoutDashboard)
        .accessibility_label(SharedString::from("重設版面"))
        .tooltip(SharedString::from("重設版面"))
}
pub fn watch_button() -> Button {
    Button::new("nav-watchlist")
        .ghost()
        .small()
        .icon(IconName::PanelLeft)
        .accessibility_label(SharedString::from("自選"))
        .tooltip(SharedString::from("自選"))
}
pub fn pane(id: &'static str) -> ObservedElement<Stateful<Div>> {
    v_flex()
        .id(id)
        .test_support()
        .size_full()
        .min_w_0()
        .min_h_0()
        .overflow_hidden()
}
pub const TRADES_DEFAULT_OPEN: bool = true;
pub fn quote_pane() -> ObservedElement<Stateful<Div>> {
    pane("quote-panel").overflow_y_scroll()
}
pub fn quote_summary(quote: &Quote, share: f32, cx: &App) -> Div {
    v_flex()
        .flex_shrink_0()
        .child(quote_header(quote, cx))
        .child(quote_metrics(quote, cx))
        .child(depth_ratio(share, cx))
        .child(depth_book(quote, cx))
        .child(position_section(quote, cx))
}
pub fn position_section(quote: &Quote, cx: &App) -> ObservedElement<Stateful<Div>> {
    let p = &quote.position;
    let price = |p: Option<crate::market::Price>| {
        p.map_or_else(|| "—".into(), |p| format_price(p.0 as f64 / 10000.))
    };
    let today = if quote.is_reference {
        "參考價 · 不標現價"
    } else if !quote.last.is_finite() {
        "尚無成交價"
    } else {
        "今日區間"
    };
    let recent = format!("近期區間 · {} 根日線", p.recent_count);
    let percentile = p
        .recent_percentile
        .map_or_else(|| "—".into(), |v| format!("{v:.0}%"));
    let volume = format!(
        "量比 · {}",
        if p.intraday {
            "盤中累計"
        } else {
            "全日累計"
        }
    );
    v_flex()
        .id("position-section")
        .test_support()
        .p(Space::Small.rem())
        .gap(Space::Micro.rem())
        .border_b_1()
        .border_color(color::get(ColorRole::Border, cx))
        .child(
            h_flex()
                .justify_between()
                .child(panel_header("位置", cx))
                .child(
                    panel_header(today, cx)
                        .id("position-today")
                        .aria_label(today)
                        .test_support(),
                ),
        )
        .child(
            div()
                .id("position-plot")
                .test_support()
                .h(rems(3.))
                .child(super::position::PositionPlot::new(quote)),
        )
        .child(
            h_flex()
                .justify_between()
                .child(
                    text::text(
                        TextRole::Caption,
                        format!("跌停 {}", format_price(quote.limit_down)),
                        cx,
                    )
                    .id("position-limit-down")
                    .aria_label(format!("跌停 {}", format_price(quote.limit_down)))
                    .test_support(),
                )
                .child(
                    text::text(
                        TextRole::Caption,
                        format!("漲停 {}", format_price(quote.limit_up)),
                        cx,
                    )
                    .id("position-limit-up")
                    .aria_label(format!("漲停 {}", format_price(quote.limit_up)))
                    .test_support(),
                ),
        )
        .child(
            text::text(TextRole::Caption, recent.clone(), cx)
                .id("position-recent")
                .aria_label(recent)
                .test_support(),
        )
        .child(
            h_flex()
                .justify_between()
                .child(text::text(
                    TextRole::Body,
                    format!("{} — {}", price(p.recent_low), price(p.recent_high)),
                    cx,
                ))
                .child(text::text(TextRole::Body, percentile, cx)),
        )
        .child(
            text::text(TextRole::Caption, volume.clone(), cx)
                .id("position-volume")
                .aria_label(volume)
                .test_support(),
        )
        .child(
            h_flex()
                .justify_between()
                .child(text::text(TextRole::Caption, "累計量 / 前 5 日均日量", cx))
                .child(text::text(
                    TextRole::Body,
                    p.volume_ratio
                        .map_or_else(|| "—".into(), |v| format!("{v:.2}×")),
                    cx,
                )),
        )
}
pub type RemoveHandler = std::rc::Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>;
pub fn watch_menu(
    menu: gpui_kit::component::menu::PopupMenu,
    handler: RemoveHandler,
) -> gpui_kit::component::menu::PopupMenu {
    menu.item(
        gpui_kit::component::menu::PopupMenuItem::new("移除選取（Delete / Backspace）")
            .on_click(move |e, w, cx| handler(e, w, cx)),
    )
}
pub fn watch_columns(cx: &App) -> Vec<Column> {
    let widths = space::watch_widths(Dimension::Watchlist.rem().to_pixels(cx.theme().font_size));
    ["名稱", "成交 / 幅度", "走勢"]
        .into_iter()
        .enumerate()
        .map(|(ix, name)| {
            let column = Column::new(["stock", "price", "spark"][ix], name)
                .width(widths[ix])
                .resizable(false);
            if ix == 2 {
                column.paddings(Edges {
                    top: Space::Tight.rem().to_pixels(cx.theme().font_size),
                    bottom: Space::Tight.rem().to_pixels(cx.theme().font_size),
                    left: Space::Tight.rem().to_pixels(cx.theme().font_size),
                    right: Space::Tight.rem().to_pixels(cx.theme().font_size),
                })
            } else {
                column.paddings(Edges {
                    top: Space::Micro.rem().to_pixels(cx.theme().font_size),
                    bottom: Space::Micro.rem().to_pixels(cx.theme().font_size),
                    left: Space::Small.rem().to_pixels(cx.theme().font_size),
                    right: Space::Small.rem().to_pixels(cx.theme().font_size),
                })
            }
        })
        .collect()
}
pub fn watch_table<D: TableDelegate>(state: &Entity<TableState<D>>, cx: &App) -> DataTable<D> {
    DataTable::new(state)
        .bordered(false)
        .stripe(false)
        .with_size(Size::Size(
            Dimension::WatchRow.rem().to_pixels(cx.theme().font_size),
        ))
        .header_row_height(Dimension::WatchHeader.rem().to_pixels(cx.theme().font_size))
}
pub fn watch_column_header(name: SharedString, col: usize, cx: &App) -> Div {
    let label = panel_header(name, cx).w_full();
    if col == 1 { label.text_right() } else { label }
}
pub fn stock_name(name: impl Into<SharedString>, selected: bool, cx: &App) -> Div {
    text::text(TextRole::Title, name, cx).text_color(color::get(
        if selected {
            ColorRole::Focus
        } else {
            ColorRole::Foreground
        },
        cx,
    ))
}
pub fn watch_name(quote: &Quote, selected: bool, cx: &App) -> ObservedElement<Stateful<Div>> {
    v_flex()
        .id(SharedString::from(format!(
            "watch-name-cell-{}",
            quote.code
        )))
        .test_support()
        .min_w_0()
        .w_full()
        .child(
            stock_name(quote.name.clone(), selected, cx)
                .id(SharedString::from(format!("watch-name-{}", quote.code)))
                .test_support()
                .truncate()
                .tooltip({
                    let name = quote.name.clone();
                    move |window, cx| {
                        Tooltip::element({
                            let name = name.clone();
                            move |_, cx| {
                                text::text(TextRole::Title, name.clone(), cx)
                                    .id("watch-full-name")
                                    .test_support()
                                    .role(gpui_kit::accesskit::Role::Label)
                                    .aria_label(name.clone())
                            }
                        })
                        .build(window, cx)
                    }
                }),
        )
        .child(
            h_flex()
                .gap(Space::Tight.rem())
                .child(text::text(TextRole::Caption, quote.code.clone(), cx))
                .child(text::text(TextRole::Label, quote.market.label(), cx)),
        )
}
fn quote_price(role: TextRole, quote: &Quote, cx: &App) -> Div {
    let text = text::text(role, format_price(quote.last), cx);
    if quote.is_reference {
        text.text_color(color::get(ColorRole::Reference, cx))
    } else {
        text
    }
}
fn quote_change(quote: &Quote, percent: bool, cx: &App) -> Div {
    let value = if percent {
        quote.change_percent()
    } else {
        quote.change()
    };
    if quote.is_reference {
        let value = if value.is_finite() {
            let value = format!("{value:.2}");
            format!(
                "{}{}",
                value.trim_end_matches('0').trim_end_matches('.'),
                if percent { "%" } else { "" }
            )
        } else {
            "—".into()
        };
        text::text(TextRole::Caption, value, cx).text_color(color::get(ColorRole::Reference, cx))
    } else if !quote.last.is_finite() && quote.has_traded {
        panel_header("成交價暫缺", cx)
    } else {
        Direction::of(quote.change()).change(value, percent, cx)
    }
}
fn reference_badge(id: impl Into<ElementId>, cx: &App) -> ObservedElement<Stateful<Div>> {
    text::text(TextRole::Label, "參考", cx)
        .text_color(color::get(ColorRole::Reference, cx))
        .whitespace_nowrap()
        .id(id)
        .test_support()
}
pub fn watch_price(quote: &Quote, row: usize, cx: &App) -> ObservedElement<Stateful<Div>> {
    v_flex()
        .id(("watch-price", row))
        .test_support()
        .w_full()
        .items_end()
        .child(
            quote_price(TextRole::Value, quote, cx)
                .id(("watch-last", row))
                .test_support()
                .w_full()
                .text_right(),
        )
        .child(
            h_flex()
                .justify_end()
                .gap(Space::Micro.rem())
                .when(quote.is_reference, |this| {
                    this.child(reference_badge(("watch-reference", row), cx))
                })
                .child(quote_change(quote, true, cx)),
        )
}
fn spark_extent(data: &[(usize, f64)]) -> (f64, f64) {
    let low = data.iter().map(|d| d.1).fold(f64::INFINITY, f64::min);
    let high = data.iter().map(|d| d.1).fold(f64::NEG_INFINITY, f64::max);
    let padding = (high - low).max(0.01) * 0.05;
    (low - padding, high + padding)
}

fn spark_data(quote: &Quote) -> Vec<(usize, f64)> {
    quote
        .closes
        .iter()
        .rev()
        .take(11)
        .rev()
        .copied()
        .chain((!quote.is_reference).then_some(quote.last))
        .filter(|value| value.is_finite())
        .enumerate()
        .collect()
}
pub fn watch_sparkline(quote: &Quote, row: usize, cx: &App) -> ObservedElement<Stateful<Div>> {
    let data = spark_data(quote);
    let root = div()
        .id(("watch-sparkline", row))
        .test_support()
        .w_full()
        .h(Dimension::DataRow.rem());
    if data.len() < 2 {
        return root;
    }
    let (low, high) = spark_extent(&data);
    root.child(
        LineChart::new(data)
            .id(("spark", row))
            .x(|d: &(usize, f64)| SharedString::from(d.0.to_string()))
            .y(|d| d.1)
            .stroke(if quote.is_reference {
                color::get(ColorRole::Reference, cx)
            } else {
                Direction::of(quote.change()).color(cx)
            })
            .y_domain(low, high)
            .y_padding(
                Space::Micro.rem().to_pixels(cx.theme().font_size).as_f32(),
                Space::Micro.rem().to_pixels(cx.theme().font_size).as_f32(),
            )
            .linear()
            .x_axis(false)
            .y_axis(false)
            .grid(false)
            .interactive(false)
            .appear(false),
    )
}
pub fn chart_header(quote: &Quote, cx: &App) -> Div {
    v_flex()
        .border_b_1()
        .border_color(color::get(ColorRole::Border, cx))
        .child(
            h_flex()
                .px(Space::Small.rem())
                .py(Space::Small.rem())
                .gap(Space::Small.rem())
                .child(text::text(TextRole::Title, quote.name.clone(), cx))
                .child(text::text(TextRole::Caption, quote.code.clone(), cx))
                .child(div().flex_1())
                .child(
                    quote_price(TextRole::Value, quote, cx)
                        .id("chart-last")
                        .test_support(),
                )
                .when(quote.is_reference, |this| {
                    this.child(reference_badge("chart-reference", cx))
                })
                .child(quote_change(quote, true, cx)),
        )
}
pub fn ohlc_row(quote: &Quote, cx: &App) -> ObservedElement<Stateful<Div>> {
    h_flex()
        .id("ohlc-row")
        .test_support()
        .px(Space::Small.rem())
        .gap(Space::Large.rem())
        .children(
            [
                ("開", quote.open),
                ("高", quote.high),
                ("低", quote.low),
                ("收", quote.last),
            ]
            .into_iter()
            .enumerate()
            .map(|(ix, (label, value))| {
                h_flex()
                    .id(("ohlc-group", ix))
                    .test_support()
                    .gap(Space::Tight.rem())
                    .py(Space::Tight.rem())
                    .child(
                        panel_header(label, cx)
                            .id(("ohlc-label", ix))
                            .test_support(),
                    )
                    .child(
                        text::text(TextRole::Body, format_price(value), cx)
                            .id(("ohlc", ix))
                            .test_support(),
                    )
            }),
        )
}
pub fn periods(
    selected: usize,
    on_click: impl Fn(&usize, &mut Window, &mut App) + 'static,
    cx: &App,
) -> TabBar {
    TabBar::new("chart-periods")
        .selected_index(selected)
        .on_click(on_click)
        .children(
            ["日", "週", "月"]
                .into_iter()
                .enumerate()
                .map(|(i, label)| {
                    Tab::new().child(
                        panel_header(label, cx)
                            .when(i == selected, |this| {
                                this.text_color(color::get(ColorRole::Focus, cx))
                            })
                            .id(["period-day", "period-week", "period-month"][i])
                            .test_support(),
                    )
                }),
        )
}
pub fn quote_header(quote: &Quote, cx: &App) -> Div {
    v_flex()
        .p(Space::Small.rem())
        .gap(Space::Small.rem())
        .border_b_1()
        .border_color(color::get(ColorRole::Border, cx))
        .child(
            h_flex()
                .gap(Space::Small.rem())
                .child(text::text(TextRole::Title, quote.name.clone(), cx))
                .child(text::text(TextRole::Caption, quote.code.clone(), cx))
                .child(panel_header(quote.market.label(), cx)),
        )
        .child(
            h_flex()
                .justify_between()
                .items_center()
                .child(
                    h_flex()
                        .gap(Space::Micro.rem())
                        .items_center()
                        .child(
                            quote_price(TextRole::Hero, quote, cx)
                                .id("quote-last")
                                .test_support(),
                        )
                        .when(quote.is_reference, |this| {
                            this.child(reference_badge("quote-reference", cx))
                        }),
                )
                .child(
                    v_flex()
                        .items_end()
                        .child(quote_change(quote, false, cx))
                        .child(quote_change(quote, true, cx)),
                ),
        )
}
pub fn stat_cell(label: &'static str, value: impl Into<SharedString>, ix: usize, cx: &App) -> Div {
    v_flex()
        .flex_1()
        .min_w_0()
        .items_end()
        .child(panel_header(label, cx))
        .child(
            text::text(TextRole::Body, value, cx)
                .id(("quote-stat", ix))
                .test_support(),
        )
}
pub fn quote_metrics(quote: &Quote, cx: &App) -> Div {
    let metrics = [
        ("開", format_price(quote.open).into()),
        ("高", format_price(quote.high).into()),
        ("低", format_price(quote.low).into()),
        ("昨收", format_price(quote.prev_close).into()),
        (
            "成交量",
            SharedString::from(if quote.volume_known {
                format!("{} 張", quote.volume)
            } else {
                "—".into()
            }),
        ),
        (
            "來源",
            SharedString::from(if quote.has_trades {
                "範例資料"
            } else {
                "MIS"
            }),
        ),
    ];
    let mut grid = v_flex()
        .p(Space::Small.rem())
        .gap(Space::Small.rem())
        .border_b_1()
        .border_color(color::get(ColorRole::Border, cx));
    for (row, chunk) in metrics.chunks(3).enumerate() {
        grid = grid.child(
            h_flex().gap(Space::Small.rem()).children(
                chunk
                    .iter()
                    .enumerate()
                    .map(|(ix, (label, value))| stat_cell(label, value.clone(), row * 3 + ix, cx)),
            ),
        );
    }
    if quote.near_limits() {
        grid = grid.child(
            h_flex()
                .id("limits-section")
                .test_support()
                .gap(Space::Small.rem())
                .child(stat_cell("漲停", format_price(quote.limit_up), 6, cx))
                .child(stat_cell("跌停", format_price(quote.limit_down), 7, cx))
                .child(div().flex_1()),
        );
    }
    grid
}
pub fn depth_ratio(share: f32, cx: &App) -> Div {
    if !share.is_finite() {
        return panel_header("尚無五檔資料", cx);
    }
    v_flex()
        .px(Space::Small.rem())
        .py(Space::Small.rem())
        .gap(Space::Tight.rem())
        .border_b_1()
        .border_color(color::get(ColorRole::Border, cx))
        .child(
            h_flex()
                .justify_between()
                .child(panel_header(format!("買 {:.1}%", share * 100.), cx))
                .child(panel_header(format!("賣 {:.1}%", (1. - share) * 100.), cx)),
        )
        .child(
            h_flex()
                .h(Space::Tight.rem())
                .child(
                    div()
                        .w(relative(share))
                        .h_full()
                        .bg(Direction::Up.color(cx).opacity(0.65)),
                )
                .child(
                    div()
                        .w(relative(1. - share))
                        .h_full()
                        .bg(Direction::Down.color(cx).opacity(0.65)),
                ),
        )
}
fn depth_cell(content: impl IntoElement) -> Div {
    div().flex_1().min_w_0().w_0().text_right().child(content)
}
fn quantity_bar(volume: Option<u64>, maximum: u64, direction: Direction, cx: &App) -> Div {
    div()
        .relative()
        .h(Dimension::DataRow.rem())
        .child(
            div()
                .absolute()
                .top_0()
                .bottom_0()
                .right_0()
                .w(relative(volume.unwrap_or(0) as f32 / maximum.max(1) as f32))
                .bg(color::get(
                    if direction == Direction::Up {
                        ColorRole::DepthUp
                    } else {
                        ColorRole::DepthDown
                    },
                    cx,
                )),
        )
        .child(
            text::text(
                TextRole::Body,
                volume.map_or_else(|| "—".into(), |v| v.to_string()),
                cx,
            )
            .relative()
            .w_full()
            .text_right(),
        )
}
pub fn depth_book(quote: &Quote, cx: &App) -> Div {
    let max = quote
        .bids
        .iter()
        .chain(quote.asks.iter())
        .map(|(_, qty)| *qty)
        .max()
        .unwrap_or(1);
    v_flex()
        .p(Space::Small.rem())
        .gap(Space::Tight.rem())
        .border_b_1()
        .border_color(color::get(ColorRole::Border, cx))
        .child(panel_header("五檔", cx))
        .child(h_flex().gap(Space::Small.rem()).children(
            ["買量", "買價", "賣價", "賣量"].map(|label| depth_cell(panel_header(label, cx))),
        ))
        .children((0..5).map(|ix| {
            h_flex()
                .gap(Space::Small.rem())
                .child(depth_cell(quantity_bar(
                    quote.bids.get(ix).map(|b| b.1),
                    max,
                    Direction::Up,
                    cx,
                )))
                .child(depth_cell(text::text(
                    TextRole::Body,
                    quote
                        .bids
                        .get(ix)
                        .map_or_else(|| "—".into(), |b| format_price(b.0)),
                    cx,
                )))
                .child(depth_cell(text::text(
                    TextRole::Body,
                    quote
                        .asks
                        .get(ix)
                        .map_or_else(|| "—".into(), |b| format_price(b.0)),
                    cx,
                )))
                .child(depth_cell(quantity_bar(
                    quote.asks.get(ix).map(|b| b.1),
                    max,
                    Direction::Down,
                    cx,
                )))
        }))
}
pub fn trade_toggle(open: bool, cx: &App) -> Button {
    Button::new("trades-toggle")
        .ghost()
        .small()
        .child(panel_header("逐筆成交", cx))
        .icon(if open {
            IconName::ChevronDown
        } else {
            IconName::ChevronRight
        })
        .accessibility_label(SharedString::from(if open {
            "收合逐筆成交"
        } else {
            "展開逐筆成交"
        }))
}
pub fn trade_feed(quote: &Quote, cx: &App) -> ObservedElement<Stateful<Div>> {
    let row = || h_flex().gap(Space::Small.rem()).h(Dimension::DataRow.rem());
    v_flex()
        .id("trade-feed")
        .test_support()
        .flex_1()
        .min_h(Dimension::DataRow.rem())
        .overflow_y_scroll()
        .px(Space::Small.rem())
        .pb(Space::Small.rem())
        .child(
            row()
                .child(panel_header("時間", cx).w(Dimension::Axis.rem()))
                .child(panel_header("成交", cx).flex_1().text_right())
                .child(
                    panel_header("張", cx)
                        .w(Dimension::Quantity.rem())
                        .text_right(),
                )
                .child(
                    panel_header("向", cx)
                        .w(Dimension::Marker.rem())
                        .text_right(),
                ),
        )
        .children(quote.trades.iter().enumerate().map(|(ix, trade)| {
            row()
                .child(
                    text::text(TextRole::Caption, trade.time.clone(), cx).w(Dimension::Axis.rem()),
                )
                .child(
                    text::text(TextRole::Body, format_price(trade.price), cx)
                        .id(("trade-price", ix))
                        .test_support()
                        .flex_1()
                        .text_right(),
                )
                .child(
                    text::text(TextRole::Body, trade.quantity.to_string(), cx)
                        .w(Dimension::Quantity.rem())
                        .text_right(),
                )
                .child(
                    Direction::of(trade.change)
                        .marker(cx)
                        .w(Dimension::Marker.rem())
                        .text_right(),
                )
        }))
}
pub fn status_bar(markets: Vec<(String, String, f64)>, status: String, cx: &App) -> StatusBar {
    StatusBar::new()
        .left(
            h_flex().children(markets.into_iter().enumerate().map(
                |(ix, (name, value, change))| {
                    let lane = h_flex()
                        .w(Dimension::StatusLane.rem())
                        .gap(Space::Small.rem())
                        .pr(Space::Small.rem())
                        .child(panel_header(name, cx))
                        .child(div().flex_1())
                        .child(
                            text::text(TextRole::Value, value, cx)
                                .id(("index-last", ix))
                                .test_support(),
                        )
                        .child(Direction::of(change).change(change, true, cx));
                    if ix < 2 {
                        lane.border_r_1()
                            .border_color(color::get(ColorRole::Border, cx))
                    } else {
                        lane
                    }
                },
            )),
        )
        .right(
            panel_header(status, cx)
                .id("market-link-status")
                .test_support(),
        )
}

#[cfg(test)]
mod tests {
    use super::{spark_extent, stock_name};
    use crate::design::color::{self, ColorRole};
    use gpui_kit::component::{ActiveTheme, ThemeMode};
    use gpui_kit::{Styled, TestAppContext};

    #[gpui_kit::test]
    fn focus_roles_preserve_neutral_prices(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_kit::init(cx);
            for mode in [ThemeMode::Dark, ThemeMode::Light] {
                crate::theme::apply(mode, None, cx);
                assert_eq!(color::get(ColorRole::Focus, cx), cx.theme().link);
                assert_eq!(color::get(ColorRole::FocusFill, cx), cx.theme().primary);
                assert_eq!(
                    color::get(ColorRole::FocusInk, cx),
                    cx.theme().primary_foreground
                );
                assert_eq!(
                    stock_name("範例", true, cx).style().text.color,
                    Some(cx.theme().link)
                );
                assert_eq!(
                    stock_name("範例", false, cx).style().text.color,
                    Some(cx.theme().foreground)
                );
            }
        });
    }
    #[test]
    fn sparkline_does_not_invent_a_price_when_last_is_missing() {
        let mut q = crate::quotes::sample().remove(0);
        q.last = f64::NAN;
        q.closes.clear();
        assert!(super::spark_data(&q).is_empty());
        q.closes = vec![124., 125.];
        assert_eq!(super::spark_data(&q), vec![(0, 124.), (1, 125.)]);
    }
    #[test]
    fn sparkline_range_preserves_variation_and_flat_prices() {
        let (low, high) = spark_extent(&[(0, 124.), (1, 130.), (2, 128.)]);
        assert!(low < 124. && high > 130. && high - low < 10.);
        let (low, high) = spark_extent(&[(0, 124.), (1, 124.)]);
        assert!(low < 124. && high > 124.);
    }
}
