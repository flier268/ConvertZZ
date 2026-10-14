#!/usr/bin/env python3
"""從內政部國土測繪中心代碼服務下載台灣縣／市／鄉／鎮／里完整名稱，寫入 place-names.txt。

只收完整「xx縣」「xx市」「xx鄉」「xx鎮」「xx里」，不收區／村或單字。
授權：政府資料開放授權條款第1版。
"""

from __future__ import annotations

import argparse
import re
import sys
import urllib.request
import xml.etree.ElementTree as ET
from pathlib import Path

API_COUNTY = "https://api.nlsc.gov.tw/other/ListCounty"
API_TOWN = "https://api.nlsc.gov.tw/other/ListTown/{code}"
API_VILLAGE = "https://api.nlsc.gov.tw/other/ListVillage/{county}/{town}"
USER_AGENT = "ConvertZZ-place-names/1.0"
CJK = re.compile(r"^[\u4e00-\u9fff]+$")
HERE = Path(__file__).resolve().parent
HEADER = """# 台灣縣／市／鄉／鎮／里完整名稱（固定整詞保護）。
# 來源：內政部國土測繪中心代碼服務 ListCounty／ListTown／ListVillage（政府資料開放授權條款第1版）。
# 更新：python3 src-tauri/resources/conversion-specials/fetch-place-names.py
# 只收完整「xx縣」「xx市」「xx鄉」「xx鎮」「xx里」，不收區／村、單字或不完全名稱。
# 分詞時釘入；轉換與 roundtrip-dict 都讀這份名單。
"""


def fetch(url: str) -> bytes:
    request = urllib.request.Request(url, headers={"User-Agent": USER_AGENT})
    with urllib.request.urlopen(request, timeout=30) as response:
        return response.read()


def parse_items(xml_bytes: bytes, item_tag: str, fields: list[str]) -> list[dict[str, str]]:
    root = ET.fromstring(xml_bytes)
    rows: list[dict[str, str]] = []
    for item in root.findall(f".//{item_tag}"):
        row: dict[str, str] = {}
        for field in fields:
            element = item.find(field)
            row[field] = (element.text or "").strip() if element is not None else ""
        rows.append(row)
    return rows


def normalize(raw: str) -> str:
    return raw.replace("[", "").replace("]", "").strip()


def keep(name: str, *suffixes: str) -> bool:
    return bool(name) and name.endswith(suffixes) and len(name) >= 2 and CJK.fullmatch(name)


def collect() -> tuple[list[str], list[str], list[str], list[str], list[str]]:
    counties = parse_items(fetch(API_COUNTY), "countyItem", ["countycode", "countyname"])
    county_names: list[str] = []
    seen_county = set()
    for county in counties:
        name = normalize(county["countyname"])
        if keep(name, "縣", "市") and name not in seen_county:
            seen_county.add(name)
            county_names.append(name)

    towns: list[dict[str, str]] = []
    for county in counties:
        code = county["countycode"]
        for town in parse_items(fetch(API_TOWN.format(code=code)), "townItem", ["towncode", "townname"]):
            town["countycode"] = code
            towns.append(town)

    town_shi: set[str] = set()
    zhen: set[str] = set()
    xiang: set[str] = set()
    li: set[str] = set()
    for town in towns:
        name = normalize(town["townname"])
        if keep(name, "市") and name not in seen_county:
            town_shi.add(name)
        elif keep(name, "鎮"):
            zhen.add(name)
        elif keep(name, "鄉"):
            xiang.add(name)

    total = len(towns)
    for index, town in enumerate(towns, 1):
        villages = parse_items(
            fetch(API_VILLAGE.format(county=town["countycode"], town=town["towncode"])),
            "village",
            ["villageName"],
        )
        for village in villages:
            name = normalize(village["villageName"])
            if keep(name, "里"):
                li.add(name)
        if index % 80 == 0 or index == total:
            print(
                f"村里 {index}/{total}，縣市 {len(county_names)}，縣轄市 {len(town_shi)}，鎮 {len(zhen)}，鄉 {len(xiang)}，里 {len(li)}",
                file=sys.stderr,
            )

    return county_names, sorted(town_shi), sorted(zhen), sorted(xiang), sorted(li)


def main() -> int:
    parser = argparse.ArgumentParser(description="下載台灣 xx縣／xx市／xx鄉／xx鎮／xx里 完整名稱")
    parser.add_argument(
        "-o",
        "--output",
        type=Path,
        default=HERE / "place-names.txt",
        help="輸出路徑（預設：同目錄 place-names.txt）",
    )
    args = parser.parse_args()
    counties, town_shi, zhen, xiang, li = collect()
    if not counties or not xiang or not li:
        print("沒有收到縣市、鄉或里名稱。", file=sys.stderr)
        return 1
    names = counties + town_shi + zhen + xiang + li
    args.output.write_text(HEADER + "\n".join(names) + "\n", encoding="utf-8")
    print(
        f"寫入 {args.output}：縣市 {len(counties)}，縣轄市 {len(town_shi)}，鎮 {len(zhen)}，鄉 {len(xiang)}，里 {len(li)}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
