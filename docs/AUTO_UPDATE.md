# 自動更新金鑰與發行說明

這份文件說明 ConvertZZ 應用程式內自動更新的金鑰怎麼產生、放到哪裡，以及發行時要注意什麼。

這組金鑰只用來驗證 `latest.json` 指向的更新檔沒有被改過。

官方步驟見 [Tauri updater 文件](https://v2.tauri.app/plugin/updater/)。

## 金鑰是什麼

自動更新使用 Tauri 的 minisign 金鑰對。

| 檔案或值 | 用途 | 可以進 git 嗎 |
| --- | --- | --- |
| 公鑰 | 安裝包用來驗證更新 | 可以。已寫在 `src-tauri/tauri.conf.json` 的 `plugins.updater.pubkey` |
| 私鑰 | 發行時簽署安裝包與 `.sig` | 不行 |
| 私鑰密碼 | 保護私鑰檔 | 不行 |

目前倉庫已放入一組公鑰。

之後發行必須用**同一把**私鑰簽署。

換私鑰等於換公鑰。舊安裝包無法驗證新簽章。

## 產生金鑰

在專案根目錄執行。

```bash
mkdir -p ~/.tauri
pnpm tauri signer generate -w ~/.tauri/convertzz.key
```

指令會詢問私鑰密碼。

可以留空。

有設密碼時，之後建置與 GitHub Secret 都要提供同一個密碼。

成功後會寫入私鑰檔，並在終端機印出公鑰。

公鑰是一段以 `dW50cnVzdGVk` 開頭的單行文字。

請立刻備份私鑰檔與密碼。

遺失私鑰後，已發行的安裝包無法再收到用新金鑰簽署的更新。

私鑰檔、密碼與終端機輸出都不要提交到 git。

不要貼到 Issue、Pull Request 或聊天室。

## 放入倉庫與 GitHub

### 公鑰

把印出的公鑰整段貼到 `src-tauri/tauri.conf.json`：

```json
"plugins": {
  "updater": {
    "pubkey": "這裡貼公鑰",
    "endpoints": [
      "https://github.com/flier268/ConvertZZ/releases/latest/download/latest.json"
    ]
  }
}
```

倉庫裡已經有目前使用的公鑰。

只有在你要輪替金鑰時才改這一段。

### 私鑰

到 GitHub 倉庫的 **Settings → Secrets and variables → Actions** 新增：

| Secret 名稱 | 內容 |
| --- | --- |
| `TAURI_SIGNING_PRIVATE_KEY` | 私鑰**檔案內容**，不是本機路徑 |
| `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | 產生金鑰時設定的密碼。沒有密碼可留空或不建這個 Secret |

CI 沒有你的家目錄，所以 Secret 必須是私鑰文字本身。

本機查看私鑰內容：

```bash
cat ~/.tauri/convertzz.key
```

沒有設定 `TAURI_SIGNING_PRIVATE_KEY` 時，發行工作流程會失敗。

這是刻意的。

## 發行時會發生什麼

推送 `v*` 標籤，或手動執行 Release 工作流程。

工作流程會讀取上述 Secrets，並加上 `src-tauri/tauri.updater.conf.json`。

該設定會開啟 `createUpdaterArtifacts`。

建置成功後會多出對應的 `.sig`。

`scripts/write-latest-json.mjs` 會收集：

- Windows：`*-setup.exe` 與它的 `.sig`
- Linux：`*.AppImage` 與它的 `.sig`

然後寫出 `latest.json`，一併放到草稿 Release。

更新端點是：

`https://github.com/flier268/ConvertZZ/releases/latest/download/latest.json`

`/releases/latest` **只看已發佈、且不是預發佈的最新 Release**。

程式預設只檢查正式版。設定中的「檢查開發／預發佈版本」開啟後，才會一併比較 alpha、beta、rc 等標籤（例如 `v2.0.0-beta1`），排序為 `2.0.0 > 2.0.0-beta2 > 2.0.0-beta1 > 2.0.0-alpha9`。

含 `-` 的發行標籤（如 `v2.0.0-beta1`）會在 GitHub Release 標成 Pre-release，因此不會成為 `/releases/latest`。

草稿 Release 不會被程式查到。

請先檢查草稿內容，再按 Publish。

## 哪些安裝包能自動更新

| 安裝方式 | 行為 |
| --- | --- |
| Windows NSIS | 先詢問，確認後下載、驗證簽章、安裝並重啟 |
| Linux AppImage | 同上 |
| Linux DEB / RPM | 無法就地覆寫。程式會改開啟 GitHub Releases |

本機 `pnpm run tauri:build` 預設不產生更新簽章。

本機驗收與 QEMU 不需要私鑰。

若要在本機產生已簽署的更新產物：

```bash
export TAURI_SIGNING_PRIVATE_KEY="$HOME/.tauri/convertzz.key"
export TAURI_SIGNING_PRIVATE_KEY_PASSWORD=""
pnpm tauri build --config src-tauri/tauri.updater.conf.json
```

環境變數必須直接設定。

Tauri 不會讀 `.env` 來找這把私鑰。

## 使用者端流程

啟動時若勾選「啟動時檢查更新」，或在關於頁按「檢查更新」：

1. 程式向 `latest.json` 查詢。
2. 有可安裝的新版本時，先詢問使用者。
3. 確認後才下載。
4. 用內建公鑰驗證簽章。
5. 驗證成功才安裝並重啟。

啟動檢查的對話框可勾選「不再詢問此版本」。勾選後按「稍後再說」，該版本不會再於啟動時跳出。之後若有更新的版本仍會提示。關於頁的「檢查更新」不受略過影響。設定頁可清除已略過的版本。

簽署通道失敗時，例如還沒有 `latest.json`，程式會改查 GitHub Releases 頁面。

這時只會開啟下載頁，不會自動寫入安裝目錄。

## 輪替金鑰

只有私鑰遺失、外洩，或確定要作廢舊金鑰時才輪替。

1. 用上面的指令產生新金鑰對。
2. 把新公鑰寫進 `src-tauri/tauri.conf.json` 並發佈一個新版本。
3. 使用者必須先手動安裝這個版本，才帶得走新公鑰。
4. 把 GitHub Secrets 改成新私鑰與新密碼。
5. 之後的更新才能用新私鑰簽署。

仍在使用舊公鑰的安裝包，無法驗證新私鑰簽出的更新。

## 常見問題

**發行工作流程寫「請設定 GitHub Secret TAURI_SIGNING_PRIVATE_KEY」。**

Secret 還沒建，或名稱拼錯。

**程式說已是最新版本，但 GitHub 已有更新。**

Release 可能還是草稿，或標成 Pre-release。

先發佈成正式版。

**簽章驗證失敗。**

發行時用的私鑰與安裝包內的公鑰不是同一對。

不要混用兩組金鑰。

**本機建置沒有 `.sig`。**

這是預期行為。

只有加上 `tauri.updater.conf.json` 且提供私鑰時才會簽署。
