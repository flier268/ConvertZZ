# 第三方授權聲明

ConvertZZ 2.0 包含第三方自由軟體。

以下清單列出主要執行期元件。

| 元件 | 版本 | 授權 |
| --- | --- | --- |
| Vue | 3.5.41 | MIT |
| Element Plus | 2.14.4 | MIT |
| Tauri | 2.11 | Apache-2.0 OR MIT |
| tauri-plugin-updater | 2.10.1 | Apache-2.0 OR MIT |
| tauri-plugin-process | 2.3.1 | Apache-2.0 OR MIT |
| cjk-convert-rs | 0.1.0 | ISC |
| ws-segment-rs | 0.1.4 | MIT |
| encoding_rs | 0.8.35 | Apache-2.0 OR MIT |
| chardetng | 1.0.0 | Apache-2.0 OR MIT |
| id3 | 1.17 | MPL-2.0 |
| lofty | 0.25 | MIT OR Apache-2.0 |
| reqwest | 0.13 | Apache-2.0 OR MIT |
| enigo | 0.6.1 | MIT |
| arboard | 3.6.1 | Apache-2.0 OR MIT |
| keyring | 4 | Apache-2.0 OR MIT |

完整 JavaScript 相依版本記錄於 `pnpm-lock.yaml`。

完整 Rust 相依版本記錄於 `src-tauri/Cargo.lock`。

`cjk-convert-rs` 對照表來自 `cjk-conv` 與 `@lazy-cjk/*`（ISC）。

`ws-segment-rs` 分詞字典來自 `segment-dict`，發行包會附帶 `src-tauri/resources/segment-dict`。

`src-tauri/resources/conversion-specials/place-names.txt` 的縣／市／鄉／鎮／里名稱來自內政部國土測繪中心代碼服務（政府資料開放授權條款第1版）。

測試樣本 `tests/fixtures/mac-399.ape` 與 `tests/fixtures/test.ogg` 來自 TagLib 測試資料。

這些樣本只用於測試。

這些樣本不會包含於發行包。
