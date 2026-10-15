# twstock-gpui

台股看盤桌面程式，自用。原生 macOS，用 [gpui-kit](https://github.com/longbridge/gpui-kit) 的 GPUI 元件寫成，不是網頁。

![深色主題，範例資料](docs/screenshots/dark.png)

截圖用的是編造的範例資料（代號 99 開頭的虛構商品），不是真實行情。

## 功能

- 三欄工作區：自選（含迷你走勢線）、走勢圖、行情（五檔、買賣比例、成交量）。面板可拖曳與縮放。
- 走勢圖用 gpui-kit 的 plot 元件自繪：蠟燭（收高實心、收低空心，不只靠顏色分方向）、MA5 / MA10 / MA20、成交量副圖、金色十字游標與軸標籤、最高最低價標註、現價線。
- 紅漲綠跌（台股慣例），平盤為灰。價格與數字一律中性色，只有漲跌上色。
- 日／週／月線可切換；週、月由既有日線聚合，不額外請求。切換股票保留目前週期，不跨重啟保存。沒有免費分鐘線，介面不放停用的分鐘選項。
- ⌘K 搜尋商品，依代號前綴或名稱子字串加入自選；上下鍵選取、Enter 加入、Esc 關閉。已加入與超過訂閱上限會明確提示。
- 深色與淺色主題。
- 連線狀態一直看得到：連線中、正常、過期、失敗、休市。斷線不會被顯示成平盤。

![十字游標](docs/screenshots/crosshair.png)

![週線](docs/screenshots/weekly.png)

![⌘K 搜尋並加入自選](docs/screenshots/palette.png)

## 資料來源

| 用途 | 來源 | 限制 |
|---|---|---|
| 即時報價與五檔 | 證交所 MIS（`mis.twse.com.tw`） | 非官方端點，無保證。約 5 秒更新一次，程式輪詢不少於 5 秒。沒有逐筆成交。成交價欄位常為 `-`。 |
| 歷史日線 | 證交所 `STOCK_DAY` | 一次一個月，往前補到 100 根，最多回溯 12 個月。目前只支援上市，上櫃歷史尚未接。 |
| 商品清單 | 證交所 ISIN 公告清單，上市 `strMode=2`、上櫃 `strMode=4` | 只收官方「股票」及「ETF」分類；以分類決定 ETF 價階，不猜代號前綴。 |

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
- 也可直接按 ⌘K（或標題列「搜尋商品」）加入。從範例模式加入第一檔時，會明確提示建立設定並切換真實 MIS；不把範例商品寫入設定。
- 在自選選取一列後按 Delete／Backspace，或開啟「自選」標題旁的「…」→「移除選取」。真實自選的加入／移除會用 `toml_edit` 原子寫回，保留未移除內容的註解與格式；支援 `[[watchlist]]`、`[[symbol]]` 等原有別名及 inline array。
- 空清單會保存為空清單，重啟不退回範例。範例商品的移除只影響當次展示，不寫入真實自選。
- 訂閱變更在現有 provider 的下一輪請求生效，不必重啟；仍遵守至少 5 秒的輪詢間隔。排序尚未提供。

### 週期與搜尋資料

- 週線按交易日所在 ISO 週分組（跨年也依 ISO 週年），月線按年月分組。開盤取首日、收盤取末日、高低取極值、量加總；已知的今日即時日線會納入未完成期間。沒有成交價時的中間參考價不會拿來造日線。
- MA5／10／20 以目前週期重算；根數不足就顯示 `—`。日期軸日／週為 `MM/DD`，月為 `YYYY/MM`。圖表資訊列顯示實際根數，不補假的月份；目前 Mock 為週 15 根、月 4 根，月 MA 因不足 5 根而留空。2026-10-15 三檔真實資料實測皆為週 22 根、月 6 根；月 MA10／20 仍不足根數。
- 搜尋排序：代號或名稱完全相符 → 代號前綴 → 名稱子字串；同級依代號、再市場代碼排序。名稱比對不區分英文字母大小寫。
- 清單保留在記憶體，磁碟快取位於 `~/.cache/twstock-gpui/directory.json`，含抓取時間；24 小時到期後重抓。搜尋再次開啟時也會檢查到期；失敗顯示原因，不使用假清單或過期快取。

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

  會讀你的 `watchlist.toml`，載入清單以取得正確商品類型，輸出 `/tmp/twstock-gpui-live-{dark,light}.png`，並列出每檔實際日／週／月根數。真實回應不會寫進 repo。

  單獨驗證商品清單的兩個市場端點（強制實抓，回報筆數與耗時）：

  ```sh
  TWSTOCK_GPUI_LIVE=1 cargo test --test directory directory_live_opt_in -- --nocapture
  ```

  Rendering 另輸出 `/tmp/twstock-gpui-weekly.png`、`/tmp/twstock-gpui-monthly.png`、`/tmp/twstock-gpui-palette.png`，均用編造資料；同時驗證搜尋、鍵盤加入、重複／超量、移除與空清單。

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
- 沒有分鐘線。日線仍最多回溯 12 個月／目前載入 100 根；因此週、月歷史較短，月線通常不足 MA10／20 所需根數。
- 商品搜尋只支援官方「股票」與「ETF」分類；權證、特別股、創新板、ETN、TDR、受益證券尚未納入，避免猜測價階。
- 商品清單與報價仍依賴外部來源；快取過期且更新失敗時，搜尋面板會顯示失敗。
- 只在 macOS 驗證過，也沒有長時間連續執行的驗證。

## 授權

自用專案，未訂授權。使用的 gpui-kit 為 Apache-2.0。
