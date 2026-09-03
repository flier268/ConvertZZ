# ConvertZZ 2.0.0-beta7

自 **v2.0.0-beta6** 起的預發行修正與體驗改進。

> 這是預發行版，供測試與回饋。正式版前行為與安裝流程仍可能調整。

## 下載

| 平台 | 建議檔案 |
| --- | --- |
| Windows x64 | `ConvertZZ_2.0.0-beta7_x64-setup.exe`（NSIS；本預發行版不產 MSI） |
| Windows x64（免安裝） | `ConvertZZ_2.0.0-beta7_x64-portable.zip`（解壓即可執行；設定寫在程式目錄） |
| Linux x64 | 優先 `ConvertZZ_2.0.0-beta7_amd64.AppImage`；亦可選 DEB／RPM |

每個發行檔請一併核對隨附的 `SHA256SUMS-*.txt`。Linux 執行相依見 `RUNTIME-DEPENDENCIES-linux-x64.txt`。

使用者**不需要**另外安裝 Node.js、FFmpeg 或 TagLib。

免安裝 zip 內含 `portable` 標記，設定會寫在程式目錄的 `settings-v2.json`，可整包帶走；不支援應用程式內自動更新。

## 相較 beta6 的主要變化

### 轉換品質

- 新增 ConvertZZ 額外修正層（`extra-correction`）：語料回環產生的分詞表與同義詞，依詞性套用，不寫入套件詞典。
- 字形特例改為檔案協議（`conversion-specials`）：`rules.txt` 與台灣完整「xx縣／xx市／xx鄉／xx鎮／xx里」`place-names.txt`，分詞釘詞後再轉換套用。
- 簡轉繁字形路徑改為 `cn2tw_min` 再接 `cjk2zht`；升級 `ws-segment-rs`。
- 檔案批次支援並行、取消，以及確認後逐檔立即寫入。

### 檔案預覽與介面

- 檔案內容預覽改為延遲載入，並可調整並排介面。
- 預覽支援全視窗 diff、長文翻頁與上下差異導航。
- 各工作頁改為鎖視窗高度並填滿剩餘空間。

### 專案與工具

- 舊 WPF 原始碼已移除（K-07）；字典改由 `resources/Dictionary.csv` 提供。
- 語料回環工具拆成獨立 `roundtrip-dict` crate，支援檢查點、增量重跑與探針旗標。

## 已知限制與平台差異

與 beta6 相同的主要限制仍適用：

- Linux **Wayland**：全域快捷鍵與自動複製貼上於本版停用；浮動球置頂依合成器能力而定。
- Linux 托盤通常需要 AppIndicator／StatusNotifier；缺少時主視窗仍可使用。Tauri 在 Linux 不提供托盤左鍵事件，請用選單開啟主視窗。
- ZhConvert API 金鑰優先存入作業系統憑證庫；Linux 缺少 Secret Service 時只保留於目前工作階段。
- 少數字形結果可能與舊版 Windows `LCMapStringEx` 不同。
- 編碼工具改為明確指定編碼，不再跟隨舊版 `Encoding.Default` 系統碼頁。
- 免安裝（portable）套件不支援應用程式內自動更新。

## 發行標籤步驟（本機，不自動 push）

升版變更合併後：

```bash
git tag -a v2.0.0-beta7 -m "ConvertZZ v2.0.0-beta7"
# 確認後再推送以觸發草稿 Release 工作流程
git push origin v2.0.0-beta7
```

草稿 Release 說明可直接貼上本文件「下載」起的內容。完整變更範圍：

https://github.com/flier268/ConvertZZ/compare/v2.0.0-beta6...v2.0.0-beta7

## 回報問題

請到 [GitHub Issues](https://github.com/flier268/ConvertZZ/issues)，並附上作業系統、桌面環境（X11／Wayland）、安裝格式與重現步驟。

授權：GPL-3.0-only。第三方聲明見發行包內與倉庫的 `THIRD_PARTY_NOTICES.md`。
