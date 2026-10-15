# twstock-gpui

台股看盤桌面程式，自用。原生 macOS，用 [gpui-kit](https://github.com/longbridge/gpui-kit) 的 GPUI 元件寫成，不是網頁。

![深色主題，範例資料](docs/screenshots/dark.png)

截圖用的是編造的範例資料（代號 99 開頭的虛構商品），不是真實行情。

## 功能

- 三欄工作區：自選（含迷你走勢線）、走勢圖、行情（五檔、買賣比例、成交量）。面板可拖曳與縮放。
- 走勢圖用 gpui-kit 的 plot 元件自繪：蠟燭（收高實心、收低空心，不只靠顏色分方向）、MA5 / MA10 / MA20、成交量副圖、金色十字游標與軸標籤、最高最低價標註、現價線。
- 紅漲綠跌（台股慣例），平盤為灰。價格與數字一律中性色，只有漲跌上色。
- 深色與淺色主題。
- 連線狀態一直看得到：連線中、正常、過期、失敗、休市。斷線不會被顯示成平盤。

![十字游標](docs/screenshots/crosshair.png)

## 資料來源

| 用途 | 來源 | 限制 |
|---|---|---|
| 即時報價與五檔 | 證交所 MIS（`mis.twse.com.tw`） | 非官方端點，無保證。約 5 秒更新一次，程式輪詢不少於 5 秒。沒有逐筆成交。成交價欄位常為 `-`。 |
| 歷史日線 | 證交所 `STOCK_DAY` | 一次一個月，往前補到 100 根。目前只支援上市，上櫃歷史尚未接。 |

只供個人看盤。不是投資建議，也不保證資料正確或即時。請不要提高輪詢頻率。

沒有設定檔時使用固定種子的範例資料，所以在任何機器上都可以直接試。

## 環境需求

- macOS，需要 Metal。只在 Apple Silicon 上驗證過。
- 完整的 Xcode（不是只有 Command Line Tools），並安裝 Metal toolchain：

  ```sh
  sudo xcode-select -s /Applications/Xcode.app/Contents/Developer
  sudo xcodebuild -license accept
  xcodebuild -runFirstLaunch
  xcodebuild -downloadComponent MetalToolchain
  ```

- [rustup](https://rustup.rs)。`rust-toolchain.toml` 會固定使用 1.97.1，第一次建置時自動安裝。
- 系統的 `curl`（行情請求用它）。

## 執行

```sh
git clone <這個 repo>
cd twstock-gpui
cargo run --release
```

要在有螢幕的桌面 session 執行（不能從 SSH 開視窗）。第一次建置需要幾分鐘。

### 自選清單

建立 `~/.config/twstock-gpui/watchlist.toml`：

```toml
[[watchlist]]
market = "tse"   # tse 上市、otc 上櫃
code = "889901"   # 範例代號，請換成自己要看的

[[watchlist]]
market = "tse"
code = "889902"
```

- 檔案不存在：使用範例資料。
- 檔案格式錯誤：顯示失敗，不會悄悄退回範例資料。
- 這個檔案只在你的機器上，不在 repo 裡。

## 驗證

```sh
cargo clippy --all-targets -- -D warnings
cargo test
cargo test --test rendering
```

- `cargo test --test rendering` 在不開視窗的情況下用 GPU 離屏渲染，輸出 `/tmp/twstock-gpui-*.png`，並檢查像素、版面邊界、選取、面板拖曳與縮放。可在 SSH 下執行。固定使用範例資料，不讀你的設定也不連網。
- 網路測試預設不跑。要對真實來源做一次煙霧測試：

  ```sh
  TWSTOCK_GPUI_LIVE=1 cargo test --test live_shot
  ```

  會讀你的 `watchlist.toml`，輸出 `/tmp/twstock-gpui-live-{dark,light}.png`。真實回應不會寫進 repo。

## 結構

```
src/
  design/    視覺的唯一來源：色階、字級角色、方向（漲跌）、間距、圓角、組合件、圖表
  market/    行情領域型別、MarketStore、MIS 與歷史 provider、設定。不依賴 gpui
  panels/    自選、走勢、行情、狀態列。只呼叫 design，不自己決定視覺
tests/
  design_rules.rs        掃描 design 以外的程式碼，出現寫死的顏色、字級、字重、間距、圓角就失敗
  provider_conformance.rs  所有 provider 共用的行為測試
  rendering.rs / live_shot.rs  離屏渲染
```

- **Design guideline**：design 以外不得決定視覺，由 `design_rules` 測試強制。
- **行情介面**：UI 只依賴領域型別與 `MarketStore`。要接其他來源（券商 API、websocket），實作一個 provider 並通過 `provider_conformance` 即可，UI 不用改。價格用定點數（單位 0.0001），升降單位屬於商品（ETF 與股票不同）。

## 已知限制

- 休市依台北時間、週末與來源日期推斷，尚未接正式交易日曆。
- 沒有逐筆成交（MIS 不提供）。
- 上櫃歷史日線、新上市不足 100 根的降級處理尚未做。
- 圖表週期只有日線，週、月、5 分、15 分目前是停用狀態。
- 只在 macOS 驗證過，也沒有長時間連續執行的驗證。

## 授權

自用專案，未訂授權。使用的 gpui-kit 為 Apache-2.0。
