#!/usr/bin/env bash
# 把 dcard 語料裡混入的簡體轉回繁體（台灣用字）。
# 整詞：scripts/dcard-simp-to-trad.pairs.txt（從 synonym 逐筆核對）。
# 單字：只收這份 dcard 實際出現、且已對過上下文的簡化字。
# 不轉：爲/裏/崐、群→羣、秘→祕、灶→竈、霉→黴、么兒、許恒、頂庄。
#
# 用法：
#   scripts/dcard-simp-to-trad.sh
#   scripts/dcard-simp-to-trad.sh /path/to/dcard.txt
#   scripts/dcard-simp-to-trad.sh /path/to/dcard.txt /path/to/out.txt
#   scripts/dcard-simp-to-trad.sh --dry-run /path/to/dcard.txt
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
PAIRS="$ROOT/scripts/dcard-simp-to-trad.pairs.txt"
DEFAULT_SRC="/home/kumei/Desktop/FlowKey/.flowkey-dictionary-sources/dcard/dcard.txt"

DRY_RUN=0
ARGS=()
for a in "$@"; do
  if [[ "$a" == "--dry-run" || "$a" == "-n" ]]; then
    DRY_RUN=1
  else
    ARGS+=("$a")
  fi
done

SRC="${ARGS[0]:-$DEFAULT_SRC}"
DST="${ARGS[1]:-$SRC}"

if [[ ! -f "$SRC" ]]; then
  echo "找不到語料：$SRC" >&2
  exit 1
fi
if [[ ! -f "$PAIRS" ]]; then
  echo "找不到對照表：$PAIRS" >&2
  exit 1
fi

export CONVERTZZ_S2T_SRC="$SRC"
export CONVERTZZ_S2T_DST="$DST"
export CONVERTZZ_S2T_PAIRS="$PAIRS"
export CONVERTZZ_S2T_DRY="$DRY_RUN"

python3 - << 'PY'
import os
from pathlib import Path

src = Path(os.environ["CONVERTZZ_S2T_SRC"])
dst = Path(os.environ["CONVERTZZ_S2T_DST"])
pairs_path = Path(os.environ["CONVERTZZ_S2T_PAIRS"])
dry = os.environ["CONVERTZZ_S2T_DRY"] == "1"

# 只收這份 dcard 實際出現、且已對過上下文的簡化字。
# 啟用台灣字形（啟不是啓）。不轉 霉/灶/虱/么/恒/庄/云/于/秘。
CHARS = {
    "与": "與",
    "丢": "丟",
    "两": "兩",
    "个": "個",
    "们": "們",
    "会": "會",
    "党": "黨",
    "内": "內",
    "别": "別",
    "务": "務",
    "动": "動",
    "县": "縣",
    "双": "雙",
    "变": "變",
    "员": "員",
    "启": "啟",
    "国": "國",
    "场": "場",
    "坚": "堅",
    "实": "實",
    "对": "對",
    "层": "層",
    "属": "屬",
    "峡": "峽",
    "帮": "幫",
    "并": "並",
    "开": "開",
    "张": "張",
    "强": "強",
    "录": "錄",
    "总": "總",
    "户": "戶",
    "执": "執",
    "担": "擔",
    "拥": "擁",
    "挟": "挾",
    "撑": "撐",
    "敌": "敵",
    "来": "來",
    "构": "構",
    "极": "極",
    "没": "沒",
    "泽": "澤",
    "济": "濟",
    "湾": "灣",
    "点": "點",
    "独": "獨",
    "猪": "豬",
    "环": "環",
    "现": "現",
    "监": "監",
    "盘": "盤",
    "着": "著",
    "积": "積",
    "苏": "蘇",
    "虚": "虛",
    "观": "觀",
    "视": "視",
    "觉": "覺",
    "议": "議",
    "论": "論",
    "说": "說",
    "请": "請",
    "财": "財",
    "质": "質",
    "资": "資",
    "转": "轉",
    "过": "過",
    "还": "還",
    "这": "這",
    "进": "進",
    "选": "選",
    "邓": "鄧",
    "阳": "陽",
    "际": "際",
    "陆": "陸",
    "长": "長",
    "间": "間",
    "黄": "黃",
    "绿": "綠",
    "纪": "紀",
    "厕": "廁",
    "亿": "億",
    "脚": "腳",
    "亲": "親",
    "哔": "嗶",
    "倾": "傾",
    "态": "態",
    "关": "關",
    "剰": "剩",
    "専": "專",
    "実": "實",
}

word_pairs = []
for line in pairs_path.read_text(encoding="utf-8").splitlines():
    line = line.strip()
    if not line or line.startswith("#"):
        continue
    left, right = line.split(",", 1)
    if left != right:
        word_pairs.append((left, right))
word_pairs.sort(key=lambda x: len(x[0]), reverse=True)

text = src.read_text(encoding="utf-8")
original = text
word_hits = []
for a, b in word_pairs:
    n = text.count(a)
    if n:
        text = text.replace(a, b)
        word_hits.append((a, b, n))

out = []
counts = {}
for ch in text:
    repl = CHARS.get(ch)
    if repl and repl != ch:
        out.append(repl)
        counts[ch] = counts.get(ch, 0) + 1
    else:
        out.append(ch)
text = "".join(out)
char_hits = sorted(counts.items(), key=lambda x: (-x[1], x[0]))

print(f"來源：{src}")
print(f"整詞替換 {len(word_hits)} 種：")
for a, b, n in word_hits:
    print(f"  {n:4d}  {a} → {b}")
print(f"單字替換 {sum(n for _, n in char_hits)} 次、{len(char_hits)} 種：")
for ch, n in char_hits:
    print(f"  {n:4d}  {ch} → {CHARS[ch]}")
print(f"原文 {len(original)} 字，結果 {len(text)} 字，內容{'有' if text != original else '無'}變更")

if dry:
    print("dry-run：未寫入")
else:
    if dst.resolve() == src.resolve():
        bak = src.with_suffix(src.suffix + ".bak")
        if not bak.exists():
            bak.write_text(original, encoding="utf-8")
            print(f"備份：{bak}")
    dst.write_text(text, encoding="utf-8")
    print(f"寫入：{dst}")
PY
