# ANAMNEZ-SYM — Source Notes

## Summary

| Field | Value |
| --- | --- |
| Code system | ANAMNEZ-SYM — custom curated symptom and clinical-findings vocabulary for Turkish primary-care anamnez |
| Fetch date | n/a (curated, not mined) |
| Scope | Symptoms and clinical findings used as `observation.code` (or `encounter.reason_code`) when the doctor's input is symptom-driven and not better expressed as an ICD-10-TM diagnosis or LOINC measurement |
| Authority | anamnez project; revised as we learn |

## What this code system is

ANAMNEZ-SYM is the custom curated list referenced in README's Code-systems table for "symptoms and clinical findings." It exists because:

- ICD-10-TM Chapter R covers the clinical concept of "symptoms and signs" but is named for billing/epidemiology, not for what a Turkish GP actually types. `R51 — Baş ağrısı` is fine; `R10.4 — Diğer ve tanımlanmamış abdominal ağrılar` is not the phrase a doctor reaches for.
- Many common Turkish primary-care presentations are not in Chapter R at all — joint pain lives in Chapter M, dysmenorrhea in Chapter N, vision changes in Chapter H, etc. Coding them under "Chapter R only" would force doctors to skip structured coding for half of what they see.
- SNOMED CT is out of scope for the MVP (see README — Turkey is not a SNOMED International member and Turkish clinical workflows do not use it).

The vocabulary is therefore curated in-house with ICD-10-TM Chapter R as the **seed** and Turkish primary-care vernacular as the **extension**.

## Curation method

The curation is performed by per-region LLM agent runs, one slice per `body_region`. Each agent receives the full Chapter R seed (`raw/seed_icd10_r.csv`, 253 rows covering R00–R69) plus its body-region label, and produces a per-region CSV at `raw/slices/<body_region>.csv` with columns `display_tr, display_en, icd10_suggestion, notes`.

The regions, matching the `body_region` column on `symptom_anamnez`:

| Region | Scope |
| --- | --- |
| `constitutional` | fever, fatigue, weight loss, night sweats, malaise — R50–R53, R63–R64 |
| `head_neck` | headache, neck pain, facial pain — R51 + M-codes for cervical |
| `eye` | visual disturbance, eye pain, redness — H-codes + R-codes for vision |
| `ent` | earache, hearing loss, sore throat, hoarseness — H-codes, R07.0, R49 |
| `respiratory` | cough, dyspnea, hemoptysis, wheeze — R05, R06, R09 |
| `cardiovascular` | chest pain, palpitations, syncope — R00, R07.1–.4, R55 |
| `gastrointestinal` | abdominal pain, nausea, vomiting, diarrhea, constipation — R10–R19 |
| `genitourinary` | dysuria, hematuria, frequency, retention — R30–R39 |
| `gynecologic` | menstrual irregularities, vaginal discharge, pelvic pain — N-codes + R10.2 |
| `musculoskeletal` | joint pain, back pain, limb pain — M-codes + R29 |
| `skin` | rash, pruritus, lesion — R20–R23, L-codes |
| `neurological` | dizziness, seizure, weakness, paresthesia — R25–R29, R40–R42, R55–R56 |
| `psychiatric` | low mood, anxiety, insomnia, somatization — R45, R40.0, F-codes that present as symptoms |
| `pediatric` | feeding difficulty, failure to thrive, irritability — only entries that are meaningfully pediatric-specific |

Deliberately excluded: ICD-10-TM R70–R94 (abnormal lab/imaging findings — LOINC and report metadata cover those) and R95–R99 (mortality — not anamnez-relevant).

## Files

- `extract_seed.py` — pulls R00–R69 from `../icd10-tm/normalized.csv` into `raw/seed_icd10_r.csv`. Idempotent. Re-run after ICD-10-TM refreshes.
- `build_normalized.py` — merges `raw/slices/*.csv` into `normalized.csv`, assigning stable `ANAMNEZ-SYM-NNNN` codes via the append-only `raw/codebook.csv`. Idempotent. Backfills English descriptions from ICD-10-TM where `icd10_suggestion` matches and the slice left `display_en` empty.
- `raw/seed_icd10_r.csv` — Chapter R seed (253 rows). Reference material for curation agents; not directly consumed by the build.
- `raw/slices/<body_region>.csv` — per-region curation output, committed verbatim.
- `raw/codebook.csv` — append-only canonical-key → ANAMNEZ-SYM code map. Source of truth for stable code assignments. Hand-edit `retired_at` to retire an entry; never reassign or delete a code.
- `normalized.csv` — build output, schema per README's `symptom_anamnez` table.

## License

Curated in-house. No upstream license attaches. `icd10_suggestion` values reference ICD-10-TM codes whose Turkish descriptions are © T.C. Sağlık Bakanlığı; ANAMNEZ-SYM only stores the code identifier, not the upstream text.

## Known gaps

- This system is the youngest in the bundle and will evolve as the autocomplete feature gets real-world use. Expect quarterly revisions for the first year.
- Synonym handling is intentionally absent in the MVP build: two Turkish phrasings of the same concept (e.g., `boyun ağrısı` vs `ense ağrısı`) get separate codes pointing at the same `icd10_suggestion`. If autocomplete UX shows this fragments matching, add a `raw/synonyms.csv` and a collapse pass to `build_normalized.py`.
