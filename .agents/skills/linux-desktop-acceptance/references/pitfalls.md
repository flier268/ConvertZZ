# 坑與正確作法

從 2026-08-18 ConvertZZ Linux X11 驗收整理。SKILL 正文只保留流程；此處為反模式對照。

## 程序與啟動

| 錯誤作法 | 後果 | 正確作法 |
| --- | --- | --- |
| `convertzz --version` 或對 GUI 二進位跑「印版本就退」 | 開主視窗／sidecar，指令不退出 | 用 `dpkg -l`、`stat` 二進位、或設定／關於頁看版本 |
| `pkill -f convertzz` | 誤殺含該字串的 agent shell | `pgrep -x convertzz` → 對 PID `kill`／`kill -9` |
| 假設已安裝 DEB 與 repo release 同建置 | 驗到舊行為 | 優先 `src-tauri/target/release/convertzz`；在 `META.txt` 記 mtime 與 git SHA |

## 輸入法與路徑

| 錯誤作法 | 後果 | 正確作法 |
| --- | --- | --- |
| `xdotool type /tmp/.../中文或長路徑` | fcitx／Rime 把按鍵組成錯字，設定寫入垃圾路徑 | `gpaste-client add "$path"` → 對話框 `Ctrl+L` → `Ctrl+V`；或完全避開對話框 |
| 依賴檔案／資料夾選擇器完成 E／D | 對話殘留、位置列彈層、路徑未進 app | 第二實例 CLI：`[bin,'/file','/f:t','/b:f', unicode_path]`（Python argv，勿經 shell 弄壞編碼） |
| 選完資料夾未確認「尚未選取路徑」是否消失 | 後續預覽空白仍繼續點 | 每次選路徑後截圖確認 path summary |

## UI 自動化

| 錯誤作法 | 後果 | 正確作法 |
| --- | --- | --- |
| 用視窗百分比盲點「下一步」 | Tour 不動或誤關 | pyatspi 找 `push button`「下一步」／「結束導覽」 |
| AT-SPI `doAction` 點 Element Plus `list item` 就當選取成功 | 下拉仍顯示舊值（例如衝突策略一直「略過」） | 點 combo 後 `Down`×N + `Return`；截圖確認欄位文字已變 |
| 原生確認找 `Yes`／`No`／`是`／`否` | 找不到按鈕 | 找 `alert`；按鈕多為 `確定(O)`／`取消(C)`；或 `Tab` + `space` |
| `Escape` 以為一定關掉 rfd 確認 | 對話殘留，擋住後續操作 | 明確點取消；重開流程前先確認無 `alert` |
| 像素找 Element Plus primary 藍鈕 | 深色主題／進度點誤判 | 以 AT-SPI name 為準；像素僅輔助 |

## 剪貼簿（G-12）

| 錯誤作法 | 後果 | 正確作法 |
| --- | --- | --- |
| `gpaste-client get 0` | UUID API 失敗，誤判轉換沒跑 | `gpaste-client history` 取第一筆內容 |
| 未先寫入 `quickActions`／`hotkeys` 就測 | 全是 `"0"`／未啟用，無輸出 | 改 `settings-v2.json` 後**重啟**再測 |
| 只測浮動球就宣稱托盤通過 | 證據不足 | 快捷鍵 + 浮動球手勢 + 與托盤共用 action id 的選單項；在 compare 檔註明托盤與右鍵選單同源 |

## 檔案／覆寫（E）

| 錯誤作法 | 後果 | 正確作法 |
| --- | --- | --- |
| CLI 注入後不改作業模式 | 一直是「轉換內容」預覽 | 取消計畫 → 改「轉換檔名」→ 再建立預覽 |
| 衝突策略 UI 未變成「覆寫」就按執行 | 不會出現第二次「確認覆寫」 | 確認警告「輸出路徑已存在。」且策略顯示「覆寫」 |
| 覆寫測完不還原樣本 | 後續重跑缺衝突 | 測前重建 `电脑.txt`／`電腦.txt`；確認框用取消離開 |

## 更新（G-15／G-16）

| 錯誤作法 | 後果 | 正確作法 |
| --- | --- | --- |
| 本機 2.0.0 + GitHub latest 1.0.0.8 卻期待「開啟下載頁」 | 只會「已是最新版本」 | 記 DEB、`latest.json` 狀態、截圖；標部分完成 |
| 無簽署 `latest.json` 卻標 G-15 通過 | 違反證據規則 | 等正式簽署發行通道 |

## 平台（I）

| 錯誤作法 | 後果 | 正確作法 |
| --- | --- | --- |
| 在 X11 截關於頁就標 I-03～I-05 通過 | 證據平台不符 | 明確寫「需 Wayland」 |
| 為 I-07 直接殺 gnome-keyring | 可能鎖死桌面金鑰／登入 | 受控環境或標受限；程式契約另用單元／程式碼核對 |

## 證據與文件

| 錯誤作法 | 後果 | 正確作法 |
| --- | --- | --- |
| 單元測試通過就改 `已通過` | 違反 AGENTS.md | 必要證據入倉庫外目錄（預設 `~/Desktop/ConvertZZ-acceptance/YYYY-MM-DD/`） |
| 把錄影／截圖提交進 `docs/` | 倉庫膨脹 | 證據留本機；遷移文件只寫摘要與路徑 |
| 不寫環境限制 | 下次重複踩坑 | README 固定「環境限制」表 |
| 驗收中覆寫使用者設定不備份 | 難以還原 | 先 `cp -a` 資料目錄到證據夾 |
