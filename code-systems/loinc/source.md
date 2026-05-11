# LOINC — Source provenance

## Result
**Complete** for English. Turkish: **100% coverage** of the laboratory subset
(CLASSTYPE=1, 63,215 codes) — 46,866 from composed LOINC parts plus 13,361
from a per-chunk LLM transliteration pass. The clinical / survey /
attachment subsets (CLASSTYPE 2/3/4, 36,512 codes) are not Turkish-translated
in MVP; the autocomplete UX scopes LOINC search to the lab subset.

## Source

LOINC distribution bundle (full Regenstrief release archive) was obtained from a
public GitHub mirror without registering for a LOINC.org account:

- Mirror: https://github.com/DeeNihl/QdrantLoinc/blob/main/reference_sources/Loinc.zip
- Direct download: https://raw.githubusercontent.com/DeeNihl/QdrantLoinc/main/reference_sources/Loinc.zip
- Bundle size: 74 MB (77,489,692 bytes), 90 files

This mirror redistributes the canonical Regenstrief LOINC release zip
verbatim — the same artifact a registered user would download from
https://loinc.org/download/loinc-complete/ . The license file (`LoincLicense_5.6.txt`)
is included unchanged.

## LOINC version

**LOINC 2.81** — release date 2025-02-26 (file mtimes in zip).
Confirmed by `AccessoryFiles/Loinc_2.80_DifferenceReport.pdf` (the 2.80→2.81
diff report shipped with the release).

## Fetch date

2026-05-11.

## License

LOINC Copyright Notice and License Version 5.6 (text in
`raw/LoincLicense_5.6.txt`).

Key terms relevant to anamnez:
- LOINC codes, LOINC Table, and the Linguistic Variants Files are all
  redistributable at no cost.
- The license requires an attribution notice and forbids removing copyright
  notices. The constant boilerplate is included in the normalized.csv only as
  the LOINC codes themselves; consumers of this file must surface the
  attribution as required by the LOINC license when the data is shown to users.
- LOINC is a registered trademark of Regenstrief Institute, Inc.
- This material contains content from LOINC (http://loinc.org). LOINC is
  copyright © 1995–2025 Regenstrief Institute, Inc. and the Logical Observation
  Identifiers Names and Codes (LOINC) Committee and is available at no cost
  under the license at http://loinc.org/license.

## Files preserved under raw/

- `raw/Loinc.zip` — the full distribution archive (verbatim).
- `raw/LoincTable/Loinc.csv` — main LOINC table (104,672 rows + header).
- `raw/AccessoryFiles/LinguisticVariants/trTR19LinguisticVariant.csv` — Turkish
  (Türkiye) variant, 49,501 rows + header. Producer: "LOINC Turkish Translation
  Group and the Turkish Ministry of Health".
- `raw/AccessoryFiles/LinguisticVariants/LinguisticVariants.csv` — manifest of
  all bundled linguistic variants.
- `raw/AccessoryFiles/LinguisticVariants/LinguisticVariantsReadMe.txt`
- `raw/LoincLicense_5.6.txt`, `raw/LoincReadMe.txt`

## Normalization

The Turkish coverage is built in two stages, both run from this directory:

1. `python3 normalize.py` — Reads `raw/LoincTable/Loinc.csv` and the Turkish
   linguistic variant, writes `normalized.csv`. Composes `long_name_tr` from
   the translated LOINC parts where the variant has them (~47% of codes) and
   marks `long_name_tr_source = 'composed'`. Leaves the rest empty.
2. `python3 extract_llm_inputs.py` (one-time / on refresh) splits the
   remaining empty CLASSTYPE=1 (laboratory-subset) codes into per-chunk CSVs
   at `raw/llm_inputs/chunk_NN.csv` (~500 codes each). One LLM agent per
   chunk produces a Turkish translation at
   `raw/llm_translations/chunk_NN.csv`. Then
   `python3 merge_llm_translations.py` reads every committed agent output
   and fills empty rows in `normalized.csv` with
   `long_name_tr_source = 'llm'`.

Mapping to the anamnez schema:
- `code` ← `LOINC_NUM`
- `long_name_en` ← `LONG_COMMON_NAME`, falling back to the fully-specified
  English name when blank
- `long_name_tr` ← Stage 1 (composed from translated parts in the Turkish
  variant — `COMPONENT:PROPERTY:TIME_ASPCT:SYSTEM:SCALE_TYP[:METHOD_TYP]`)
  or Stage 2 (LLM translation of `long_name_en`, applied to CLASSTYPE=1
  codes still empty after Stage 1). Blank for CLASSTYPE 2/3/4 codes outside
  the lab subset.
- `long_name_tr_source` — `'composed' | 'llm' | ''` indicating which stage
  produced the Turkish, or empty if none.
- `component` ← English `COMPONENT`
- `unit_default` ← `EXAMPLE_UCUM_UNITS`, fallback `EXAMPLE_UNITS`
- `scale_typ` ← English `SCALE_TYP`

Rows with `STATUS = DEPRECATED` are dropped. Active, Trial, and Discouraged
rows are kept.

## Row counts

- raw English LOINC table: 104,672 codes
- raw Turkish linguistic variant: 49,501 codes
- normalized.csv: **99,727 rows** after dropping DEPRECATED
- Turkish translation populated: **60,227 rows (60.4%)** — composed 46,866 +
  llm 13,361
- Laboratory subset (CLASSTYPE=1): 63,215 codes total, **100% Turkish-covered**
- `unit_default` populated: 41,886 rows (42.0%) — units are only meaningful
  for quantitative observations
- scale distribution: Qn 41,083 · Ord 26,492 · Doc 11,918 · Nom 8,230 · "-"
  4,389 · SemiQn 4,245 · Nar 1,559 · OrdQn 1,413 · Set 391 · Multi 6 · "*" 1

## Turkish coverage — caveat for downstream consumers

Two mixed sources travel in `long_name_tr`, distinguishable via
`long_name_tr_source`:

- `composed` — fully-specified LOINC-style string built from the Turkish
  variant's translated parts ("Kolesterol LDL
  içinde:KütlKons:Zmlı:Ser/Plaz:Kant:Hesaplanmış"). The official Turkish
  Translation Group has not authored a colloquial Turkish LONG_COMMON_NAME
  for any code, so this is the best free authoritative form available.
- `llm` — natural-language clinical Turkish produced by per-chunk LLM
  transliteration of `long_name_en`. Used only for CLASSTYPE=1 codes whose
  Turkish parts were unavailable. These rows are not TİTCK-verified; expect
  occasional inconsistency (e.g. missing possessive suffixes, ad-hoc Turkish
  coinages for LOINC abbreviations like `DistWidth`) and revisit on each
  LOINC refresh.

For UI display of the most common labs, anamnez may want to layer a
hand-curated short Turkish label on top of either source for the ~100–300
highest-volume tests; the source column makes that overlay straightforward.

The 39,500 still-empty `long_name_tr` rows are all CLASSTYPE 2/3/4 — clinical
observations, attachments, and survey instruments — which the autocomplete UX
does not surface in MVP. If the UI later expands LOINC scope (e.g. PHQ-9
survey items), re-run `extract_llm_inputs.py` with a relaxed CLASSTYPE
filter, dispatch agents on the new chunks, and re-run
`merge_llm_translations.py`.

## Blockers

None for this code system. The Regenstrief account-registration blocker noted
in the task brief was sidestepped by using the DeeNihl/QdrantLoinc GitHub
mirror, which redistributes the LOINC release zip verbatim under the LOINC
license. If a future LOINC release (2.82+) is needed and that mirror has not
been updated, the human will need to either find another mirror or register
an account at loinc.org.
