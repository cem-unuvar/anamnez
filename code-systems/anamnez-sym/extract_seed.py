#!/usr/bin/env python3
"""
Extract the ICD-10-TM Chapter R seed for ANAMNEZ-SYM curation.

Reads ../icd10-tm/normalized.csv, keeps rows whose code falls in R00-R69
(the symptom and signs range relevant to anamnez), and writes them to
raw/seed_icd10_r.csv. R70-R94 (abnormal lab/imaging findings — covered by
LOINC and report metadata) and R95-R99 (mortality) are excluded.

The seed is reference material for per-slice curation agents; it is not
itself an input to build_normalized.py.
"""
import csv
import os
import re

ROOT = os.path.dirname(os.path.abspath(__file__))
SRC = os.path.normpath(os.path.join(ROOT, "..", "icd10-tm", "normalized.csv"))
OUT = os.path.join(ROOT, "raw", "seed_icd10_r.csv")

# Match the leading numeric part of an R-code or block:
#   R00, R09.88, R10-R19, R00-R99
LEAD_NUM = re.compile(r"^R(\d{1,2})")


def first_num(code: str) -> int | None:
    m = LEAD_NUM.match(code)
    return int(m.group(1)) if m else None


with open(SRC, encoding="utf-8", newline="") as f_in, open(
    OUT, "w", encoding="utf-8", newline=""
) as f_out:
    reader = csv.DictReader(f_in)
    writer = csv.DictWriter(
        f_out,
        fieldnames=[
            "icd10_code",
            "description_tr",
            "description_en",
            "parent_code",
            "is_billable",
        ],
    )
    writer.writeheader()
    kept = 0
    for row in reader:
        code = row["code"]
        n = first_num(code)
        if n is None or n >= 70:
            continue
        writer.writerow(
            {
                "icd10_code": code,
                "description_tr": row["description_tr"],
                "description_en": row["description_en"],
                "parent_code": row["parent_code"],
                "is_billable": row["is_billable"],
            }
        )
        kept += 1

print(f"wrote {kept} rows to {OUT}")
