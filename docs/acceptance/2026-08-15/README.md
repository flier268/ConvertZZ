# 2026-08-15 Linux X11 人工驗收

- 應用程式版本：`2.0.0`
- 原始碼版本：`efe9463`（畫面取自先前正式建置的 `target/release/convertzz`）
- 平台：Linux Mint、X11、XFCE、x86_64
- 執行日期：2026-08-15
- 執行者：代理程式本機操作

| 檔案 | 對應項目 | 說明 |
| --- | --- | --- |
| `H-03-E-01-file-preview.png` | H-03、E-01 | `/file /f:t` 自動建立內容預覽，顯示來源、輸出、編碼與轉換結果。 |
| `E-02-mode-dropdown.png` | E-02 | 作業可選轉換檔名。檔名預覽列未在本次擷取到。 |
| `E-05-confirm-overwrite.png` | E-05 | 衝突策略已改為覆寫。原生覆寫對話方塊未截到。 |
| `F-07-F-08-audio-preview.png` | F-07、F-08、G-13 | 第二個程序把 `/audio` 交給既有視窗。APE／OGG 不顯示 ID3 編碼選項；OGG 列出自訂與多值欄位。 |
| `G-02-floating-ball.png` | G-02、G-05、G-06 | 獨立 72×72 透明視窗，無白底，圖示為 Z²。 |
| `G-03-floating-dragged.png` | G-03 | 拖動後座標由 `1778,186` 改為 `1564,364`。 |
| `G-07-desktop-tray.png` | G-07 | 通知區可見應用程式圖示。 |
| `H-06-settings-linux.png` | H-06 | Linux 設定頁沒有 SendTo 區塊。 |
| `D-11-dictionary.png` | D-11 | 字典頁可開啟。儲存確認對話未截到。 |

關閉主視窗後，程序 `4158576` 與浮動球視窗仍在，對應 G-10。
