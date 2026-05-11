#!/usr/bin/env python3
"""Normalize the fabkury/atcd WHO ATC-DDD CSV into the anamnez schema.

Input  : raw/WHO_ATC-DDD_2026-04-25.csv  (schema: atc_code,atc_name,ddd,uom,adm_r,note)
Output : normalized.csv                  (schema: atc_code,description_en,description_tr,
                                                   description_tr_source)

Some atc_codes appear multiple times in the source (level-5 codes with multiple
DDD / route / uom variants). We collapse to one row per atc_code, keeping the
first atc_name encountered (they are identical across rows for the same code).
description_tr starts empty (description_tr_source also empty); later pipeline
steps (backfill_tr_from_titck.py, merge_llm_translations.py) fill it and set
the source accordingly ('titck' or 'llm').
"""
from __future__ import annotations
import csv
from pathlib import Path

HERE = Path(__file__).resolve().parent
SRC = HERE / "raw" / "WHO_ATC-DDD_2026-04-25.csv"
DST = HERE / "normalized.csv"


def main() -> None:
    seen: dict[str, str] = {}
    order: list[str] = []
    with SRC.open(newline="", encoding="utf-8") as fh:
        reader = csv.DictReader(fh)
        for row in reader:
            code = (row.get("atc_code") or "").strip()
            name = (row.get("atc_name") or "").strip()
            if not code:
                continue
            if code not in seen:
                seen[code] = name
                order.append(code)
            else:
                # Sanity: if names diverge for the same code, prefer the
                # longer / non-empty one but warn.
                if name and name != seen[code] and len(name) > len(seen[code]):
                    seen[code] = name

    # ATC sort: by length first (shorter prefixes come first), then lexical.
    # This places L1 (1 char) before L2 (3 chars) before L3 (4) before L4 (5)
    # before L5 (7), which makes the file easier to read.
    order.sort(key=lambda c: (len(c), c))

    with DST.open("w", newline="", encoding="utf-8") as fh:
        writer = csv.writer(fh)
        writer.writerow(
            ["atc_code", "description_en", "description_tr", "description_tr_source"]
        )
        for code in order:
            writer.writerow([code, seen[code], "", ""])

    # Summary
    by_level = {1: 0, 3: 0, 4: 0, 5: 0, 7: 0}
    other = 0
    for code in order:
        n = len(code)
        if n in by_level:
            by_level[n] += 1
        else:
            other += 1
    print(f"wrote {DST} with {len(order)} unique ATC codes")
    print(f"  L1 (1-char): {by_level[1]}")
    print(f"  L2 (3-char): {by_level[3]}")
    print(f"  L3 (4-char): {by_level[4]}")
    print(f"  L4 (5-char): {by_level[5]}")
    print(f"  L5 (7-char): {by_level[7]}")
    if other:
        print(f"  unexpected lengths: {other}")


if __name__ == "__main__":
    main()
