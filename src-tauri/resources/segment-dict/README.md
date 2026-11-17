# segment-dict

`ws-segment-rs` 預設字典資料，來自 `segment-dict` 套件。

執行期會透過 `NOVEL_SEGMENT_DICT_ROOT` 指向這個目錄。

只收錄 `ws-segment-rs` `use_default_dicts` 與 `ws-segment-rs-dict` 會讀取的檔案。套件裡的 `jieba-js/`、`nodejieba/`、`segment/jieba/` 不會被載入，因此未收錄；升級 `ws-segment-rs` 時須重新確認載入清單。

ConvertZZ 語料回環額外修正請放 `../extra-correction`，不要改這個目錄。

