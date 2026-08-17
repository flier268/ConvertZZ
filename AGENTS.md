# Repository Guidelines

ConvertZZ 2.0 是跨平台中文轉換桌面程式。產品說明與發行流程見 [README.md](README.md)。遷移範圍與未完成驗收條件見 [docs/MIGRATION_PLAN.md](docs/MIGRATION_PLAN.md)；已通過項目見 [docs/MIGRATION_COMPLETED.md](docs/MIGRATION_COMPLETED.md)。

使用繁體中文與使用者溝通。使用者介面字串也使用繁體中文。

## 目錄與責任

| 路徑 | 責任 |
| --- | --- |
| `src/` | Vue 3 畫面、預覽、確認與桌面狀態 |
| `src/pages/` | 快速轉換、檔案、剪貼簿、音訊、工具、字典、設定、關於 |
| `src/lib/` | 前端動作、設定、sidecar 客戶端、CLI、托盤與快捷鍵協調 |
| `sidecar/src/` | 文字、編碼、檔案、字典、音訊標籤與舊設定匯入 |
| `src-tauri/` | 視窗、托盤、快捷鍵、憑證庫、sidecar 程序與平台能力 |
| `shared/contracts.ts` | 前後端共用的 NDJSON 操作與型別 |
| `scripts/` | sidecar 編譯、打包、git hook 與 Linux 驗證 |
| `ConvertZZ/` | 舊 WPF 原始碼。驗收完成前不得刪除 |
| `tests/fixtures/` | 音訊與發行驗證樣本 |
| `e2e/` | Playwright 前端端對端測試 |

前端用 `@` 對應 `src/`，用 `@shared` 對應 `shared/`。Sidecar 直接匯入 `shared/contracts.ts` 編譯後的相對路徑。

## 層級邊界

- 轉換、編碼、檔案寫入、音訊標籤、字典與設定遷移只放在 sidecar。
- 視窗、托盤、全域快捷鍵、憑證庫、平台能力與 sidecar 程序只放在 Rust。
- Vue 只負責呈現、預覽、確認與呼叫型別化操作。
- 變更通訊協定時，先改 `shared/contracts.ts`，再同步 sidecar dispatch 與前端客戶端。
- Sidecar 記錄只寫標準錯誤。標準輸出只承載 NDJSON。
- 前端不得取得任意 Shell。檔案路徑只可來自選擇器或已驗證的工作項目。
- 新式引擎只使用 `novel-segment` 的分詞與 `ZhtSynonymOptimizer`，字形只交給 `cjk-conv`。不要新增硬編碼語意取代清單。

## 建置與檢查

需要 Node.js 24、Rust stable，以及 `package.json` 的 `packageManager` 所指定的 pnpm。Linux 建置套件見 README，不要自行加入 `libxdo-dev` 或 `libssl-dev`。

```bash
pnpm install --frozen-lockfile
pnpm run dev
pnpm run check
pnpm test
pnpm fmt
pnpm run sidecar:build
```

- `pnpm run check` 是 CI 門檻：Prettier、型別檢查、Vitest 與前端建置。
- `pnpm fmt` 會格式化 Vue、TypeScript 與 Rust。存檔與 commit 前也會自動執行。
- Rust 格式檢查：`cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`。
- `pnpm run tauri:build` 與 `pnpm run test:qemu` 很重，只在使用者明確要求發行或乾淨環境驗證時執行。
- `pnpm run test:e2e` 用 Playwright 對 Vite 前端做畫面測試，Tauri API 由 `e2e/mocks/` 模擬。不要把這條指令加進 `pnpm run check`。

## 套件更新

更新套件時優先用套件管理員指令，讓它同時改清單與 lockfile。不要手動改 `package.json`、`pnpm-lock.yaml`、`Cargo.toml` 或 `Cargo.lock` 裡的版本號。

```bash
pnpm add <套件>@<版本>
pnpm add -D <套件>@<版本>
pnpm update <套件>
corepack use pnpm@<版本>
cargo add <crate>@<版本> --manifest-path src-tauri/Cargo.toml
cargo update -p <crate> --manifest-path src-tauri/Cargo.toml
```

- 現有 Node 套件多數為精確版本。新增或升級時加上 `--save-exact`，不要改成 `^` 或 `~`，除非該套件本來就使用範圍版本。
- 升級 pnpm 用 `corepack use`，不要手改 `packageManager`。
- GitHub Actions 的 `pnpm/action-setup` 不要寫 `version`，讓它讀取 `package.json` 的 `packageManager`。
- 只改功能旗標、可選相依或目標平台區段時，可以編輯 `Cargo.toml` 的非版本欄位，版本仍交給 `cargo add` 或 `cargo update`。
- 更新後執行 `pnpm run check`。授權或 WASM 相依變更時同步更新第三方聲明。

## 程式風格

- TypeScript、Vue 與 JSON 使用 Prettier：行寬 100、雙引號、分號、多行尾逗號。
- Rust 使用 `rustfmt`。
- 新程式碼使用 TypeScript。維持現有的 `camelCase` 函式與欄位、`PascalCase` 元件與型別。
- Vue 頁面維持 Composition API 與現有 Element Plus 用法。
- 測試檔與實作放在同一目錄，檔名為 `*.test.ts`。
- 註解只說明非顯而易見的限制。不要用註解記錄實作過程。

## 測試

Vitest 涵蓋 `sidecar/src/**/*.test.ts`、`src/**/*.test.ts` 與 `tests/**/*.test.ts`。

前端 e2e 使用 `e2e/` 的 Playwright 規格，執行 `pnpm run test:e2e`。這會啟動 Vite 並模擬 Tauri／sidecar，不啟動桌面視窗。

- 引擎變更必須覆蓋黃金案例、空白標點保留、長文分段，以及舊字典的啟用、優先權與 `9999` 保護詞。
- 檔案變更必須覆蓋預覽未確認不寫入、衝突略過、暫存驗證、兩階段重新命名、失敗回復與不跟隨符號連結。
- 音訊變更必須覆蓋未選欄位、未知欄位、二進位欄位與封面不變，且不得重新編碼音訊內容。
- ZhConvert 變更必須用模擬伺服器鎖定官方 `/convert`，並確認網路失敗時不切換引擎。
- 單元測試通過不等於桌面或發行包驗收通過。不要把項目標為 `已通過` 並移入 [docs/MIGRATION_COMPLETED.md](docs/MIGRATION_COMPLETED.md)，除非指定證據已存在。

## 安全與相容

- 檔案與標籤作業必須先預覽，使用者確認後才寫入。
- 內容先寫入同目錄暫存檔，驗證成功後才取代原檔。
- 檔名衝突預設略過。覆寫需要額外確認。
- 舊版 `ConvertZZ.json` 只讀取，結果另存為 2.0 設定，不得修改來源。`Dictionary.csv` 必須先詢問，再建立不覆蓋的時間戳備份。備份失敗時不得寫入或覆寫。
- `Dictionary.csv` 維持 UTF-8 BOM 六欄格式。
- 命令列保持舊參數語意，但檔案作業仍要先預覽。不要恢復無確認寫入。
- Linux sidecar 以 target triple 檔名建立，發行包用 gzip 資源加 SHA-256 解壓。不要改回會被 AppImage 破壞的內嵌資料配置。
- 自動更新只簽署 Windows 安裝程式與 Linux AppImage。公鑰放在 `tauri.conf.json`，私鑰只存在 GitHub Secrets `TAURI_SIGNING_PRIVATE_KEY`。
- 授權維持 GPL-3.0-only。第三方與 WASM 授權聲明必須同步更新。

## 代理程式注意事項

若倉庫根目錄有 `.codegraph/`，先用 CodeGraph 找符號、呼叫路徑與影響範圍，再廣搜或通讀檔案。

不要還原工作區裡無關的變更。變更建置、測試、層級邊界或安全規則時，同步更新本文件。

修正錯誤時優先恢復正確契約，不要用最小可見補丁繞過預覽、備份、原子寫入或引擎邊界。
