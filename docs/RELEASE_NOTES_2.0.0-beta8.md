# ConvertZZ 2.0.0-beta8

自 **v2.0.0-beta7** 起的預發行修正與體驗改進。

> 這是預發行版，供測試與回饋。正式版前行為與安裝流程仍可能調整。

## 下載

| 平台 | 建議檔案 |
| --- | --- |
| Windows x64 | `ConvertZZ_2.0.0-beta8_x64-setup.exe`（NSIS；本預發行版不產 MSI） |
| Windows x64（免安裝） | `ConvertZZ_2.0.0-beta8_x64-portable.zip`（解壓即可執行；設定寫在程式目錄） |
| Linux x64 | 優先 `ConvertZZ_2.0.0-beta8_amd64.AppImage`；亦可選 DEB／RPM |

每個發行檔請一併核對隨附的 `SHA256SUMS-*.txt`。Linux 執行相依見 `RUNTIME-DEPENDENCIES-linux-x64.txt`。

使用者**不需要**另外安裝 Node.js、FFmpeg 或 TagLib。

免安裝 zip 內含 `portable` 標記，設定會寫在程式目錄的 `settings-v2.json`，可整包帶走；不支援應用程式內自動更新。

## 相較 beta7 的主要變化

### 命令列

- 加入無頭 CLI（`--headless`；或明確 `--output`／`-o` 且未指定 `--file`／`--audio`），在 GUI／single-instance 之前結束。
- 以 2.0 `--flag` 為準（如 `--output`、`--direction`、`--yes`）；舊版 `/o:`、`/f:` 等仍可解析但不做後續擴充。
- 無頭預設不讀設定，須由命令列提供必要參數（至少 `--direction`）；`--globalconfig` 載入本機全域／可攜設定，`--config <路徑>` 載入指定設定檔（二者不可併用）；明確參數覆寫設定。
- 寫入需 `--yes`／`-y` 或 TTY「是／否」確認；`--audio --filename` 先寫標籤再改名、只確認一次。

### 介面

- 設定頁浮動球改為點擊／拖入矩陣操作。
- 關於頁改為並排差異卡，內容超出時才出現捲軸。

### 文件

- 更新 wiki submodule（無頭 CLI 與相關說明）。

## 已知限制與平台差異

與 beta7 相同的主要限制仍適用：

- Linux **Wayland**：全域快捷鍵與自動複製貼上於本版停用；浮動球置頂依合成器能力而定。
- Linux 托盤通常需要 AppIndicator／StatusNotifier；缺少時主視窗仍可使用。Tauri 在 Linux 不提供托盤左鍵事件，請用選單開啟主視窗。
- ZhConvert API 金鑰優先存入作業系統憑證庫；Linux 缺少 Secret Service 時只保留於目前工作階段。
- 少數字形結果可能與舊版 Windows `LCMapStringEx` 不同。
- 編碼工具改為明確指定編碼，不再跟隨舊版 `Encoding.Default` 系統碼頁。
- 免安裝（portable）套件不支援應用程式內自動更新。

## 發行標籤步驟（本機，不自動 push）

升版變更合併後：

```bash
git tag -a v2.0.0-beta8 -m "ConvertZZ v2.0.0-beta8"
# 確認後再推送以觸發草稿 Release 工作流程
git push origin v2.0.0-beta8
```

草稿 Release 說明可直接貼上本文件「下載」起的內容。完整變更範圍：

https://github.com/flier268/ConvertZZ/compare/v2.0.0-beta7...v2.0.0-beta8

## 回報問題

請到 [GitHub Issues](https://github.com/flier268/ConvertZZ/issues)，並附上作業系統、桌面環境（X11／Wayland）、安裝格式與重現步驟。

授權：GPL-3.0-only。第三方聲明見發行包內與倉庫的 `THIRD_PARTY_NOTICES.md`。
