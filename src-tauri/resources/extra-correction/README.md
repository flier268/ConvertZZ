# extra-correction

ConvertZZ 額外修正層。在 `ws-segment-rs`／`cjk-convert-rs` 套件字典載入之後套用。

字形特例與固定整詞（璇／疱／么、和牛、胜肽、里長、台灣 xx縣／xx市／xx鄉／xx鎮／xx里、一簡多繁、異體 skip）在 `../conversion-specials/`，不在本目錄。

與套件字典同一趟分詞：`zht.corpus.dict.txt` 是分詞表（`詞|詞性|權值`，含簡繁詞形），`zht.corpus.synonym.txt` 是同義詞（`正字,錯字,...`，與套件 `synonym.txt` 相同）。不寫入套件詞典檔，也不在同義詞列上加 `|POS`。字形後只對已分出的整詞查 extra 同義詞，不再分第二次。`roundtrip-dict` 產出分詞表時會寫入分詞器詞性與簡體詞形，並把 conversion-specials 的 `pin`／`word=`／`place-names.txt` 整詞寫回分詞表（和牛、胜肽、里長、三星鄉、莊敬里等）。

`roundtrip-dict` 預設寫到倉庫根目錄的 `data/roundtrip-correction/`，**不會**自動放到這裡。檢視 `pairs.tsv` 後用 `merge-extra` 合併核准列，**不要整包覆蓋**。

正字用台灣常用寫法（為／裡／說／啟／眾、台灣、麵包、日誌、機率）。`一个,一個`、`日志,日誌` 這類左簡右繁要**反過來**寫成 `一個,一个`、`日誌,日志`，讓 ConvertZZ 在同一趟分詞把語料錯字改回常用繁體。`roundtrip-dict` 寫入產出後會自動產生 `synonym-orientation-min.tsv`／`synonym-orientation-full.tsv`；也可對本目錄跑 `check-synonym-orientation`（預設 min，`--full` 擴大一簡多繁並需人工覆核）。異體與一簡多繁只收整詞，不做單字取代（避免 公里、南庄、茶几、秘密、皇后）。方位詞（這裡／那裡／哪裡／裡面／家裡）在分詞表標方位詞性，或單字「里」本身是方位詞才轉「裡」；不因前面是名詞就改。公里、里民、里辦、里長、里名、巴西里維持「里」。「表／錶」只收整詞（手錶、水錶、鐘錶）；表現、表面、表示不改錶。了若指掌、了解、簡單明瞭收整詞；不要把「明了／明瞭」互改（說明了是助詞「了」，簡單明瞭是成語）。人名杰／傑（彭傑燊）不進同義詞，那不是簡繁對錯。胜肽、膿疱維持用字，不改勝肽、膿皰。`roundtrip-dict` 會丟掉跨分詞邊界的假詞對（避免「四|隻有」對成「只有」）。單字「里→裡」會與前後單字粘成 2 字詞（本里、里辦）再寫入分詞表與同義詞；不跟「房子」「垃圾車」這類多字鄰居粘。同義詞導向檢查（`synonym-orientation-*.tsv`）只供人工覆核，不會自動篩掉條目。套用 extra 時依分詞器在上下文標的詞性，不走套件 `convert_synonym` 直接換詞。

檢視 `pairs.tsv` 後用 `merge-extra` 合併，不要整包覆蓋：

```bash
src-tauri/target/release/roundtrip-dict merge-extra \
  --from data/roundtrip-correction \
  --into src-tauri/resources/extra-correction
```

只放這兩個檔。不要 `cp -r` 整個 `data/roundtrip-correction`（內含 `checkpoint.json` 與 `state/`）。不要把檔案寫入 `../segment-dict`。那個目錄屬於套件，不屬這層修正。也不要把 `--output` 指到本目錄。

`roundtrip-dict --extra-correction` 讀本目錄的 synonym 錯詞，探針產出裡略過已知保護詞。請輸出到別的目錄（例如 `data/roundtrip-llm-probe`），不要把探針產出複製回來。產生產品層時不要加該旗標。
