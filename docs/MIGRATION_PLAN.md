# ConvertZZ 2.0 遷移計畫與驗收清單

## 文件目的

本文件定義 ConvertZZ 2.0 的遷移範圍、完成定義，以及**尚未通過／待確認**的驗收項目。

已通過的驗收項目與歷史證據摘要已封存於 [MIGRATION_COMPLETED.md](MIGRATION_COMPLETED.md)。

所有必要項目通過後，遷移才算完成。

舊 WPF 專案已於 K-07 自倉庫移除；可自標籤 `legacy-wpf-final` 回復。

## 目標

- 版本提升為 `2.0.0`。
- 桌面介面遷移至 Vue 3、Element Plus 與 Tauri 2。
- 文字與檔案核心遷移至 Rust（`cjk-convert-rs`、`ws-segment-rs`）。
- Windows x64 與 Linux x64 共用相同轉換核心。
- 新式分詞轉換成為預設方法。
- 舊版六欄字典保留為相容方法。
- APE 與 OGG 標籤成為正式支援功能。
- 發行包不要求使用者另行安裝 Node.js。
- GPL-3.0-only 授權保持不變。

## 完成定義

目前狀態以 2026-08-14 的工作目錄為準，並於後續日期持續更新本文件中的未完成項目。

目前狀態不是正式發行簽核。

驗收狀態使用下列標記。

| 標記 | 定義 |
| --- | --- |
| `已通過` | 功能已實作，而且指定證據已產生。通過後移至 [MIGRATION_COMPLETED.md](MIGRATION_COMPLETED.md)。 |
| `待人工驗收` | 功能已實作，但仍缺少指定平台的人工證據。 |
| `部分完成` | 驗收條件只滿足一部分；本文件必須寫明「已成立」與「仍缺」。 |
| `未完成` | 前置條件未滿足，或最終閘門／簽核尚未執行；本文件必須寫明卡在哪裡。 |

單元測試通過不等同於桌面整合驗收通過。

本機建置通過不等同於安裝包驗收通過。

每一項驗收證據必須記錄版本、平台與日期。

螢幕互動項目必須保留截圖或錄影。

檔案安全項目必須保留輸入、輸出與雜湊紀錄。

發行項目必須以乾淨環境產生證據。

## 目標架構

| 層級 | 責任 | 技術 |
| --- | --- | --- |
| 前端 | 畫面、預覽、確認與狀態呈現 | Vue 3.5、TypeScript、Vite、Element Plus 2.14 |
| 桌面層 | 視窗、托盤、快捷鍵、檔案選擇與程序管理 | Tauri 2.11、Rust |
| 核心層 | 文字、檔案、編碼、字典與音訊標籤 | Rust（與桌面層同程序） |
| 發行層 | 分詞字典與安裝包 | pnpm、Tauri Bundler |

前端透過 Tauri `core_request` 呼叫型別化操作。

每個要求都包含識別碼、操作名稱與型別化資料。

每個回應都包含結果、警告、進度或結構化錯誤。

核心進度透過 Tauri 事件 `core://progress` 回傳。

前端不取得任意 Shell 執行權限。

檔案路徑只可來自選擇器或已驗證的工作項目。

## 遷移階段

### 第一階段：基線與相容資料

保留舊版原始碼與資料格式。

建立 `SettingsV2` 與舊設定匯入流程。

保留 `Dictionary.csv` 的 UTF-8 BOM 六欄格式。

本階段完成後，不得直接覆寫舊設定或舊字典。

### 第二階段：共用轉換核心

在 `src-tauri/src/core` 建立 Rust 轉換核心與型別化 `core_request` 協定。

實作新式分詞、舊版字典與 ZhConvert 三種引擎。

實作長文字切分與所有必要編碼。

本階段完成後，相同輸入應在 Windows 與 Linux 產生相同結果。

### 第三階段：桌面介面

建立 Vue 與 Element Plus 主介面。

建立快速轉換、檔案、剪貼簿、音訊、工具、字典、設定與關於頁面。

建立浮動球、托盤、全域快捷鍵與單一執行個體行為。

本階段完成後，使用者可在不開啟舊 WPF 程式的情況下完成主要操作。

### 第四階段：檔案與音訊安全

建立檔案與標籤預覽。

建立確認、衝突、暫存寫入、驗證與回復流程。

使用 Rust `id3` 處理 MP3。

使用 Rust `lofty` 處理 APE、OGG、OGA 與 Opus。

本階段完成後，文字標籤以外的音訊內容不得改變。

### 第五階段：平台與發行

完成 Windows、X11 與 Wayland 的能力分流。

Linux 托盤使用 AppIndicator 執行函式庫。

AppIndicator 開發套件只存在於建置環境。

將分詞字典與字典資源一併封入安裝包；轉換核心與桌面層同程序，不另外封裝外部程序。

建立 Windows 與 Linux 發行工作流程。

本階段完成後，乾淨環境可直接安裝及執行。

### 第六階段：最終驗收與舊專案退場

執行本文件與封存文件中的所有必要驗收項目。

整理失敗紀錄與平台差異。

所有必要項目通過後，才可移除舊 WPF 專案。

## 實作決策與已知差異

- 新式引擎只使用 `ws-segment-rs` 的分詞與 `ZhtSynonymOptimizer`。
- 新式引擎只使用 `cjk-convert-rs` 完成字形轉換。
- 專案不維護額外的硬編碼語意取代清單。
- 轉換核心在 Tauri 同程序內執行，不另外封裝外部 Node.js 程序。
- 分詞字典以 `resources/segment-dict` 封入安裝包。
- Linux 的 Tauri 托盤不提供左鍵事件。
- Linux 使用者需從托盤選單開啟主視窗。
- 舊版 `Encoding.Default` 會隨 Windows 系統碼頁改變。
- 新版編碼工具改用明確指定的編碼。
- 舊命令列批次改為先顯示預覽並要求確認。
- 此變更保留參數相容性，但刻意不保留無確認寫入行為。
- Linux 乾淨環境可用 `pnpm run test:qemu`，以 QEMU 啟動 Ubuntu 22.04 cloud image，安裝 DEB 後離線驗證主程式與分詞字典。

前端 Playwright（`pnpm run test:e2e`）與 `src/acceptance-contracts.test.ts`、`tests/release-workflow.test.ts` 等會鎖定畫面契約、動作路由、發行工作流程與能力旗標，作為改壞後的自動護欄。這仍不能取代桌面視窗、托盤實機點擊、簽署安裝與乾淨環境的人工證據。未完成這些證據前，下列「待人工驗收」項目不得改為已通過。

## 已通過範圍摘要

下列區塊的必要項目已全部（或該子集）通過，細節見 [MIGRATION_COMPLETED.md](MIGRATION_COMPLETED.md)：

| 區塊 | 說明 |
| --- | --- |
| A | 專案與建置基線（A-01～A-08） |
| B | 核心與通訊協定（B-01～B-09） |
| C | 文字轉換引擎（C-01～C-16） |
| D | 編碼／工具／相容資料（D-01～D-12） |
| E | 檔案與檔名安全（E-01～E-11） |
| F | 音訊標籤（F-01～F-15） |
| G | 桌面整合（G-01～G-16） |
| H | 命令列相容（H-01～H-06） |
| I | 平台差異（I-01～I-09） |
| J | 測試與發行（J-01～J-13） |
| K | 退場閘門（K-01～K-07） |

## 未完成與待確認項目

目前**無**尚未通過的必要驗收項目。歷史與證據見 [MIGRATION_COMPLETED.md](MIGRATION_COMPLETED.md) 與 `~/Desktop/ConvertZZ-acceptance/`。

Linux X11：2026-08-18／19 本機證據見各日期目錄。Linux Wayland：2026-08-24（`2026-08-24-wayland/`）。K 簽核與舊 WPF 移除：2026-08-24（`2026-08-24-k-signoff/`、`2026-08-24-k07-remove-wpf/`；標籤 `legacy-wpf-final`）。媒體證據不進倉庫（見 skill `linux-desktop-acceptance`）。

## 驗收紀錄格式

每次驗收使用下列表格。

| 欄位 | 內容 |
| --- | --- |
| 驗收編號 | 例如 `F-13`。 |
| 應用程式版本 | 例如 `2.0.0-rc.1`。 |
| 原始碼版本 | Git commit SHA。 |
| 平台 | Windows、Linux X11 或 Linux Wayland。 |
| 環境 | 作業系統版本、桌面環境與架構。 |
| 執行日期 | 使用 ISO 8601 日期。 |
| 執行者 | 驗收人姓名或帳號。 |
| 結果 | 通過或失敗。 |
| 證據 | CI 連結、檔案、截圖、錄影或雜湊。 |
| 備註 | 限制、重現步驟或後續工作。 |

## 最終簽核

| 平台 | 版本 | 結果 | 驗收人 | 日期 | 證據 |
| --- | --- | --- | --- | --- | --- |
| Windows x64 | 2.0.0-beta6 | 通過 | flier268 | 2026-08-24 | 使用者確認可用；I-01、G-15 Windows NSIS、J-08 |
| Linux x64 X11 | 2.0.0-beta6 | 通過 | flier268 | 2026-08-24 | `~/Desktop/ConvertZZ-acceptance/2026-08-18-*`、`2026-08-19-linux-update/`、`2026-08-19-i06-i07/` |
| Linux x64 Wayland | 2.0.0-beta6 | 通過 | flier268 | 2026-08-24 | `~/Desktop/ConvertZZ-acceptance/2026-08-24-wayland/` |

三平台已完成簽核。舊 WPF 可回復標記為 `legacy-wpf-final`（commit `9425ab8a230faf8b573201ef72a6c71dfd17ea7d`）。K-07 已於 2026-08-24 移除倉庫內 `ConvertZZ/`；字典資源改由 `resources/Dictionary.csv` 提供。
