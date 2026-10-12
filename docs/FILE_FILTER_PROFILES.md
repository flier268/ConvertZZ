# 副檔名篩選 profile

檔案頁與無頭模式用「篩選 profile」決定資料夾掃描收哪些檔案。profile 有三種模式：

| 模式 | 行為 |
| --- | --- |
| 白名單 | 只收清單內的副檔名；勾選「包含無副檔名檔案」才收無副檔名檔案。 |
| 黑名單 | 排除清單內的副檔名，其餘都收；勾選「同時排除無副檔名檔案」代表**排除**無副檔名檔案（不勾則收）。 |
| 全部 | 不篩選副檔名。 |

## 副檔名的判定

- 只看最後一段，並轉小寫：`a.tar.gz` 是 `.gz`；舊篩選器樣式 `*.tar.gz` 也只取 `.gz`。
- 點開頭的檔案（例如 `.bashrc`）算**無副檔名**，不是副檔名 `.bashrc`。
- 檔名以點結尾（例如 `a.`）也算無副檔名。
- 只有內部暫存檔（`.convertzz-` 開頭）一定略過。其他點開頭的檔案照上述規則處理；
  掃描途中遇到點開頭的**資料夾**（含 `.git`）則整個略過。

## 無頭模式

`--config`／`--globalconfig` 載入設定時，使用 `files.defaultFilterProfileId` 指向的 profile。
找不到該 profile 時，退回由舊 `files.typeFilter` 遷移出的 profile。沒有設定檔時使用預設副檔名。

## 設定驗證

載入設定時會丟棄格式錯誤的 profile：`mode` 只能是 `allow`／`deny`／`all`、`extensions` 須為字串陣列、
`includeNoExtension` 若存在須為布林、`id` 須為不重複的非空字串。Rust 與前端共用
`tests/fixtures/filter-profile-normalize.json` 驗證正規化規則。
