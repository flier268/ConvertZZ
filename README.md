# ConvertZZ 2.0

ConvertZZ 是跨平台中文轉換工具。

2.0 版使用 Vue、Element Plus、Node.js 與 Tauri 2。

正式支援 Windows x64 與 Linux x64。

下載請前往 [GitHub Releases](https://github.com/flier268/ConvertZZ/releases)。

問題請回報至 [GitHub Issues](https://github.com/flier268/ConvertZZ/issues)。

## 主要功能

- 快速簡繁轉換。
- 剪貼簿轉換。
- 檔案內容轉換。
- 批次檔名轉換。
- MP3 ID3v1 與 ID3v2 標籤轉換。
- APE APEv2 標籤轉換。
- OGG、OGA 與 Opus Vorbis Comment 轉換。
- Unicode 與傳統文字編碼轉換。
- HTML 字元實體轉換。
- 全形與半形轉換。
- 舊版六欄字典編輯。
- 獨立浮動球。
- 系統托盤與全域快捷鍵。

## 轉換引擎

「新式分詞」是預設引擎。

它使用 [novel-segment](https://github.com/bluelovers/ws-segment) 分詞。

簡轉繁會啟用同義詞最佳化。

語意修正只使用 `novel-segment` 提供的 `ZhtSynonymOptimizer`。

專案不另外維護硬編碼語意取代清單。

字形轉換由 [cjk-conv](https://github.com/bluelovers/cjk-convert) 完成。

空白、換行與標點會保留原位。

長文字會依安全邊界分段。

「舊版字典」會讀取 `Dictionary.csv`。

它會保留啟用狀態與優先權。

它會保留長詞優先規則。

優先權 `9999` 仍代表保護詞。

未命中字元會交由 `cjk-conv` 處理。

少數字形會與 Windows `LCMapStringEx` 不同。

「ZhConvert」是選用的網路服務。

程式會使用官方 `/convert` 端點。

程式會快取 `/service-info` 二十四小時。

網路錯誤不會切換至其他引擎。

使用前請閱讀 [ZhConvert API 文件](https://docs.zhconvert.org/api/0-getting-started/)。

商業使用前請確認服務條款。

API 金鑰會優先存入作業系統憑證庫。

Linux 缺少 Secret Service 時只會保留於目前工作階段。

## 音訊標籤

| 格式 | 標籤容器 | Windows | Linux |
| --- | --- | --- | --- |
| MP3 | ID3v1、ID3v2.3、ID3v2.4 | 支援 | 支援 |
| APE | APEv2 | 支援 | 支援 |
| OGG、OGA | Vorbis Comment | 支援 | 支援 |
| Opus | Vorbis Comment | 支援 | 支援 |

MP3 標籤由 `mp3tag.js` 處理。

ID3v1 可修復 Big5 與 GBK 文字。

APE、OGG 與 Opus 由 `taglib-wasm` 處理。

APEv2 與 Vorbis Comment 固定使用 UTF-8。

多值文字欄位會逐值轉換。

未選欄位不會被改寫。

未知欄位不會被刪除。

二進位欄位不會被轉換。

封面圖片會保持不變。

音訊內容不會重新編碼。

`taglib-wasi.wasm` 會包含於安裝包。

程式不會在執行時下載 WASM。

## 檔案安全

檔案與標籤作業都會先建立預覽。

執行前需要使用者確認。

檔名衝突預設會略過。

覆寫會要求額外確認。

內容會先寫入同目錄暫存檔。

驗證成功後才會取代原檔。

檔名會以兩階段方式重新命名。

中途失敗時會嘗試回復。

遞迴處理不會跟隨符號連結。

舊版字典儲存前會詢問使用者。

確認後會先建立不覆蓋的時間戳備份。

## 平台差異

| 功能 | Windows | Linux X11 | Linux Wayland |
| --- | --- | --- | --- |
| 文字、檔案與檔名轉換 | 完整 | 完整 | 完整 |
| ID3、APEv2、OGG 與 Opus | 完整 | 完整 | 完整 |
| 全域快捷鍵 | 完整 | 完整 | 本版停用 |
| 自動複製與貼上 | 完整 | 完整 | 停用 |
| 浮動球置頂 | 完整 | 完整 | 依合成器能力 |
| SendTo 捷徑 | 完整 | 不適用 | 不適用 |
| 系統托盤 | 左鍵與選單完整 | 需要 AppIndicator；使用選單開啟主視窗 | 需要 AppIndicator；使用選單開啟主視窗 |
| 憑證庫 | Windows Credential Manager | Secret Service | Secret Service |

Wayland 不允許一般應用程式注入鍵盤事件。

Wayland 的全域快捷鍵需要合成器或桌面入口整合。

本版在 Wayland 停用全域快捷鍵。

Wayland 的浮動球置頂會受合成器限制。

Tauri 在 Linux 不提供托盤左鍵事件。

Linux 使用者可從托盤選單開啟主視窗。

## 遷移驗收狀態

完整遷移範圍、完成定義與逐項驗收方式請見 [ConvertZZ 2.0 遷移計畫與驗收清單](docs/MIGRATION_PLAN.md)。

| 範圍 | 狀態 | 說明 |
| --- | --- | --- |
| Vue、Element Plus 與 Tauri 2 外殼 | 已完成 | 主視窗、浮動球、托盤與單一執行個體已接通。 |
| 新式、舊字典與 ZhConvert 引擎 | 已完成 | 黃金測試與服務模擬測試已建立。 |
| 檔案與檔名安全寫入 | 已完成 | 預覽、確認、衝突、暫存、回復與符號連結規則已實作。 |
| MP3、APE、OGG 與 Opus 標籤 | 已實作 | 完整音訊樣本驗收仍需在發行環境執行。 |
| 舊版設定匯入 | 已實作 | 匯入前會詢問並備份。完整平台行為仍待人工驗收。 |
| 舊版文字工具 | 已完成 | HTML、Unicode 跳脫、編碼與全半形工具已接通。 |
| Sidecar 進度事件 | 已完成 | 檔案與音訊作業會送出中間進度與最終結果。 |
| 發行包乾淨環境驗收 | 部分完成 | Linux AppImage 已通過離線 sidecar、轉換、APE 與 OGG 實包驗證。仍需乾淨虛擬機與 Windows 驗收。 |
| 舊 WPF 專案移除 | 尚未執行 | 會在上述驗收完成後移除。 |

## Linux 使用者相依

一般使用者不需要安裝 Linux 建置套件。

一般使用者不需要安裝 `libayatana-appindicator3-dev`。

README 開頭列出的 `apt-get install` 套件只供 Ubuntu 建置機使用。

`build-essential`、`patchelf`、`pkg-config` 與所有 `*-dev` 套件不屬於使用者端相依。

`rpm` 只用於建立 RPM 產物。

`curl`、`wget` 與 `file` 只用於建置及檢查流程。

建議優先下載 AppImage。

| 發行格式 | 使用者端需求 |
| --- | --- |
| AppImage | 不需要 `*-dev` 套件。系統仍需相容的 Linux 核心與 glibc。部分發行版另需 FUSE 2。 |
| DEB | 請用 APT 安裝。APT 會依套件中繼資料補齊執行函式庫。 |
| RPM | 請用 DNF 或相容套件管理器安裝。套件管理器會補齊執行函式庫。 |

使用者不需要另外安裝 Node.js。

使用者不需要另外安裝 FFmpeg。

使用者不需要另外安裝 TagLib。

AppImage 會封裝 Tauri 收集到的桌面執行函式庫。

AppImage 不會封裝額外的媒體框架。

Linux sidecar 會以 gzip 資源封裝。

啟動時會驗證 SHA-256。

驗證後會解壓到 ConvertZZ 應用程式快取目錄。

這個流程不使用系統 `gzip` 指令。

詳情可參考 [Tauri AppImage 文件](https://v2.tauri.app/distribute/appimage/)。

桌面環境缺少 AppIndicator 支援時仍可使用主視窗。

桌面環境缺少 AppIndicator 支援時可能不會顯示系統托盤。

Tauri 的 Linux 托盤會在執行時載入 Ayatana AppIndicator 或舊版 AppIndicator 函式庫。

DEB 與 RPM 會透過套件中繼資料宣告對應的執行函式庫。

AppImage 會封裝建置時收集到的對應執行函式庫。

部分 GNOME 環境仍需啟用 AppIndicator 或 StatusNotifier 擴充功能。

缺少 Secret Service 時仍可使用轉換功能。

缺少 Secret Service 時不會永久保存 ZhConvert API 金鑰。

每次發行會附上 `RUNTIME-DEPENDENCIES-linux-x64.txt`。

該檔案會列出 DEB 與 RPM 的實際執行相依。

## 舊版設定與命令列

程式首次找到舊版 `ConvertZZ.json` 時會詢問使用者。

使用者同意後只讀取該檔，並將結果另存為 `SettingsV2`。

不會修改舊版 `ConvertZZ.json`。

新設定會存入 Tauri 應用程式設定目錄。

舊版 `Dictionary.csv` 會依 UTF-8 BOM 六欄格式讀取。

`/file` 會開啟檔案預覽。

`/audio` 會開啟音訊標籤預覽。

`/e:l` 代表舊版字典引擎。

`/e:f` 代表 ZhConvert 引擎。

`/e:n` 代表新式分詞引擎。

`/f:t` 代表簡轉繁。

`/f:s` 代表繁轉簡。

`/f:d` 代表不轉換字形。

`/d:t` 代表啟用詞彙修正。

`/d:f` 代表停用詞彙修正。

`/d:s` 代表沿用設定。

`/i:*` 與 `/o:*` 會設定來源與輸出編碼。

未指定 `/file` 時，第二個檔案參數會沿用舊版的輸出路徑語意。

輸入檔名與輸出檔名支援舊版的 `*` 萬用字元對應。

命令列檔案仍會先顯示預覽。

這項行為符合 2.0 版的寫入安全規則。

## 開發與建置環境

需要 Node.js 24。

需要 pnpm 10.26.1。

需要 Rust stable。

Windows 需要 WebView2。

Ubuntu 22.04 建置全部 Linux 發行格式時可安裝下列套件。

```bash
sudo apt-get update
sudo apt-get install -y --no-install-recommends \
  build-essential file libayatana-appindicator3-dev libdbus-1-dev \
  librsvg2-dev libwebkit2gtk-4.1-dev patchelf pkg-config rpm
```

這些套件只供開發與打包使用。

`libayatana-appindicator3-dev` 只供 Linux 托盤的建置與打包使用。

使用者端只需要對應的執行函式庫。

使用者端不需要任何 AppIndicator 開發套件。

套件用途可對照 [Tauri Linux 建置需求](https://v2.tauri.app/start/prerequisites/)。

目前相依圖不使用 libxdo。

目前 Linux 目標不使用 OpenSSL。

因此不需要 `libxdo-dev` 與 `libssl-dev`。

GTK 與 GLib 的開發套件會由 WebKitGTK 開發套件帶入。

不建立 RPM 時可以省略 `rpm`。

不建立 AppImage 時可以省略 `patchelf`。

音訊整合測試使用開發相依 `ffmpeg-static`。

`pnpm install` 會下載測試用 ffmpeg，不必另外安裝系統套件。

該 ffmpeg 不會打進發行包。

安裝依賴。

```bash
pnpm install --frozen-lockfile
```

啟動桌面開發模式。

```bash
pnpm run dev
```

執行型別與單元測試。

```bash
pnpm run check
```

格式化 Rust、Vue 與 Node.js。編輯器存檔與 commit 前也會自動執行。

```bash
pnpm fmt
```

用 QEMU 在乾淨的 Ubuntu 22.04 虛擬機安裝 DEB，確認沒有 Node.js 與 `*-dev`，並離線掃描 APE／OGG。

需要本機的 `qemu-system-x86_64`、`qemu-img`、`genisoimage` 或 `xorriso`，以及先建立好的 Linux 發行檔。

```bash
pnpm tauri build --bundles deb,appimage
pnpm run test:qemu
```

映像會快取於 `tests/.cache/qemu`。第一次會依序嘗試台灣的 TWDS、NCHC 鏡像，最後才連 Ubuntu 官方站。之後重跑只開虛擬機。

若官方站逾時，可自行下載 `jammy-server-cloudimg-amd64.img` 後指定路徑：

```bash
export CONVERTZZ_QEMU_IMAGE=$HOME/Downloads/jammy-server-cloudimg-amd64.img
pnpm run test:qemu
```

建立 sidecar。

```bash
pnpm run sidecar:build
```

建立目前平台的安裝包。

```bash
pnpm run tauri:build
```

`@yao-pkg/pkg` 會將 Node.js 24 與 sidecar 一起封裝。

## 發行

Windows x64 會產生 NSIS 與 MSI。

Linux x64 會產生 AppImage、DEB 與 RPM。

Linux 基準環境是 Ubuntu 22.04。

GitHub Actions 會建立草稿 Release。

每個發行檔會附帶 SHA-256。

第一版安裝包不含作業系統程式碼簽章。

Windows 安裝程式與 Linux AppImage 支援應用程式內自動更新。

更新檔會以 minisign 公鑰驗證。

DEB 與 RPM 不會自動覆寫，程式會改開啟 GitHub Releases。

發行工作流程會簽署更新產物並上傳 `latest.json`。

金鑰產生、GitHub Secrets 與發行注意事項請見 [自動更新金鑰與發行說明](docs/AUTO_UPDATE.md)。

第三方授權請見 [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md)。

舊 WPF 原始碼會保留至 2.0 驗收完成。

驗收完成後才會移除舊專案。

## 授權

ConvertZZ 使用 GPL-3.0-only。

完整條款請見 [LICENSE](LICENSE)。
