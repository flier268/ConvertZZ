# ConvertZZ 2.0.0-beta4

自 **v2.0.0-beta3** 起的預發行修正與體驗改進。

> 這是預發行版，供測試與回饋。正式版前行為與安裝流程仍可能調整。

## 下載

| 平台 | 建議檔案 |
| --- | --- |
| Windows x64 | `ConvertZZ_2.0.0-beta4_x64-setup.exe`（NSIS；本預發行版不產 MSI） |
| Linux x64 | 優先 `ConvertZZ_2.0.0-beta4_amd64.AppImage`；亦可選 DEB／RPM |

每個發行檔請一併核對隨附的 `SHA256SUMS-*.txt`。Linux 執行相依見 `RUNTIME-DEPENDENCIES-linux-x64.txt`。

使用者**不需要**另外安裝 Node.js、FFmpeg 或 TagLib。

## 相較 beta3 的主要變化

### 啟動與首次導覽（Windows）

- 修正首次啟動時，若 AppData 設定目錄尚不存在，`plugin-store` 回傳「找不到路徑」導致啟動失敗的問題；改視為尚無設定並載入預設。
- 強化啟動錯誤診斷與記錄；浮動球初始化失敗時不再中止整個啟動流程。
- 首次導覽未完成時強制顯示主視窗，並避免啟動隱藏對主視窗重複 hide／show，減少 Windows 上主視窗閃過後只剩懸浮球的情況。

### 檔案選擇

- 選檔對話框預設加入「支援的檔案」篩選，副檔名依目前設定分類聯集產生，不重複寫入設定字串本身。

### 驗收與文件

- 封存自動更新閉環、AppIndicator／Secret Service 受控驗收、Windows 無 Node.js 安裝與 RPM／DNF 安裝等已通過項目。

## 已知限制與平台差異

與 beta3 相同的主要限制仍適用：

- Linux **Wayland**：全域快捷鍵與自動複製貼上於本版停用；浮動球置頂依合成器能力而定。
- Linux 托盤通常需要 AppIndicator／StatusNotifier；缺少時主視窗仍可使用。Tauri 在 Linux 不提供托盤左鍵事件，請用選單開啟主視窗。
- ZhConvert API 金鑰優先存入作業系統憑證庫；Linux 缺少 Secret Service 時只保留於目前工作階段。
- 少數字形結果可能與舊版 Windows `LCMapStringEx` 不同。
- 編碼工具改為明確指定編碼，不再跟隨舊版 `Encoding.Default` 系統碼頁。

## 發行標籤步驟（本機，不自動 push）

升版變更合併後：

```bash
git tag -a v2.0.0-beta4 -m "ConvertZZ v2.0.0-beta4"
# 確認後再推送以觸發草稿 Release 工作流程
git push origin v2.0.0-beta4
```

草稿 Release 說明可直接貼上本文件「下載」起的內容。完整變更範圍：

https://github.com/flier268/ConvertZZ/compare/v2.0.0-beta3...v2.0.0-beta4

## 回報問題

請到 [GitHub Issues](https://github.com/flier268/ConvertZZ/issues)，並附上作業系統、桌面環境（X11／Wayland）、安裝格式與重現步驟。

授權：GPL-3.0-only。第三方聲明見發行包內與倉庫的 `THIRD_PARTY_NOTICES.md`。
