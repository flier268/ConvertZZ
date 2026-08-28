# extra-correction

ConvertZZ 額外修正層。在 `ws-segment-rs`／`cjk-convert-rs` 套件字典載入之後套用。

`roundtrip-dict` 預設寫到倉庫根目錄的 `data/roundtrip-correction/`，**不會**自動放到這裡。檢視 `pairs.tsv` 後再複製：

```bash
cp data/roundtrip-correction/zht.corpus.synonym.txt \
   src-tauri/resources/extra-correction/zht.corpus.synonym.txt
cp data/roundtrip-correction/zht.corpus.dict.txt \
   src-tauri/resources/extra-correction/zht.corpus.dict.txt
```

只放這兩個檔。不要 `cp -r` 整個 `data/roundtrip-correction`（內含 `checkpoint.json` 與 `state/`）。不要把檔案寫入 `../segment-dict`。那個目錄屬於套件，不屬這層修正。也不要把 `--output` 指到本目錄。
