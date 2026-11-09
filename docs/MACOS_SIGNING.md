# macOS 簽名與公證

這份文件說明如何讓 ConvertZZ 通過 Gatekeeper，使用者開啟時不再出現「Apple 無法驗證是否為惡意軟體」。

這與應用程式內自動更新的 minisign 金鑰無關。minisign 見 [自動更新金鑰與發行說明](AUTO_UPDATE.md)。

## 為什麼會出現這則提示

目前發行的 Universal DMG 若沒有 **Developer ID Application** 簽名並經 Apple **公證**，從瀏覽器或 GitHub 下載後，macOS 會把應用程式標成未識別開發者。

macOS 15 的對話框通常只有「完成」與「移到垃圾桶」，沒有「打開」。

沒有付費 Apple Developer 帳號（每年 99 美元）就無法公證。免費帳號只能本機測試，無法消除這則提示。

## 使用者暫時開啟方式

1. 把 `ConvertZZ.app` 拖到「應用程式」資料夾。
2. 打開「系統設定 → 隱私權與安全性」，捲到頁面下方，按「仍要打開」。
3. 或在終端機執行：

```bash
xattr -cr /Applications/ConvertZZ.app
open /Applications/ConvertZZ.app
```

## 發行時要準備什麼

到 [Apple Developer](https://developer.apple.com) 申請付費帳號後：

1. 在 Mac 上產生 CSR，到 Certificates, Identifiers & Profiles 建立 **Developer ID Application** 憑證。
2. 把憑證裝進登入鑰匙圈，匯出為有密碼的 `.p12`。
3. 轉成單行 Base64：

```bash
openssl base64 -A -in ConvertZZ-Developer-ID.p12 -out certificate-base64.txt
```

4. 公證認證擇一：
   - Apple ID：帳號、[app-specific password](https://support.apple.com/en-ca/HT204397)、Team ID
   - App Store Connect API：Key ID、Issuer ID、一次下載的 `.p8`

官方步驟見 [Tauri macOS Code Signing](https://v2.tauri.app/distribute/sign/macos/)。

## GitHub Secrets

到倉庫 **Settings → Secrets and variables → Actions** 新增：

| Secret | 內容 |
| --- | --- |
| `APPLE_CERTIFICATE` | `.p12` 的 Base64 全文 |
| `APPLE_CERTIFICATE_PASSWORD` | 匯出 `.p12` 時的密碼 |
| `APPLE_SIGNING_IDENTITY` | 可選。鑰匙圈裡的身分名稱，例如 `Developer ID Application: Name (TEAMID)`。未設時由憑證推斷 |
| `APPLE_ID` | Apple ID 電子郵件。與 API 金鑰二選一 |
| `APPLE_PASSWORD` | Apple ID 的 app-specific password |
| `APPLE_TEAM_ID` | 開發者 Team ID |
| `APPLE_API_KEY` | App Store Connect Key ID。與 Apple ID 二選一 |
| `APPLE_API_ISSUER` | App Store Connect Issuer ID |
| `APPLE_API_KEY_P8` | `AuthKey_<KeyID>.p8` 檔案內容 |

只設憑證、沒有 Apple ID 或 API 金鑰時，Tauri 會簽名但跳過公證，Gatekeeper 仍會擋。兩組都要有。

沒有這些 Secrets 時，Release 仍會產出未公證的 DMG。

## 發行時會發生什麼

`pnpm tauri build` 在 macOS 會讀取上述環境變數，為 Universal `.app` 套用 hardened runtime、送公證並 staple。

設定 `APPLE_CERTIFICATE` 後，工作流程會確認簽章是 Developer ID Application。

同時有公證帳號時，會再跑 `spctl --assess` 與 `stapler validate`。

本機若鑰匙圈已有憑證：

```bash
export APPLE_SIGNING_IDENTITY="Developer ID Application: Name (TEAMID)"
export APPLE_ID="you@example.com"
export APPLE_PASSWORD="app-specific-password"
export APPLE_TEAM_ID="TEAMID"
pnpm tauri build --bundles app,dmg --target universal-apple-darwin
```
