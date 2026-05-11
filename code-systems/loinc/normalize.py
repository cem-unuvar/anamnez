#!/usr/bin/env python3
"""
Normalize the official LOINC distribution into the anamnez schema:
  code, long_name_en, long_name_tr, long_name_tr_source, component,
  unit_default, scale_typ

Inputs (from raw/):
  - LoincTable/Loinc.csv (main LOINC table, English)
  - AccessoryFiles/LinguisticVariants/trTR19LinguisticVariant.csv (Turkish)

Output:
  - normalized.csv (UTF-8)

Turkish notes:
  The Turkish linguistic variant file leaves LONG_COMMON_NAME blank for every
  row; it instead translates each LOINC part (COMPONENT, PROPERTY, TIME_ASPCT,
  SYSTEM, SCALE_TYP, METHOD_TYP). We construct a Turkish long-name by joining
  the translated parts with ':' in canonical LOINC part-name order. Rows with
  no Turkish translation are left blank in long_name_tr.

  long_name_tr_source values:
    - 'composed' — built from translated LOINC parts here (Stage 1).
    - 'llm'      — written by `merge_llm_translations.py` from per-CLASS agent
                   outputs in raw/llm_translations/ (Stage 2, currently only
                   for CLASSTYPE=1 lab codes).
    - ''         — long_name_tr is empty (not translated).
"""
import csv
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
RAW = os.path.join(HERE, 'raw')
LOINC_EN = os.path.join(RAW, 'LoincTable', 'Loinc.csv')
LOINC_TR = os.path.join(RAW, 'AccessoryFiles', 'LinguisticVariants', 'trTR19LinguisticVariant.csv')
OUT = os.path.join(HERE, 'normalized.csv')


def build_tr_long_name(row):
    """Compose a Turkish long-name from translated LOINC parts.

    Canonical LOINC fully-specified name order:
      COMPONENT:PROPERTY:TIME_ASPCT:SYSTEM:SCALE_TYP[:METHOD_TYP]
    Skip trailing empty parts; preserve empties between filled parts.
    """
    parts = [
        row.get('COMPONENT', '').strip(),
        row.get('PROPERTY', '').strip(),
        row.get('TIME_ASPCT', '').strip(),
        row.get('SYSTEM', '').strip(),
        row.get('SCALE_TYP', '').strip(),
        row.get('METHOD_TYP', '').strip(),
    ]
    # Drop trailing empties only.
    while parts and not parts[-1]:
        parts.pop()
    if not parts:
        return ''
    return ':'.join(parts)


def main():
    # Load Turkish translations into dict keyed by LOINC code.
    tr_map = {}
    with open(LOINC_TR, encoding='utf-8', newline='') as f:
        r = csv.DictReader(f)
        for row in r:
            code = row['LOINC_NUM'].strip()
            if not code:
                continue
            tr_map[code] = build_tr_long_name(row)

    # Stream the English LOINC table; write normalized output.
    rows_written = 0
    rows_with_tr = 0
    with open(LOINC_EN, encoding='utf-8', newline='') as fin, \
         open(OUT, 'w', encoding='utf-8', newline='') as fout:
        reader = csv.DictReader(fin)
        writer = csv.writer(fout, quoting=csv.QUOTE_MINIMAL, lineterminator='\n')
        writer.writerow(['code', 'long_name_en', 'long_name_tr',
                         'long_name_tr_source', 'component',
                         'unit_default', 'scale_typ'])

        for row in reader:
            code = row['LOINC_NUM'].strip()
            if not code:
                continue
            # Skip DEPRECATED / DISCOURAGED to keep the active vocabulary.
            status = row.get('STATUS', '').strip().upper()
            if status in ('DEPRECATED', 'TRIAL'):
                # Trial is rare; keep it. Drop only DEPRECATED.
                if status == 'DEPRECATED':
                    continue

            long_en = row.get('LONG_COMMON_NAME', '').strip()
            if not long_en:
                # Fall back to a fully-specified name composed from English parts.
                parts = [
                    row.get('COMPONENT', '').strip(),
                    row.get('PROPERTY', '').strip(),
                    row.get('TIME_ASPCT', '').strip(),
                    row.get('SYSTEM', '').strip(),
                    row.get('SCALE_TYP', '').strip(),
                    row.get('METHOD_TYP', '').strip(),
                ]
                while parts and not parts[-1]:
                    parts.pop()
                long_en = ':'.join(parts)

            long_tr = tr_map.get(code, '')
            long_tr_source = 'composed' if long_tr else ''
            if long_tr:
                rows_with_tr += 1

            component = row.get('COMPONENT', '').strip()
            # Prefer EXAMPLE_UCUM_UNITS (machine-readable UCUM) over EXAMPLE_UNITS.
            unit_default = (row.get('EXAMPLE_UCUM_UNITS', '').strip()
                            or row.get('EXAMPLE_UNITS', '').strip())
            scale_typ = row.get('SCALE_TYP', '').strip()

            writer.writerow([code, long_en, long_tr, long_tr_source,
                             component, unit_default, scale_typ])
            rows_written += 1

    print(f'rows_written={rows_written}', file=sys.stderr)
    print(f'rows_with_tr={rows_with_tr}', file=sys.stderr)
    print(f'tr_map_size={len(tr_map)}', file=sys.stderr)


if __name__ == '__main__':
    main()
