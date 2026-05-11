#!/usr/bin/env python3
"""
Build normalized ICD-10-TM CSV for anamnez.

Sources:
  - raw/icd10trd.xls  (SBSGM, T.C. Sağlık Bakanlığı, Sağlık Hizmetleri Genel
    Müdürlüğü, Sağlık Hizmetleri Geri Ödeme ve Uygulama Daire Başkanlığı)
    Tabular file containing the Turkish translation/modification of ICD-10
    ("ICD-10-TRM" — file naming convention used by SBSGM). Contains 3-char,
    4-char and TM-specific 5-char (e.g. M53.29) codes plus chapter/block heads.
  - raw/icd10_who_fendis.json  (WHO ICD-10 master, en+id; we only use en).
    Joined on `code` to provide English descriptions when available.

Output schema:
  code, description_tr, description_en, parent_code, is_billable
"""
import csv
import json
import os
import re
import xlrd

ROOT = os.path.dirname(os.path.abspath(__file__))
RAW = os.path.join(ROOT, "raw")
OUT = os.path.join(ROOT, "normalized.csv")

# -- Load WHO ICD-10 English ---------------------------------------------------
who_en = {}
with open(os.path.join(RAW, "icd10_who_fendis.json"), encoding="utf-8") as f:
    for entry in json.load(f):
        code = entry["kode_icd"].strip()
        name = (entry.get("nama_icd") or "").strip()
        if code and name:
            who_en[code] = name

# -- Load ICD-10-TM tabular ----------------------------------------------------
wb = xlrd.open_workbook(
    os.path.join(RAW, "icd10trd.xls"), encoding_override="cp1254"
)
sh = wb.sheet_by_name("TABULAR LİSTE")

# Columns: 2=CodeType, 6=FieldName, 10=Code, 12=Name
# CodeType 1=chapter range (e.g. A00-B99); 2=block range (e.g. A00-A09);
#          3=actual code (3, 4, or 5 chars). Daggers (†) and asterisks (*)
# decorate certain codes; we strip them for the canonical `code`.

records = []  # list of (code_clean, code_raw, code_type, name)
for r in range(1, sh.nrows):
    ct = sh.cell_value(r, 2)
    fn = sh.cell_value(r, 6)
    code = (sh.cell_value(r, 10) or "").strip()
    name = (sh.cell_value(r, 12) or "").strip()
    if not code or not name:
        continue
    if fn:  # INC/EXC/IT/etc. annotation rows — skip
        continue
    if ct not in (1.0, 2.0, 3.0):
        continue
    # Normalize en-dash variants to ASCII dash inside range codes for codetypes 1/2.
    code_clean = code.replace("–", "-").replace("—", "-")
    # Strip dagger / asterisk decorations from the canonical code.
    code_clean = code_clean.rstrip("†*").rstrip()
    # A handful of source rows are lowercase data-entry slips (e.g. 'w13.3').
    # Canonicalize the alphabetic prefix to uppercase.
    m = re.match(r"^([A-Za-z])(.*)$", code_clean)
    if m:
        code_clean = m.group(1).upper() + m.group(2)
    records.append((code_clean, code, int(ct), name))

# Deduplicate by code, keep first occurrence.
seen = {}
order = []
for code_clean, code_raw, ct, name in records:
    if code_clean in seen:
        continue
    seen[code_clean] = (ct, name, code_raw)
    order.append(code_clean)

# -- Compute parent + is_billable ---------------------------------------------
def parent_of(code: str, ct: int, all_codes: set) -> str:
    """Return the hierarchical parent for `code` based on shape only.

    Block→chapter and 3-char→block linkage is computed separately because it
    depends on range membership, not string prefix.
    """
    if ct in (1, 2):
        return ""
    # ct == 3: real code
    if "." in code:
        head, tail = code.split(".", 1)
        if len(tail) >= 2:
            # 5-char (TM extension): parent is the 4-char.
            candidate = f"{head}.{tail[0]}"
            if candidate in all_codes:
                return candidate
        if head in all_codes:
            return head
        return ""
    return ""

all_codes = set(order)
# Pre-compute block ranges so we can attach 3-char codes to their block parent.
blocks = []  # list of (start, end, code)
range_re = re.compile(r"^([A-Z]\d{2})-([A-Z]\d{2})$")
for c in order:
    ct, name, _ = seen[c]
    if ct in (1, 2):
        m = range_re.match(c)
        if m:
            blocks.append((m.group(1), m.group(2), c, ct))

def find_range_parent(code: str, want_ct: int):
    """Return the smallest enclosing range of the given CodeType (1=chapter,
    2=block) that contains `code` (in alpha-then-numeric ASCII order, which
    matches ICD-10's contiguous letter→two-digit layout)."""
    m = re.match(r"^([A-Z])(\d{2})$", code)
    if not m:
        return ""
    best = None
    best_span = None
    for start, end, bcode, bct in blocks:
        if bct != want_ct:
            continue
        if start <= code <= end:
            span = (ord(end[0]) - ord(start[0])) * 100 + int(end[1:]) - int(start[1:])
            if best is None or span < best_span:
                best = bcode
                best_span = span
    return best or ""

# Determine children to compute is_billable (leaf = no children).
children_count = {c: 0 for c in order}
for c in order:
    ct, name, _ = seen[c]
    if ct != 3:
        continue
    p = parent_of(c, ct, all_codes)
    if p and p in children_count:
        children_count[p] += 1

# -- Write CSV -----------------------------------------------------------------
rows_written = 0
en_hits = 0
with open(OUT, "w", encoding="utf-8", newline="") as f:
    w = csv.writer(f)
    w.writerow(["code", "description_tr", "description_en", "parent_code", "is_billable"])
    for c in order:
        ct, name_tr, _ = seen[c]
        if ct == 3:
            parent = parent_of(c, ct, all_codes)
            if not parent:
                # 3-char codes link to their block.
                m = re.match(r"^[A-Z]\d{2}$", c)
                if m:
                    parent = find_range_parent(c, want_ct=2)
            is_billable = (children_count.get(c, 0) == 0)
        elif ct == 2:
            # Block heading: parent is the enclosing chapter range.
            start_code = c.split("-", 1)[0]
            parent = find_range_parent(start_code, want_ct=1)
            is_billable = False
        else:  # ct == 1 chapter head
            parent = ""
            is_billable = False
        # English description: try exact match, then 4-char form, then 3-char.
        en = who_en.get(c, "")
        if not en and "." in c:
            head = c.split(".", 1)[0]
            en = who_en.get(head, "")
        if en:
            en_hits += 1
        w.writerow([c, name_tr, en, parent, "true" if is_billable else "false"])
        rows_written += 1

print(f"rows: {rows_written}")
print(f"english coverage: {en_hits} ({100*en_hits/rows_written:.1f}%)")
