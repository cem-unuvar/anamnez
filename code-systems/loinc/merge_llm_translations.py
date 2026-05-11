#!/usr/bin/env python3
"""Merge LLM-sourced Turkish translations into LOINC normalized.csv.

Reads every CSV under raw/llm_translations/*.csv (committed agent outputs) and
fills long_name_tr where currently empty, setting long_name_tr_source='llm'.
Never overwrites existing translations (the Stage-1 composed names win over
LLM transliteration).

Input CSV schema: code,long_name_tr  (additional columns ignored).

Re-runnable. If a code appears in multiple chunk files, the lexicographically
first filename's translation wins; a warning is logged on stderr.
"""
from __future__ import annotations
import csv
import glob
import os
import sys

csv.field_size_limit(sys.maxsize)

HERE = os.path.dirname(os.path.abspath(__file__))
NORM = os.path.join(HERE, "normalized.csv")
LLM_DIR = os.path.join(HERE, "raw", "llm_translations")


def main() -> int:
    if not os.path.isdir(LLM_DIR):
        print(f"no LLM translations directory at {LLM_DIR}", file=sys.stderr)
        return 0

    translations: dict[str, str] = {}
    warnings: list[str] = []
    for path in sorted(glob.glob(os.path.join(LLM_DIR, "*.csv"))):
        slug = os.path.basename(path)
        with open(path, encoding="utf-8", newline="") as f:
            for row in csv.DictReader(f):
                code = (row.get("code") or "").strip()
                tr = (row.get("long_name_tr") or "").strip()
                if not code or not tr:
                    continue
                if code in translations:
                    if translations[code] != tr:
                        warnings.append(
                            f"conflicting LLM translation for {code} "
                            f"(keeping earlier, ignoring '{tr}' from {slug})"
                        )
                    continue
                translations[code] = tr

    with open(NORM, encoding="utf-8", newline="") as f:
        reader = csv.DictReader(f)
        fieldnames = reader.fieldnames or []
        rows = list(reader)

    if "long_name_tr_source" not in fieldnames:
        print(
            "normalized.csv is missing long_name_tr_source — re-run normalize.py first",
            file=sys.stderr,
        )
        return 1

    filled = 0
    already = 0
    no_translation_available = 0
    for r in rows:
        if (r.get("long_name_tr") or "").strip():
            already += 1
            continue
        code = r["code"]
        if code in translations:
            r["long_name_tr"] = translations[code]
            r["long_name_tr_source"] = "llm"
            filled += 1
        else:
            no_translation_available += 1

    with open(NORM, "w", encoding="utf-8", newline="") as f:
        w = csv.DictWriter(f, fieldnames=fieldnames)
        w.writeheader()
        w.writerows(rows)

    print(f"LOINC table rows: {len(rows)}")
    print(f"  already populated (pre-LLM, composed): {already}")
    print(f"  newly filled from LLM: {filled}")
    print(f"  still empty: {no_translation_available}")
    print(f"LLM translation entries loaded: {len(translations)}")
    if warnings:
        print(f"{len(warnings)} warning(s):", file=sys.stderr)
        for w in warnings:
            print(f"  - {w}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main())
