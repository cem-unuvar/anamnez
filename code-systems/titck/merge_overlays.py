#!/usr/bin/env python3
"""Merge SGK reimbursable + (future) OTC overlays into TİTCK normalized.csv.

- Reimbursable: SGK EK-4/A publishes a definitive list. Barcodes in
  sgk_reimbursable.csv → reimbursable=true. All other rows → reimbursable=false
  (we have a complete published list; what isn't on it isn't reimbursed).
- OTC: Turkish law 1262/1928 makes every licensed drug Rx by default and TİTCK
  publishes no OTC list. otc.csv exists but is empty; the merge is a no-op
  today, drop-in ready if TİTCK ever publishes one.

Side files:
  code-systems/titck/sgk_reimbursable.csv  (columns: barcode,titck_product_code)
  code-systems/titck/raw/otc.csv           (columns: barcode,titck_product_code)
"""
from __future__ import annotations
import csv
from pathlib import Path

ROOT = Path(__file__).resolve().parent
NORM = ROOT / "normalized.csv"
SGK = ROOT / "sgk_reimbursable.csv"
OTC = ROOT / "raw" / "otc.csv"


def load_id_set(path: Path) -> tuple[set[str], set[str]]:
    if not path.exists():
        return set(), set()
    barcodes: set[str] = set()
    titck_codes: set[str] = set()
    with path.open(encoding="utf-8") as f:
        for row in csv.DictReader(f):
            b = (row.get("barcode") or "").strip()
            t = (row.get("titck_product_code") or "").strip()
            if b:
                barcodes.add(b)
            if t:
                titck_codes.add(t)
    return barcodes, titck_codes


def main() -> None:
    sgk_barcodes, sgk_codes = load_id_set(SGK)
    otc_barcodes, otc_codes = load_id_set(OTC)
    print(f"SGK reimbursable: {len(sgk_barcodes)} barcodes, {len(sgk_codes)} titck_codes")
    print(f"OTC:              {len(otc_barcodes)} barcodes, {len(otc_codes)} titck_codes")

    with NORM.open(encoding="utf-8", newline="") as f:
        reader = csv.DictReader(f)
        fieldnames = reader.fieldnames or []
        rows = list(reader)

    reimb_hits = 0
    otc_hits = 0
    for r in rows:
        barcode = (r.get("barcode") or "").strip()
        tcode = (r.get("titck_product_code") or "").strip()
        # reimbursable: definitive — true if on the SGK list, else false
        if barcode in sgk_barcodes or (tcode and tcode in sgk_codes):
            r["reimbursable"] = "true"
            reimb_hits += 1
        else:
            r["reimbursable"] = "false"
        # rx_only: only flip to false when OTC has a definitive match.
        # If row was already true (from SKRS Rx list), keep true. If empty
        # (unknown), leave empty — we can't claim Rx without evidence.
        if barcode in otc_barcodes or (tcode and tcode in otc_codes):
            r["rx_only"] = "false"
            otc_hits += 1

    with NORM.open("w", encoding="utf-8", newline="") as f:
        w = csv.DictWriter(f, fieldnames=fieldnames)
        w.writeheader()
        w.writerows(rows)

    print()
    print(f"TİTCK rows: {len(rows)}")
    print(f"  reimbursable=true: {reimb_hits}")
    print(f"  reimbursable=false: {len(rows) - reimb_hits}")
    print(f"  rx_only flipped to false from OTC list: {otc_hits}")

    # rx_only distribution post-merge
    from collections import Counter
    rx_dist = Counter(r["rx_only"] for r in rows)
    print(f"  rx_only distribution: {dict(rx_dist)}")


if __name__ == "__main__":
    main()
