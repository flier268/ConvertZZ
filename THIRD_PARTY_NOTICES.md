# 第三方授權聲明

ConvertZZ 2.0 包含第三方自由軟體。

以下清單列出主要執行期元件。

| 元件 | 版本 | 授權 |
| --- | --- | --- |
| Vue | 3.5.41 | MIT |
| Element Plus | 2.14.4 | MIT |
| Tauri | 2.11 | Apache-2.0 OR MIT |
| cjk-conv | 1.2.150 | ISC |
| novel-segment | 2.7.121 | ISC |
| taglib-wasm | 2.0.0 | MIT |
| TagLib WASM | 2.3.1 | LGPL-2.1-or-later |
| mp3tag.js | 3.17.0 | MIT |
| chardet | 2.2.0 | MIT |
| encoding-japanese | 2.2.0 | MIT |
| iconv-lite | 0.7.3 | MIT |
| enigo | 0.6 | MIT |
| keyring | 3 | Apache-2.0 OR MIT |
| flate2 | 1.1.9 | Apache-2.0 OR MIT |
| sha2 | 0.10.9 | Apache-2.0 OR MIT |

完整 JavaScript 相依版本記錄於 `pnpm-lock.yaml`。

完整 Rust 相依版本記錄於 `src-tauri/Cargo.lock`。

`taglib-wasi.wasm` 包含以 LGPL-2.1-or-later 發布的 TagLib 程式碼。

發行包會附帶 taglib-wasm 的授權文件。

發行包會附帶 TagLib 的 LGPL 完整條款。

ConvertZZ 未修改 TagLib WASM。

對應原始碼可由 [taglib-wasm 專案](https://github.com/CharlesWiltgen/TagLib-Wasm) 取得。

音訊整合測試使用開發相依 `ffmpeg-static`（GPL-3.0-or-later）。

該 ffmpeg 只存在於開發與 CI 環境。

發行包不會包含 ffmpeg。

測試樣本 `tests/fixtures/mac-399.ape` 與 `tests/fixtures/test.ogg` 來自 TagLib 測試資料。

這些樣本只用於測試。

這些樣本不會包含於發行包。
