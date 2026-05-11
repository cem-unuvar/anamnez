#!/usr/bin/env python3
"""Merge LLM-sourced Turkish translations into ATC normalized.csv.

Reads every CSV under raw/llm_translations/*.csv (committed agent outputs) and
fills description_tr where currently empty, setting description_tr_source='llm'.
Never overwrites existing translations (TİTCK backfills win over LLM).

Input CSV schema: atc_code,description_tr  (additional columns ignored).

Re-runnable. If a code appears in multiple slice files, the lexicographically
first filename's translation wins; a warning is logged on stderr.
"""
from __future__ import annotations
import csv
import glob
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ATC_CSV = os.path.join(HERE, "normalized.csv")
LLM_DIR = os.path.join(HERE, "raw", "llm_translations")


def main() -> int:
    if not os.path.isdir(LLM_DIR):
        print(f"no LLM translations directory at {LLM_DIR}", file=sys.stderr)
        return 0

    # Load all LLM translations. First-file-wins on conflicts.
    translations: dict[str, str] = {}
    warnings: list[str] = []
    for path in sorted(glob.glob(os.path.join(LLM_DIR, "*.csv"))):
        slug = os.path.basename(path)
        with open(path, encoding="utf-8", newline="") as f:
            for row in csv.DictReader(f):
                code = (row.get("atc_code") or "").strip()
                tr = (row.get("description_tr") or "").strip()
                if not code or not tr:
                    continue
                if code in translations:
                    if translations[code] != tr:
                        warnings.append(
                            f"conflicting LLM translation for {code} "
                            f"(keeping earlier '{translations[code]}' over '{tr}' from {slug})"
                        )
                    continue
                translations[code] = tr

    # Load ATC table.
    with open(ATC_CSV, encoding="utf-8", newline="") as f:
        reader = csv.DictReader(f)
        fieldnames = reader.fieldnames or []
        rows = list(reader)

    if "description_tr_source" not in fieldnames:
        print(
            "normalized.csv is missing description_tr_source — re-run normalize.py first",
            file=sys.stderr,
        )
        return 1

    filled = 0
    already = 0
    no_translation = 0
    for r in rows:
        if (r.get("description_tr") or "").strip():
            already += 1
            continue
        code = r["atc_code"]
        if code in translations:
            r["description_tr"] = translations[code]
            r["description_tr_source"] = "llm"
            filled += 1
        else:
            no_translation += 1

    with open(ATC_CSV, "w", encoding="utf-8", newline="") as f:
        w = csv.DictWriter(f, fieldnames=fieldnames)
        w.writeheader()
        w.writerows(rows)

    print(f"ATC table rows: {len(rows)}")
    print(f"  already populated (pre-LLM): {already}")
    print(f"  newly filled from LLM: {filled}")
    print(f"  still empty: {no_translation}")
    print(f"LLM translation entries loaded: {len(translations)}")
    if warnings:
        print(f"{len(warnings)} warning(s):", file=sys.stderr)
        for w in warnings:
            print(f"  - {w}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main())
