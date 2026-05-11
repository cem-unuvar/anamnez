#!/usr/bin/env python3
"""Extract chunked LOINC rows for LLM translation.

Selects rows that are:
  - empty `long_name_tr` in normalized.csv, AND
  - CLASSTYPE=1 (laboratory class) in raw/LoincTable/Loinc.csv.

Sorts by (CLASS, code), then fills chunks of CHUNK_SIZE rows sequentially.
Chunks can span multiple classes; each row carries its CLASS in the file so
the agent sees what kind of code it is. Output: raw/llm_inputs/chunk_NN.csv.

Each chunk file has schema: code,long_name_en,class
(class is informational for the agent prompt; not used by the merge.)

Idempotent. The raw/llm_inputs/ directory is wiped and rebuilt on each run.
"""
import csv
import os
import shutil
import sys
from collections import defaultdict

csv.field_size_limit(sys.maxsize)

HERE = os.path.dirname(os.path.abspath(__file__))
NORM = os.path.join(HERE, "normalized.csv")
LOINC_RAW = os.path.join(HERE, "raw", "LoincTable", "Loinc.csv")
OUT_DIR = os.path.join(HERE, "raw", "llm_inputs")
CHUNK_SIZE = 500


def main() -> int:
    empty_codes: dict[str, str] = {}
    with open(NORM, encoding="utf-8", newline="") as f:
        for row in csv.DictReader(f):
            if (row.get("long_name_tr") or "").strip():
                continue
            empty_codes[row["code"]] = row["long_name_en"]

    by_class: dict[str, list[tuple[str, str]]] = defaultdict(list)
    with open(LOINC_RAW, encoding="utf-8", newline="") as f:
        for row in csv.DictReader(f):
            code = (row.get("LOINC_NUM") or "").strip()
            if code not in empty_codes:
                continue
            if (row.get("CLASSTYPE") or "").strip() != "1":
                continue
            cls = (row.get("CLASS") or "").strip() or "UNKNOWN"
            by_class[cls].append((code, empty_codes[code]))

    all_rows: list[tuple[str, str, str]] = []
    for cls in sorted(by_class.keys()):
        for code, en in sorted(by_class[cls], key=lambda x: x[0]):
            all_rows.append((cls, code, en))

    if os.path.isdir(OUT_DIR):
        shutil.rmtree(OUT_DIR)
    os.makedirs(OUT_DIR, exist_ok=True)

    chunks = 0
    for i in range(0, len(all_rows), CHUNK_SIZE):
        chunk = all_rows[i : i + CHUNK_SIZE]
        n = (i // CHUNK_SIZE) + 1
        path = os.path.join(OUT_DIR, f"chunk_{n:02d}.csv")
        with open(path, "w", encoding="utf-8", newline="") as f:
            w = csv.writer(f)
            w.writerow(["code", "long_name_en", "class"])
            for cls, code, en in chunk:
                w.writerow([code, en, cls])
        chunks += 1

    print(f"wrote {chunks} chunk files covering {len(all_rows)} rows into {OUT_DIR}")
    print(f"  classes spanned: {len(by_class)}")
    print("  top classes by row count:")
    for cls, rs in sorted(by_class.items(), key=lambda x: -len(x[1]))[:15]:
        print(f"    {len(rs):>6}  {cls}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
