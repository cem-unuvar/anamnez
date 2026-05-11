#!/usr/bin/env python3
"""
Build normalized ANAMNEZ-SYM CSV from per-slice agent outputs.

Inputs (all under raw/):
  - slices/<body_region>.csv  — per-region curation output, columns:
      display_tr, display_en, icd10_suggestion, notes
    body_region is taken from the filename stem.
  - codebook.csv             — append-only canonical_key -> code map, columns:
      canonical_key, display_tr, display_en, icd10_suggestion, body_region,
      code, retired_at
    Source of truth for stable ANAMNEZ-SYM-NNNN code assignments. Hand-edit
    retired_at to retire an entry; never reassign or delete a code.

Output:
  - normalized.csv  — schema:
      code, display_tr, display_en, icd10_suggestion, body_region, retired_at

Deterministic. Re-running with unchanged inputs produces byte-identical
output. New entries in slices append to codebook.csv with the next sequential
code in deterministic order (sorted by canonical_key). If a codebook entry
has no matching slice row, it is emitted using the codebook's last-known
metadata; a warning is logged unless retired_at is set.

If ../icd10-tm/normalized.csv is present, missing display_en values are
backfilled from the ICD-10-TM English descriptions on icd10_suggestion match.
"""
import csv
import glob
import os
import sys
import unicodedata

ROOT = os.path.dirname(os.path.abspath(__file__))
SLICES_DIR = os.path.join(ROOT, "raw", "slices")
CODEBOOK = os.path.join(ROOT, "raw", "codebook.csv")
OUT = os.path.join(ROOT, "normalized.csv")
ICD10TM = os.path.normpath(os.path.join(ROOT, "..", "icd10-tm", "normalized.csv"))

CODEBOOK_FIELDS = [
    "canonical_key",
    "display_tr",
    "display_en",
    "icd10_suggestion",
    "body_region",
    "code",
    "retired_at",
]
OUT_FIELDS = [
    "code",
    "display_tr",
    "display_en",
    "icd10_suggestion",
    "body_region",
    "retired_at",
]


def canon(s: str) -> str:
    return unicodedata.normalize("NFC", s.strip()).casefold()


def load_codebook() -> dict[str, dict]:
    book: dict[str, dict] = {}
    if not os.path.exists(CODEBOOK):
        return book
    with open(CODEBOOK, encoding="utf-8", newline="") as f:
        for row in csv.DictReader(f):
            book[row["canonical_key"]] = row
    return book


def write_codebook(book: dict[str, dict]) -> None:
    rows = sorted(book.values(), key=lambda r: r["code"])
    with open(CODEBOOK, "w", encoding="utf-8", newline="") as f:
        w = csv.DictWriter(f, fieldnames=CODEBOOK_FIELDS)
        w.writeheader()
        for r in rows:
            w.writerow({k: r.get(k, "") for k in CODEBOOK_FIELDS})


def load_icd10_en() -> dict[str, str]:
    if not os.path.exists(ICD10TM):
        return {}
    out: dict[str, str] = {}
    with open(ICD10TM, encoding="utf-8", newline="") as f:
        for row in csv.DictReader(f):
            en = (row.get("description_en") or "").strip()
            if en:
                out[row["code"]] = en
    return out


def next_code(book: dict[str, dict]) -> int:
    nums = []
    for r in book.values():
        c = r.get("code", "")
        if c.startswith("ANAMNEZ-SYM-"):
            try:
                nums.append(int(c.rsplit("-", 1)[-1]))
            except ValueError:
                pass
    return (max(nums) + 1) if nums else 1


def main() -> int:
    if not os.path.isdir(SLICES_DIR):
        print(f"missing slices directory: {SLICES_DIR}", file=sys.stderr)
        return 1

    book = load_codebook()
    icd10_en = load_icd10_en()
    warnings: list[str] = []

    # Collect slice rows. First-seen wins on duplicates across regions.
    collected: dict[str, dict] = {}
    for slice_path in sorted(glob.glob(os.path.join(SLICES_DIR, "*.csv"))):
        region = os.path.splitext(os.path.basename(slice_path))[0]
        with open(slice_path, encoding="utf-8", newline="") as f:
            reader = csv.DictReader(f)
            for row in reader:
                display_tr = (row.get("display_tr") or "").strip()
                if not display_tr:
                    continue
                key = canon(display_tr)
                cand = {
                    "canonical_key": key,
                    "display_tr": display_tr,
                    "display_en": (row.get("display_en") or "").strip(),
                    "icd10_suggestion": (row.get("icd10_suggestion") or "").strip(),
                    "body_region": region,
                }
                if key in collected:
                    prev = collected[key]
                    if prev["body_region"] != region:
                        warnings.append(
                            f"duplicate display '{display_tr}' in "
                            f"{prev['body_region']} and {region}; "
                            f"keeping {prev['body_region']}"
                        )
                    if (
                        prev["icd10_suggestion"]
                        and cand["icd10_suggestion"]
                        and prev["icd10_suggestion"] != cand["icd10_suggestion"]
                    ):
                        warnings.append(
                            f"conflicting icd10_suggestion for '{display_tr}': "
                            f"{prev['icd10_suggestion']} vs {cand['icd10_suggestion']}; "
                            f"keeping {prev['icd10_suggestion']}"
                        )
                    continue
                collected[key] = cand

    # Backfill display_en from ICD-10-TM.
    for cand in collected.values():
        if not cand["display_en"] and cand["icd10_suggestion"]:
            en = icd10_en.get(cand["icd10_suggestion"])
            if en:
                cand["display_en"] = en

    # Assign codes for new keys in deterministic (sorted) order.
    counter = next_code(book)
    for key in sorted(collected.keys()):
        if key not in book:
            book[key] = {
                **collected[key],
                "code": f"ANAMNEZ-SYM-{counter:04d}",
                "retired_at": "",
            }
            counter += 1
        else:
            # Refresh metadata from slice; keep code + retired_at stable.
            existing = book[key]
            book[key] = {
                **collected[key],
                "code": existing["code"],
                "retired_at": existing.get("retired_at", ""),
            }

    # Flag codebook entries with no matching slice row.
    for key, row in book.items():
        if key not in collected and not row.get("retired_at"):
            warnings.append(
                f"codebook entry '{row.get('display_tr','')}' "
                f"({row.get('code','')}) missing from slices; not auto-retired"
            )

    write_codebook(book)

    # Emit normalized.csv from codebook (sorted by code).
    with open(OUT, "w", encoding="utf-8", newline="") as f:
        w = csv.DictWriter(f, fieldnames=OUT_FIELDS)
        w.writeheader()
        for row in sorted(book.values(), key=lambda r: r["code"]):
            w.writerow({k: row.get(k, "") for k in OUT_FIELDS})

    print(f"wrote {len(book)} rows to {OUT}")
    if warnings:
        print(f"{len(warnings)} warning(s):", file=sys.stderr)
        for w in warnings:
            print(f"  - {w}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main())
