# ATC code source

## Upstream

- **Authoritative publisher**: WHO Collaborating Centre for Drug Statistics
  Methodology (WHOCC), Oslo. The ATC/DDD Index is browseable for free at
  <https://atcddd.fhi.no/atc_ddd_index/>. Structured electronic files (Excel /
  XML) of the full index are sold by WHOCC for EUR 200 per annual release
  (<https://atcddd.fhi.no/atc_ddd_index_and_guidelines/order/>) and are not
  freely redistributable.

## Mirror used

- **Repository**: <https://github.com/fabkury/atcd> (Fabrício Kury)
- **File fetched**: `WHO ATC-DDD 2026-04-25.csv`
  (URL: <https://raw.githubusercontent.com/fabkury/atcd/master/WHO%20ATC-DDD%202026-04-25.csv>)
- **Companion file fetched**: `WHO ATC-DDD-combinations 2026-04-25.csv`
  (kept under `raw/` for reference; not merged into `normalized.csv` since the
  schema differs — brand_name / dosage_form / ingredients / ddd_comb. Useful
  later if anamnez needs DDDs for fixed-dose combinations such as J01EE01.)
- **Snapshot date**: 2026-04-25 (latest snapshot in the repository at fetch
  time; the upstream WHOCC site is updated continuously, so any snapshot is a
  point-in-time mirror)
- **Fetch date**: 2026-05-11
- **Generator**: the repository ships an R scraper (`atcd.R`) and a Python
  port (`atcd.py`) that walk the WHOCC site recursively and write a flat CSV
  with the schema `atc_code,atc_name,ddd,uom,adm_r,note`.

## License

- Repository license: **Creative Commons Attribution-NonCommercial-ShareAlike
  4.0 International** (CC BY-NC-SA 4.0); see `raw/LICENSE`.
- Underlying ATC/DDD content is copyright WHOCC. The upstream copyright
  disclaimer (<https://atcddd.fhi.no/copyright_disclaimer/>) permits
  non-commercial reuse of the data as published on the website. anamnez is a
  local-first clinical records appliance; if/when commercial distribution is
  contemplated, a paid WHOCC license must be obtained and this file replaced
  with the official Excel/XML release.

## Normalization

- Scripts (run in this order): `normalize.py` → `backfill_tr_from_titck.py` → `merge_llm_translations.py`.
- Input columns: `atc_code,atc_name,ddd,uom,adm_r,note`.
- Output columns: `atc_code,description_en,description_tr,description_tr_source`.
- DDD / route / uom / note columns are dropped — anamnez only needs the
  hierarchy and the official English description for now. Level-5 codes that
  have multiple DDD/route variants in the source (and therefore multiple
  rows) are collapsed to one row per `atc_code`.
- `description_tr_source` values: `''` (no Turkish), `titck` (back-filled
  from TİTCK active-substance names — see Caveats), `llm` (LLM-translated
  from `description_en` using Turkish INN orthography — see Caveats).
- Output sort order: by code length, then lexically. This places L1 (1-char)
  codes first, then L2 (3-char), L3 (4-char), L4 (5-char), L5 (7-char) —
  hierarchy reads top-down.

## Row count

`normalized.csv`: **6,996** unique ATC codes (plus 1 header row).

Breakdown by hierarchy level (matches the fabkury/atcd README for the
2026-04-25 snapshot):

| Level | Code length | Count |
|------:|------------:|------:|
| L1    | 1           | 14    |
| L2    | 3           | 94    |
| L3    | 4           | 271   |
| L4    | 5           | 939   |
| L5    | 7           | 5,678 |
| total |             | 6,996 |

## Caveats and follow-ups

- **Turkish names — L5 substance coverage at 100% via a two-stage backfill
  (2026-05-11).** Stage 1: `backfill_tr_from_titck.py` joins the TİTCK
  normalized list at `code-systems/titck/normalized.csv` — for each ATC code
  seen in TİTCK, the most-frequent `active_substance_tr` (NFC + lowercased,
  lexicographic tiebreaker) is written to `description_tr` with
  `description_tr_source = 'titck'`. Stage 2: `merge_llm_translations.py`
  fills any remaining empty rows from the per-chapter LLM translations in
  `raw/llm_translations/<chapter>.csv` (committed agent outputs covering all
  L5 substances not in TİTCK), marking them `description_tr_source = 'llm'`.
  A small `_stragglers.csv` overlay covers the 10 rows the agents skipped.

  **5,794 of 6,996 codes (82.8%) now have Turkish names**; coverage by level:

  | Level | Code length | TR coverage         | Sources |
  |------:|------------:|---------------------|---------|
  | L1    | 1           | 0/14 (0.0%)         | — |
  | L2    | 3           | 2/94 (2.1%)         | titck (bogus — see below) |
  | L3    | 4           | 13/271 (4.8%)       | titck (mostly bogus) |
  | L4    | 5           | 101/939 (10.8%)     | titck (mostly bogus) |
  | L5    | 7           | 5,678/5,678 (100%)  | titck 1,750 / llm 3,928 |

  **L1–L4 hierarchy labels are intentionally not Turkish-translated.** They
  are category descriptors (anatomical group, therapeutic subgroup,
  pharmacological subgroup, chemical subgroup) that the autocomplete UX does
  not need translated — clinicians prescribe by L5 substance, not by
  hierarchy label. The handful of L2–L4 rows that *are* populated were filled
  by `backfill_tr_from_titck.py` from TİTCK substance lists and are
  semantically wrong as hierarchy translations (e.g. `R05 COUGH AND COLD
  PREPARATIONS` got `description_tr = "kodein, efedrin"`, the most-frequent
  substance in that therapeutic group, not the group name). They are
  preserved as-is because the doctor-facing surface is L5; if a future UI
  change starts displaying L1–L4 Turkish labels, those rows must be cleared
  and re-translated.

- **LLM-translated L5 names follow Turkish INN orthography but are not
  TİTCK-verified.** Conventions applied: `ph→f`, `c` before front vowel `→s`,
  `c` before back vowel `→k`, `y→i`, `x→ks`, `ll→l`, `acid→asit`, lowercase,
  NFC. Where a substance has a TİTCK form, that takes precedence (Stage 1
  wins over Stage 2). Stage 2 covers substances not marketed in Turkey —
  these names may differ from any future TİTCK form when the upstream WHOCC
  releases or TİTCK registers them; revisit `description_tr_source = 'llm'`
  rows on each TİTCK refresh.

- **Salt-form divergence.** TİTCK records the actual salt present in the
  product, not the INN base — so we get `description_en = "metformin"` and
  `description_tr = "metformin hidroklorür"`, `description_en = "bisoprolol"`
  and `description_tr = "bisoprolol fumarat"`, etc. For a clinician this is
  arguably more useful; for strict ATC-base lookups it's a divergence. If
  base-only Turkish names are ever needed, post-process to strip common salt
  suffixes (`hidroklorür`, `sülfat`, `fumarat`, `maleat`, `tartarat`, …).

- **39 TİTCK ATCs not in the WHO snapshot.** TİTCK references ATC codes that
  don't exist in the 2026-04-25 WHO snapshot. Some are obvious typos
  (`DO3AX` — letter O for digit 0 in `D03AX`; `N06AX2I` — capital I for digit
  1 in `N06AX21`); others are newer codes WHOCC hasn't yet added. Not merged
  into the ATC table. Tracked in TİTCK's `raw/diagnostics.txt` separately.
- **All-caps L1/L2 descriptions.** The upstream site stores level-1 and
  level-2 names in uppercase ("ALIMENTARY TRACT AND METABOLISM"); level-3+
  use sentence case. We preserve the upstream casing verbatim. If the UI
  needs consistent casing, fix it at render time, not in the data file.
- **Combination products.** Some level-5 codes for fixed-dose combinations
  (e.g. J01EE01, J04AM02) have no DDD on the main index — their DDDs live in
  the separate combinations file (preserved under `raw/` but not merged).
  This does not affect descriptions; both files have the same `atc_name` for
  the same code.
- **Snapshot drift.** WHOCC adds/retires codes each year (next major
  publication is typically January). Re-fetch annually or pin a release.

## No blockers

The upstream Excel/XML is paywalled, but a freely-redistributable mirror
(fabkury/atcd, CC BY-NC-SA) with the complete, current hierarchy was
available and is what we used. No scraping was performed by anamnez.
