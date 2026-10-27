---
name: linux-desktop-acceptance
description: >
  Linux X11 桌面人工驗收：避開 IME／檔案選擇器／原生對話／剪貼簿坑，用 AT-SPI、CLI
  路徑注入與 gpaste 歷史取證。觸發：驗收、人工證據、ConvertZZ D/E/G/I、Tauri Linux UI
  驗證、/linux-desktop-acceptance。
---

# Linux 桌面人工驗收

在 Linux 上為 ConvertZZ（或同類 Tauri 桌面程式）補齊遷移驗收證據。先讀環境閘門，再依正確操作取證；細節反模式見 [references/pitfalls.md](references/pitfalls.md)。

## 何時使用

- 使用者要求驗證 `docs/MIGRATION_PLAN.md` 的 D／E／G／I 待人工項
- 需要截圖、錄影、程序清單等桌面證據
- 先前自動化卡在檔案對話、輸入法、Element Plus 下拉或原生確認框

## 開始前（環境閘門）

記錄並依結果裁切範圍，不可硬測：

```bash
echo "SESSION=$XDG_SESSION_TYPE DESKTOP=$XDG_CURRENT_DESKTOP"
echo "WAYLAND=${WAYLAND_DISPLAY:-unset} DISPLAY=${DISPLAY:-unset}"
busctl --user list 2>/dev/null | grep -iE 'StatusNotifier|secrets|keyring' || true
systemctl --user is-active gnome-keyring-daemon.service 2>/dev/null || true
```

| 條件 | 可測 | 不可測（標環境受限） |
| --- | --- | --- |
| `XDG_SESSION_TYPE=x11` | X11 項、多數 D／E／G | I-03～I-05（需 Wayland） |
| 無 Wayland session | — | Wayland 簽核 |
| 有 StatusNotifierWatcher | 托盤正常路徑 | I-06（缺 AppIndicator） |
| Secret Service 啟用 | 金鑰可持久 | I-07 完整「關閉後重啟兩次」（勿貿然殺 keyring） |
| GitHub latest ＜ 本機版 | G-16 後備查詢 | G-16「開啟下載頁」、G-15 簽署安裝 |

二進位優先用今日 `src-tauri/target/release/convertzz`；資料目錄為 `~/.local/share/dev.flier268.convertzz/`（見 `tauri.conf.json` 的 `identifier`）。

**禁止** `convertzz --version`／對 GUI 二進位跑會掛起的「版本」指令（會開視窗且不退出）。  
**禁止** `pkill -f convertzz`（會誤殺含該字串的 shell wrapper）；改 `pgrep -x convertzz` 後依 PID `kill`。

## 證據目錄

證據**不要**寫進倉庫的 `docs/`（錄影／截圖體積大）。預設：

```text
~/Desktop/ConvertZZ-acceptance/YYYY-MM-DD/
  META.txt          # git SHA、二進位、平台、日期
  README.md         # 項目→檔案對照與環境限制
  <ID>-*.png|mp4|txt
```

驗收前備份設定目錄到同一證據夾。只有必要證據齊全才把項目改為 `已通過` 並移入 `docs/MIGRATION_COMPLETED.md`（見 `AGENTS.md`）；遷移文件只記摘要路徑，不提交媒體檔。

## 工具選擇

| 目的 | 用法 |
| --- | --- |
| 點選主視窗控件 | **pyatspi**：依 `name`／`role` 找節點，`queryAction().doAction(0)`；失敗再用 `getExtents` + `xdotool` |
| 視窗截圖 | `xdotool getwindowgeometry --shell` → `ffmpeg -f x11grab -video_size WxH -i :0.0+X,Y -frames:v 1 -update 1 out.png` |
| 操作錄影 | `ffmpeg -f x11grab -video_size 螢幕 -framerate 8 -i :0.0 -vf scale=1600:-2 ... out.mp4` |
| 寫入剪貼簿 | `gpaste-client add '文字'` |
| 讀取剪貼簿 | `gpaste-client history \| head -1 \| sed 's/^[^:]*: //'`（**不要** `gpaste-client get 0`） |
| 注入檔案路徑 | Python `subprocess.Popen([bin,'/file','/f:t','/b:f', unicode_path])` 交給既有實例；**不要**用 `xdotool type` 打路徑 |
| Element Plus 下拉 | 點 combo → `xdotool key Down`（次數依選項）→ `Return`；驗證欄位顯示已變 |
| 原生確認（rfd／plugin-dialog） | AT-SPI 找 `alert`；按鈕常為 `確定(O)`／`取消(C)`；`Tab` 後 `space` 可確認 |

切換輸入法至英文（例如 `fcitx5-remote -s keyboard-us`）仍不足以保證 `xdotool type` 路徑正確；路徑一律走剪貼簿貼上或 CLI argv。

## 項目流程（ConvertZZ）

### D-07 首次匯入前詢問

1. 備份並移走資料目錄，建立空目錄。
2. 將舊 `ConvertZZ.json` 放在啟動 cwd（`legacy_settings_path` 查 exe 旁與 cwd）。
3. 從該 cwd 啟動 release 二進位；導覽至「匯入舊版設定」。
4. 證據：畫面須有「匯入找到的設定／選擇檔案／略過匯入」，且未自動寫入。

### D-11 儲存字典前詢問

1. 設定可寫 `dictionaryPath`（可寫入 `settings-v2.json` 後重啟，或成功選檔後）。
2. 「新增」一筆即可讓 `changeCount>0`（單元格未填也可觸發儲存確認）。
3. 按「儲存變更」→ 擷取「確認字典備份」。

### E-02 檔名預覽顯示來源與輸出

1. 用 CLI 注入簡體檔名樣本路徑（避免檔案選擇器）。
2. 取消自動「內容」計畫 → 作業改「轉換檔名」、方向「簡轉繁」→「建立預覽」。
3. 證據：表格同時有「來源」「輸出」欄且檔名已轉換。

### E-05 覆寫額外確認

1. 同目錄備好 `电脑.txt` 與已存在的 `電腦.txt`。
2. CLI 注入來源 → 作業「轉換檔名」、衝突策略「覆寫」（鍵盤選取並確認 UI 顯示「覆寫」與警告「輸出路徑已存在。」）。
3. 「確認執行」→「確認檔案轉換」按 `確定(O)` → 擷取「確認覆寫」。不要真的覆寫時按取消。

### G-12 快捷鍵／托盤／浮動球同路由

1. 寫入設定：`quickActions.leftClickCtrl=a3`；啟用一組快捷鍵指向 `a3`；重啟以套用。
2. 同一輸入文字，依序：浮動球 Ctrl+左鍵、快捷鍵、右鍵選單「Unicode 簡 → Unicode 繁」（與托盤選單同一組 legacy action id）。
3. 用 gpaste history 比對三次輸出；寫入 `G-12-compare.txt`。

### G-13 第二實例交棒

1. `pgrep -ax convertzz` 存 before。
2. 再啟動 `convertzz /audio`（或 `/file …`）。
3. after：只剩一個 PID；第二程序應已退出；UI 導向對應頁。

### G-15／G-16 更新

- **G-15**：需要已發佈且已簽署、版本新於本機的 AppImage／Windows 安裝包與有效 `latest.json`。
- **G-16**：DEB／無簽署通道時應走 GitHub 後備。若遠端 latest 舊於本機，只會「已是最新」——記錄 DEB、`latest.json` HTTP 狀態與截圖，標部分完成；勿偽造成「已開啟下載頁」。

### I 平台項

- I-03～I-05：僅 Wayland；可先截 X11 關於頁能力表作對照，但不算通過。
- I-06／I-07：需缺托盤或可安全關閉 Secret Service 的受控環境。
- I-08：三平台矩陣簽核後才結案。

## 結束

1. 寫 `README.md`／`META.txt`。
2. 通過項移入 `docs/MIGRATION_COMPLETED.md`，更新 `docs/MIGRATION_PLAN.md` 摘要與待補備註。
3. 告知使用者設定備份路徑；非經要求不要默默覆寫其設定。
