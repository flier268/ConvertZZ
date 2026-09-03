# conversion-specials

ConvertZZ 字形特例。取代 `conversion.rs` 裡的硬編碼（璇／疱／么），用同一份規則描述：

| kind | 用途 |
| --- | --- |
| `s2t` | 簡轉繁（無條件單字可進字形表） |
| `t2s` | 繁轉簡 |
| `s2t-multi` | 一簡多繁（依上下文選繁體） |
| `t2s-multi` | 一繁多簡（依上下文選簡體） |
| `variant` | 異體正規化（例如台灣醫學用疱，不用皰） |
| `skip` | 字形引擎（`cn2tw_min`／`tw2cn`）不要改這個字 |
| `pin` | 固定整詞：分詞時釘入，避免被切開（和牛）。`to` 可留空 |

這層與套件 `segment-dict`、語料回環 `extra-correction` 分開。語料產生的整詞同義詞仍放 extra-correction。`pin` 與 `word=`／`word^=` 列出的 2 字以上詞會在分詞時釘入（固定 extra），轉換繁體時仍走 skip／s2t-multi。不要把規則寫進 `conversion.rs`。

## 檔案

`rules.txt`，UTF-8，**欄位用 Tab 分隔**：

```
kind<TAB>from<TAB>to<TAB>dir<TAB>when
```

`#` 或 `//` 開頭為註解。空白行忽略。

- `from`：來源字或詞；`|` 表示多個來源（`么|幺`）
- `to`：目標；`skip` 可留空
- `dir`：`s2t`／`t2s`／`both`。省略時 `skip`／`s2t`／`variant`／`s2t-multi` 預設 `s2t`，`t2s`／`t2s-multi` 預設 `t2s`
- `when`：省略＝一律套用。多個條件用 `;`（AND），同一條件內 `|` 為 OR

`when` 可用：

| 條件 | 意義 |
| --- | --- |
| `word=老么\|么兒` | 整詞等於 |
| `word^=老么\|老幺` | 整詞以此開頭 |
| `next=兒` | 下一詞等於 |
| `next^=兒\|女` | 下一詞以此開頭 |
| `ch0=么\|幺` | 本詞第一字 |
| `ch1=兒\|女` | 本詞第二字 |
| `pos=D_MQ+A_Q` | 本詞詞性（與 extra-correction 相同） |
| `prev-pos=A_M` | 上一詞詞性 |

同一方向、能對上 `from` 的規則**由上而下第一條命中即停**。有 `when` 的例外要寫在無條件預設前面。

有上下文的字（出現在帶 `when` 的 `from`）不會寫進字形 override 表，以免分詞前就把「老么」變成「老麼」。無條件且一對一的單字（異體 `皰→疱`）走字形表，不依賴分詞。

## 現況

- **璇**：`cn2tw_min` 會改成璿；人名曾依璇與孫運璿是不同字，只 `skip`，不互改。
- **疱**：台灣醫學常用疱，`skip` 避免改成皰；異體皰正規成疱。
- **么**：語氣／疑問用麼；老么、么兒、么女等排行保留么（幺在這些詞正規成么）。
- **和牛**：`pin`，避免「和牛只剩」切成「牛只」。
- **胜肽**：`word=胜肽|勝肽`，分詞釘整詞，轉換維持胜（不要勝肽）。
- **里長／里名／本里／里辦／里民**：`word=` 釘整詞，村里用「里」不改「裡」。

一簡多繁如「只／隻」「制／製」若已有 extra-correction 整詞＋詞性，不要在這裡再做無條件單字取代。需要時用 `s2t-multi`＋`pos=`。

一繁多簡示例（預設不啟用；`乾隆` 若被引擎改成「干隆」可加）：

```
skip	乾		t2s
t2s-multi	乾	乾	t2s	word^=乾隆|乾坤
t2s-multi	乾	干	t2s
```

執行檔會找 `conversion-specials/rules.txt`（與 extra-correction 同層）。開發時讀 `src-tauri/resources/conversion-specials/rules.txt`。可用 `CONVERTZZ_CONVERSION_SPECIALS` 指到檔案或目錄覆寫。
