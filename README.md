# twstock-gpui

[繁體中文](README_tw.md) · English

An **experimental** desktop watch app for Taiwan stocks, built for personal use. Native macOS in Rust with [gpui-kit](https://github.com/longbridge/gpui-kit) — not a web wrapper.

> **Experimental Project:** This repo is an ongoing experiment in building a native financial UI and strict visual/domain separation with GPUI. It relies on unofficial public web endpoints, has known coverage gaps, and is strictly for personal exploration — not a production trading terminal.

![Dark theme](tests/snapshots/dark.png)

*Screenshots show deterministic sample data (fictional instruments starting with `99`), not live market quotes. When no config file is present, the app starts in this sample mode so you can try it out of the box.*

## Workspace & Controls

The window is split into three resizable panels: **Watchlist** (with sparklines), **Chart**, and **Quote Detail** (5-level order book, buy/sell pressure, volume, and price position).

- **Charts & Market Conventions**
  - Custom-drawn candlesticks (solid when closing up, hollow when down — direction never relies on color alone), MA5 / MA10 / MA20, a volume sub-chart, high/low markers, a gold crosshair, and a last-price line.
  - Switch between **Day / Week / Month** bars. Weekly and monthly bars are aggregated directly from loaded daily history without extra network requests. (There is no free minute-bar source, so no disabled minute button is shown.)
  - Scroll vertically or pinch on the trackpad to zoom around the cursor; scroll horizontally or drag to pan. Double-click or switch instrument/period to reset the view.
  - Follows Taiwan market colors (**red up, green down**, flat in grey). Prices and quantities stay neutral; only price changes carry color. Includes both dark and light themes.

- **Keyboard Shortcuts**
  - `⌘K` — Open instrument search (listed and OTC stocks & ETFs). Filter by code prefix or case-insensitive name substring; `↑`/`↓` to select, `Enter` to add, `Esc` to close.
  - `Delete` / `Backspace` — Remove the selected row from the watchlist (also available via the `…` menu on the watchlist header).
  - `⌘⇧R` — Reset panel layout to default (disabled while the search dialog is open).

![Light theme](tests/snapshots/light.png)

## Getting Started

### Requirements

- **macOS with Metal** (verified on Apple Silicon).
- **Full Xcode** (not just Command Line Tools) with the Metal toolchain installed:
  ```sh
  sudo xcode-select -s /Applications/Xcode.app/Contents/Developer
  sudo xcodebuild -license accept
  xcodebuild -runFirstLaunch
  xcodebuild -downloadComponent MetalToolchain
  ```
- **[rustup](https://rustup.rs)** — `rust-toolchain.toml` pins Rust `1.97.1` and installs it automatically on first build.
- System `curl` (used for market data requests).

### Run & Bundle

Run inside a desktop session with a display (a window cannot be opened over plain SSH). The first build takes a few minutes:

```sh
git clone <this repo>
cd twstock-gpui
cargo run --release
```

`cargo run` sets the macOS Dock icon at startup from `assets/icon/icon-1024.png`. If you prefer a standalone `.app` bundle:

```sh
./scripts/bundle-app.sh   # produces target/bundle/twstock-gpui.app
```

- The bundle is unsigned; on first launch you may need to right-click → **Open** in Finder.
- The icon artwork (`assets/icon/icon-1024.png`) was generated with Gemini and cut out from its background. Run `./scripts/make-icon.sh` to regenerate `AppIcon.icns` using macOS's built-in `sips` and `iconutil`.

### Watchlist Configuration

Press `⌘K` to search and add your first instrument — when starting from sample mode, the app creates `~/.config/twstock-gpui/watchlist.toml` and switches to live MIS quotes (without copying sample symbols into your config). You can also create the file by hand:

```toml
[[watchlist]]
market = "tse"    # "tse" for listed (TWSE), "otc" for OTC (TPEx)
code = "889901"   # replace with the code you want to watch

[[watchlist]]
market = "tse"
code = "889902"
```

- **Preserves formatting:** Adding or removing symbols in the UI updates the file atomically via `toml_edit`, keeping your comments, formatting, inline arrays, and table aliases (`[[watchlist]]`, `[[symbol]]`, `[[symbols]]`, `[[instruments]]`). Changes take effect on the next polling round (at least 5 seconds between rounds) without restarting.
- **No silent fallback:** Missing config uses sample data. A malformed config displays the parse error instead of quietly falling back to sample data. Emptying your watchlist saves an empty list and stays empty across restarts.

---

## Data Sources & Honest States

A core principle of this codebase is **never inventing data to fill the screen**:

- **Connection status** is always visible in the status bar (`connecting`, `ok`, `stale`, `failed`, `market closed`). A dropped connection is never disguised as a flat price.
- **Missing trades:** When an instrument has no trade price yet, no last-price line is drawn and the quote explicitly marks the reference price. Mid-quote reference prices are never used to fabricate daily bars.
- **Explicit loading & errors:** Loading states use a centered spinner; failures show the reason; empty or too-narrow panels state why in the center of the canvas. While recent daily bars are still loading, the UI never flashes a misleading "0 daily bars".

| Use | Source | Behavior & Limits |
|---|---|---|
| **Live quotes & order book** | TWSE MIS (`mis.twse.com.tw`) | Unofficial endpoint, no guarantees. Updates roughly every 5 s; the app polls no faster than every 5 s. No tick-by-tick trade feed (the trade-list section collapses completely when unsupported). The trade price field is often `-` between matches. |
| **Daily history** | TWSE `STOCK_DAY` | Fetched one month at a time, back-filled up to 100 bars (at most 12 months back). **Listed (TSE) only**; OTC history is not wired up yet. |
| **Instrument directory** | TWSE ISIN lists (`strMode=2` listed, `strMode=4` OTC) | Includes only the official "stock" and "ETF" categories. Tick-size rules are determined strictly from the official category, never guessed from code prefixes. |

**Caching & Backoff:**
- **Daily history** is cached per instrument under `~/.cache/twstock-gpui/history/`. A past month is treated as final once fetched after that month ended (any mid-month snapshot of a past month is refetched); the current month's cache is reused until a newer weekday session has closed (14:00 Taipei time). Error responses are never cached, and corrupt cache files fall back to the network. The active symbol loads first while others queue in the background with at least 2 seconds between requests. Failed loads retry with exponential backoff (60 s up to 10 min), or immediately when you re-select the symbol.
- **Instrument directory** is cached at `~/.cache/twstock-gpui/directory.json` and refreshed after 24 hours (also checked when reopening search). If a refresh fails, the error is shown rather than serving stale or fake data.

> **Disclaimer:** For personal use only. Not investment advice, and data is not guaranteed to be accurate or real-time. Please do not raise the polling frequency.

---

## Chart & Metric Definitions

![Crosshair](tests/snapshots/crosshair.png)

### Volume Display & Spike Clipping
- **Lot formatting:** Volume figures are rounded to whole lots with thousands separators, while axis ticks use `k` / `M` abbreviations. Hovering a bar highlights its exact volume on the right axis; without a cursor, the latest volume is shown faintly. Date, price, and volume badges avoid colliding with axis ticks.
- **Spike clipping:** To keep a single outlier day from flattening every other volume bar, the sub-chart applies a ceiling of `nice(P95 × 1.2)` — but **only** when the visible maximum strictly exceeds `2.5×` that ceiling (P95 uses nearest-rank `ceil(0.95 × count)` including zeros; series under 20 bars are never clipped). Clipped bars render with a break mark, a true-volume label, and a "clipped at N" note in the header, while the crosshair still reads the unclipped volume.
- **Weekly & monthly aggregation:** Weeks group by the trading day's ISO week (using ISO week-year across year boundaries); months group by `YYYY/MM`. Today's live bar is folded into the current unfinished period. MA5 / 10 / 20 are recomputed for the active period and display `—` when bars are insufficient.

![Weekly chart](tests/snapshots/weekly.png)

![Monthly chart](tests/snapshots/monthly.png)

### The "Position" Block (Below the Order Book)
- **Today's range bar:** Spans the min/max of known low, high, previous close, open, and trade price, padded by 8% on each side (at least one tick so flat days remain readable). A previous close outside today's high/low is included in the span; a reference price is not. Labels measure their actual text width to spread apart and connect back to their price level with leader lines. Limit-down and limit-up prices sit at the two ends below the axis so they don't compress intraday movement.
- **Recent range percentile:** Computed strictly from the low/high of loaded daily bars (showing the exact bar count), unaffected by switching the chart to weekly or monthly view. Clamped to `0–100%` (`50%` if high equals low at the trade price; `—` if there is no trade price).
- **Volume ratio (Today / 5-day average):** Divides today's volume by the average of the 5 most recent *complete* daily bars prior to the quote date (today's unfinished bar is never mixed into the denominator). Labeled "intraday cumulative" during the session and "full-day cumulative" after close — no time-of-day extrapolation is performed, and the UI draws no surge/dry-up conclusions for you.

---

## Architecture & Verification

```text
src/
  design/    Single source of visual truth: palette, typography, up/down direction, spacing, radii, shared parts, charts
  market/    Domain types, MarketStore, MIS & history providers, config persistence (no gpui dependency)
  panels/    Watchlist, chart, quote, and status bar panels (delegates all visual styling to design)
tests/
  design_rules.rs          AST/token scanner that fails if code outside src/design hard-codes colors, sizes, weights, spacing, or radii
  provider_conformance.rs  Shared behavioral contract tests for all market providers
  rendering.rs             GPU off-screen rendering & golden snapshot tests
  live_shot.rs             Opt-in smoke test against live TWSE endpoints
```

- **Strict visual boundary:** Enforced by `design_rules`. Panels compose semantic components from `design` rather than choosing colors or pixel spacing ad hoc.
- **Decoupled market interface:** The UI talks only to domain types and `MarketStore` (prices use `0.0001` fixed-point representation; tick rules belong to the instrument). Adding a broker API or WebSocket feed only requires implementing a provider that passes `provider_conformance`.

### Running Tests

```sh
cargo clippy --all-targets -- -D warnings
cargo test
cargo test --test rendering
```

- **Off-screen GPU rendering (`rendering`):** Renders headless on the GPU to `/tmp/twstock-gpui-*.png` without opening a window (works over SSH, uses fixed sample data, never touches your config or the network). Verifies layout geometry, panel dragging/resizing, search, keyboard navigation, and compares 10 static scenes against golden PNGs in `tests/snapshots/` (2×2 box-filtered, ignoring per-channel diffs ≤ 2, failing above 0.05% differing pixels).
  - To update golden snapshots after visual changes: `UPDATE_SNAPSHOTS=1 cargo test --test rendering`, then inspect the diffs under `/tmp/twstock-gpui-snapshot/`.
- **Live network smoke tests (off by default):**
  ```sh
  # Fetches live data for your watchlist.toml and renders /tmp/twstock-gpui-live-{dark,light}.png
  TWSTOCK_GPUI_LIVE=1 cargo test --test live_shot

  # Verifies both TWSE/OTC directory endpoints directly and prints counts & latency
  TWSTOCK_GPUI_LIVE=1 cargo test --test directory directory_live_opt_in -- --nocapture
  ```

## Known Limitations

- **Market calendar:** Market closure is inferred from Taipei time, weekends, and the quote's date; the official TWSE holiday calendar is not integrated yet.
- **OTC history & history depth:** OTC daily history is not wired up yet. Listed daily history loads up to 100 bars (at most 12 months back), which yields roughly 20+ weekly bars and 5–6 monthly bars — meaning monthly MA10 / MA20 will normally show `—`.
- **Instrument scope:** Search covers official "stock" and "ETF" categories only; warrants, preferred shares, Innovation Board, ETNs, TDRs, and beneficiary securities are excluded so tick sizes are never guessed.
- **Platform:** Tested on macOS (Apple Silicon) only, without multi-day soak testing.

## License

Code in this repository is licensed under [Apache-2.0](LICENSE) (as is `gpui-kit`).
TWSE market data usage and redistribution are subject to TWSE's own terms and are not covered by this repo's license; check with TWSE before any commercial use.
