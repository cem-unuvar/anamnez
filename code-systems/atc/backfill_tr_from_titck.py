#!/usr/bin/env python3
"""Backfill description_tr in the ATC table from TİTCK active_substance_tr.

For each ATC code in /Users/Shared/code/anamnez/code-systems/titck/normalized.csv,
pick the most-frequent active_substance_tr (lexicographic tiebreaker), and
write it as description_tr in /Users/Shared/code/anamnez/code-systems/atc/normalized.csv.

Only TİTCK ATC codes that appear in the ATC table are used (no new rows added).
Codes already populated in the ATC table are not overwritten.
"""
from __future__ import annotations
import csv
import unicodedata
from collections import Counter, defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
ATC_CSV = ROOT / "atc" / "normalized.csv"
TITCK_CSV = ROOT / "titck" / "normalized.csv"


def normalize(s: str) -> str:
    return unicodedata.normalize("NFC", (s or "").strip()).lower()


def main() -> None:
    # Group TİTCK rows by atc_code → Counter[normalized substance name]
    by_atc: dict[str, Counter[str]] = defaultdict(Counter)
    with TITCK_CSV.open(encoding="utf-8") as f:
        for row in csv.DictReader(f):
            atc = (row["atc_code"] or "").strip()
            sub = normalize(row["active_substance_tr"])
            if atc and sub:
                by_atc[atc][sub] += 1

    # Pick canonical Turkish name per ATC: highest count, then lexicographic
    canonical_tr: dict[str, str] = {}
    for atc, counter in by_atc.items():
        most = max(counter.items(), key=lambda kv: (kv[1], -ord(kv[0][0]) if kv[0] else 0, kv[0]))
        # Simpler: max count, then lexicographic ascending
        most = sorted(counter.items(), key=lambda kv: (-kv[1], kv[0]))[0]
        canonical_tr[atc] = most[0]

    # Load ATC table, backfill description_tr where empty
    with ATC_CSV.open(encoding="utf-8", newline="") as f:
        reader = csv.DictReader(f)
        fieldnames = reader.fieldnames or []
        atc_rows = list(reader)

    filled = 0
    already = 0
    no_match_for_titck_codes = set(by_atc.keys())
    atc_table_codes = {r["atc_code"] for r in atc_rows}
    for r in atc_rows:
        code = r["atc_code"]
        no_match_for_titck_codes.discard(code)
        existing = (r.get("description_tr") or "").strip()
        if existing:
            already += 1
            continue
        if code in canonical_tr:
            r["description_tr"] = canonical_tr[code]
            r["description_tr_source"] = "titck"
            filled += 1

    # Write back
    with ATC_CSV.open("w", encoding="utf-8", newline="") as f:
        w = csv.DictWriter(f, fieldnames=fieldnames)
        w.writeheader()
        w.writerows(atc_rows)

    total = len(atc_rows)
    print(f"ATC table rows: {total}")
    print(f"  description_tr already populated: {already}")
    print(f"  description_tr newly filled from TİTCK: {filled}")
    print(f"  description_tr still empty: {total - already - filled}")
    print(f"TİTCK distinct ATC codes seen: {len(by_atc)}")
    print(f"  intersected with ATC table: {len(by_atc) - len(no_match_for_titck_codes)}")
    print(f"  TİTCK-only (not in ATC table): {len(no_match_for_titck_codes)}")
    if no_match_for_titck_codes:
        sample = sorted(no_match_for_titck_codes)[:8]
        print(f"  sample TİTCK-only ATCs: {sample}")

    # Coverage by hierarchy level
    from collections import Counter as C
    levels = C()
    for r in atc_rows:
        code = r["atc_code"]
        has = bool((r.get("description_tr") or "").strip())
        levels[(len(code), has)] += 1
    print()
    print("Coverage by code length (hierarchy level):")
    for length in sorted({k[0] for k in levels}):
        yes = levels.get((length, True), 0)
        no = levels.get((length, False), 0)
        total_lvl = yes + no
        pct = 100 * yes / total_lvl if total_lvl else 0
        print(f"  L(len={length}): {yes}/{total_lvl} ({pct:.1f}%) have Turkish")


if __name__ == "__main__":
    main()
