# 測試樣本

| 檔案 | 來源 | SHA-256 |
| --- | --- | --- |
| `mac-399.ape` | [TagLib `mac-399.ape`](https://github.com/taglib/taglib/blob/3ace0483c0d7a2a4c1dbf9277274f0e733530504/tests/data/mac-399.ape) | `8c851b4f52bbbd822262040ae8961b8c27f8f702d1be3fa50cca283d009aadea` |
| `test.ogg` | [TagLib `test.ogg`](https://github.com/taglib/taglib/blob/3ace0483c0d7a2a4c1dbf9277274f0e733530504/tests/data/test.ogg) | `b064240a5cca0f9241b942b8d4680cedda555e5aab5f15c1d983767193a7f54d` |

這些樣本只用於測試與乾淨環境驗收，不會包含於發行包。

## 二進位音訊（issue #76）

`测试音乐.mp3` 帶 ID3v2（標題「测试音乐」、藝人「测试艺人」）。`测试音乐b.mp3` 沒有 ID3，檔頭為 MPEG 幀同步 `FF Fx`。兩者都是 1 秒、單聲道、32 kbps 正弦波，用來確認檔案轉換不會把音訊當文字寫回。

```bash
ffmpeg -y -f lavfi -i "sine=frequency=440:duration=1:sample_rate=22050" \
  -ac 1 -c:a libmp3lame -b:a 32k -id3v2_version 3 \
  -metadata title="测试音乐" -metadata artist="测试艺人" \
  -write_id3v1 0 -map_metadata 0 \
  tests/fixtures/测试音乐.mp3

ffmpeg -y -f lavfi -i "sine=frequency=440:duration=1:sample_rate=22050" \
  -ac 1 -c:a libmp3lame -b:a 32k \
  -write_id3v1 0 -id3v2_version 0 -map_metadata -1 \
  tests/fixtures/测试音乐b.mp3
```
