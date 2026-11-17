---
name: conversion-specials
description: >
  只在使用者要新增或修改 ConvertZZ 字形特例時使用：改
  conversion-specials/rules.txt 或 place-names.txt、加入 skip／variant／s2t-multi／t2s-multi／pin
  規則。觸發：加入特例、修改特例、改 rules.txt、台灣縣市鄉鎮里名稱、/conversion-specials。
  不要在一般轉換、extra-correction、roundtrip、除錯既有行為時套用。
---

# 撰寫 conversion-specials

只在本次任務要**新增或修改**字形特例時跟這份 skill。討論轉換結果、修 extra-correction、跑 roundtrip、或只讀現有規則，不要套用。

字形特例只改 `src-tauri/resources/conversion-specials/rules.txt`。台灣完整「xx縣／xx市／xx鄉／xx鎮／xx里」放 `place-names.txt`（用 `fetch-place-names.py` 更新），不要把幾千個地名寫進 `rules.txt`。欄位、`when` 語法與現況案例以同目錄 [README.md](../../../src-tauri/resources/conversion-specials/README.md) 為準；改規則前先讀完。不要把字寫進 `conversion.rs`，也不要寫進 `segment-dict`。

## 放哪一層

| 情況 | 放哪 |
| --- | --- |
| 單字、有上下文的一簡多繁／一繁多簡、異體、引擎錯映射 | `rules.txt` |
| 固定整詞保護（和牛、胜肽、里長） | `rules.txt` 的 `pin` 或 `word=`／`word^=`（分詞釘入、轉換再套用） |
| 台灣完整縣／市／鄉／鎮／里名 | `place-names.txt`（分詞釘入、簡轉繁拉回、roundtrip-dict 寫回 extra） |
| 語料產生的整詞同義（手錶、範圍、機制）＋詞性 | `extra-correction` 同義詞／分詞表 |
| 語意取代清單或 `skip: "…"` 字串 | 不准；改 `rules.txt` |

extra 已有整詞＋詞性的（只／隻、制／製），不要再在這裡做無條件單字取代。需要單字量詞時用 `s2t-multi`＋`pos=`。

## 選 kind

| 目標 | kind | 注意 |
| --- | --- | --- |
| 引擎會改、我們要維持原字（璇不要變璿） | `skip` | `to` 留空。不要再 `variant` 把目標改回來（璿／璇是不同人名） |
| 無條件單字正規化（皰→疱） | `variant`／`s2t`／`t2s` | 一對一單字走字形表，不靠分詞 |
| 同一簡體依上下文選不同繁體 | `s2t-multi` | 有 `when` 的例外必須寫在無條件預設**上面** |
| 同一繁體依上下文選不同簡體 | `t2s-multi` | 若引擎會先改掉來源字，先對該字 `skip` 該 `dir` |
| 只要整詞不被切開、不必改字 | `pin` | `to` 留空。2 字以上才釘入 |

`dir`：`s2t`／`t2s`／`both`。省略時 `skip`／`s2t`／`variant`／`s2t-multi` 預設 `s2t`。

## 寫一行

UTF-8，**欄位用 Tab，不是空白**。`from` 多個來源用 `|`。`when` 多條件用 `;`（AND），同一條件內 `|` 為 OR。`#`／`//` 為註解。

```
kind<TAB>from<TAB>to<TAB>dir<TAB>when
```

同一方向、能對上 `from` 的規則由上而下第一條命中即停。出現在帶 `when` 的 `from` 裡的字不會進字形 override 表（避免「老么」在分詞前變成「老麼」）。

`when` 只准 README 列出的鍵：`word=`、`word^=`、`next=`、`next^=`、`ch0=`、`ch1=`、`pos=`、`prev-pos=`。不要發明新欄位；`pos` 名稱與 extra-correction 相同（`D_MQ`、`A_Q`、`D_MQ+A_Q`）。

寫完用真實 Tab 檢查，例如：

```bash
python3 -c "
from pathlib import Path
p = Path('src-tauri/resources/conversion-specials/rules.txt')
for i, line in enumerate(p.read_text().splitlines(), 1):
    s = line.strip()
    if not s or s.startswith('#') or s.startswith('//'):
        continue
    print(i, line.split('\t'))
"
```

## 驗證

1. 在 `src-tauri/src/core/conversion/specials.rs` 的 `bundled_rules_parse`（或新測試）鎖定新行為。
2. 需要整句／分詞時補 `src-tauri/src/core/conversion/tests.rs` 黃金案例。
3. 跑：

```bash
cargo test --manifest-path src-tauri/Cargo.toml --lib core::conversion::specials
cargo test --manifest-path src-tauri/Cargo.toml --lib glyph_s2t
```

規則已載入後，`OnceLock` 不會重讀；改 `rules.txt` 後相關測試要新進程。發行包路徑與 `CONVERTZZ_CONVERSION_SPECIALS` 覆寫見 README，一般改倉庫檔即可。
