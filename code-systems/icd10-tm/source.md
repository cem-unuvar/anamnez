# ICD-10-TM — Source Notes

## Summary

| Field | Value |
| --- | --- |
| Code system | ICD-10-TM (Turkish modification) — published by T.C. Sağlık Bakanlığı as "ICD-10-TRM" in some file names |
| Fetch date | 2026-05-11 |
| Total rows in `normalized.csv` | 19,046 |
| Real (`is_billable=true`) codes | 16,090 |
| Categories / blocks / chapters (`is_billable=false`) | 2,956 |
| Rows with Turkish description | 19,046 (100 %) |
| Rows with English description (joined from WHO ICD-10) | 11,033 (57.9 %) |

## Primary source — Turkish (ICD-10-TM)

- **URL:** <https://dosyamerkez.saglik.gov.tr/Eklenti/3535/0/icd10trdxls.xls>
- **Saved as:** `raw/icd10trd.xls` (12.5 MB, Compound Document file)
- **Publisher:** T.C. Sağlık Bakanlığı — Sağlık Hizmetleri Genel Müdürlüğü,
  Sosyal Güvenlik Uygulamaları Daire Başkanlığı (SBSGM/SHGM). Page that
  links to the file:
  <https://shgmsgudb.saglik.gov.tr/TR,6220/icd-10-trm-hastalik-ve-saglik-girisimi-siniflandirma-sistemleri-gelistirilmesi.html>
  (redirected from `tig.saglik.gov.tr/TR,6220/...`).
- **Version year:** The web page itself describes "ICD-10-TRM hastalık ve
  sağlık girişimi sınıflandırma sistemleri geliştirilmesi" and lists this
  file as the authoritative tabular release. Document metadata says
  "Last Saved: 2008-04-03 by Ümit Başara"; the file was confirmed as the
  current version on the Ministry's index page as of 2014 (and still the
  one published in 2026). The contents are the SBSGM Turkish translation
  of the WHO ICD-10 tabular list plus TM-specific 5-character extensions
  (e.g. `M53.29`, `A41.50`, `B96.31`, …). This is the most recent
  publicly-available tabular ICD-10-TM artifact discovered; the SKRS
  Code-System index (<https://skrs.saglik.gov.tr/Anasayfa/SkrsCodeSystemList>)
  references an ICD10 code-system entry "last modified 2026-03-11", but
  that index lists no downloadable artifact — the codes are exposed only
  via the SKRS XML web service which requires registered hospital/HBYS
  credentials.
- **Workbook structure:** sheet `TABULAR LİSTE` (52,479 rows) is the main
  table with `CodeType` 1=chapter range, 2=block range, 3=real code.
  Other sheets (`Bolumler`, `Ek A` morphology, `Ek B Liste 1..5` for
  mortality/morbidity tabulations) were not used for this normalization.

### Secondary Turkish references downloaded for cross-checking (not used in the join)

- `raw/teletip_ICD10Codes.pdf` — 369 pp PDF, "T.C. Sağlık Bakanlığı SBSGM,
  ICD10 Code / Açıklama" listing, source PDF dated 2020-03-19, fetched from
  <https://www.teletip.saglik.gov.tr/docs/ICD10Codes.pdf>. Codes and Turkish
  descriptions match `icd10trd.xls` row-for-row in spot checks (e.g.
  `M53.29 — Spinal İnstabiliteler, Yer Tanımlanmamış`).
- `raw/teleradyoloji_ICD10_Codes_Table.pdf` — 532 pp duplicate of the same
  Ministry artifact, fetched from
  <https://teleradyoloji.saglik.gov.tr/docs/ICD10_Codes_Table.pdf>.
- `raw/meb_icd_tani_kod_listesi.pdf` — 6 pp curated subset of chronic-disease
  ICD codes published by MoH for the Ministry of Education school health
  programme (2020-09-16), <https://ogm.meb.gov.tr/meb_iys_dosyalar/2020_09/16094220_ICD_Tani_ve_Kod_Listesi.pdf>.
  Not authoritative for the full code system.

## English descriptions — WHO ICD-10 join

- **URL:** <https://raw.githubusercontent.com/fendis0709/icd-10/master/master_icd_x.json>
- **Saved as:** `raw/icd10_who_fendis.json` (1.8 MB)
- **Provenance:** community mirror of WHO ICD-10 (10,469 codes,
  3-char and 4-char only — i.e. plain WHO tabular). The upstream is the
  WHO ICD-10 Online Browser (<https://www.who.int/classifications/icd/icdonlineversions/en>).
  The `nama_icd` (English) column was used; the Indonesian column was
  ignored.
- **Join key:** `code` exact match. WHO does not publish Turkish 5-character
  TM extensions, so codes like `M53.29` and `A41.50` have empty
  `description_en` — this is expected and matches the README's "empty
  otherwise" rule.

A second WHO-ICD-10-CM CSV was downloaded
(`raw/icd10_who_en_k4m.csv`, k4m1113/ICD-10-CSV) but **not used** — it is
US ICD-10-CM (71,704 codes including 7-character extensions) and would have
introduced description drift relative to WHO base codes.

## Normalization logic

Implemented in `build_normalized.py` (kept in this directory so the CSV is
fully reproducible). Highlights:

- Strips dagger/asterisk decorations (`†`, `*`) from canonical `code`.
- Normalizes en-dashes (`–`) to ASCII `-` in range codes.
- Uppercases the alphabetic prefix to fix a couple of lowercase
  data-entry slips in the source (`w13.3` → `W13.3`, `w13.4` → `W13.4`).
- `parent_code`:
  - chapter range (e.g. `A00-B99`) → empty
  - block range (e.g. `A00-A09`) → smallest enclosing chapter range
  - 3-char code (e.g. `A00`) → smallest enclosing block range
  - 4-char code (e.g. `A00.0`) → its 3-char head
  - 5-char TM code (e.g. `M53.29`) → its 4-char head
- `is_billable` = true iff the code has no children in this dataset.

## Known gaps / caveats

- 57.9 % English coverage: by design, TM-specific 5-character extensions and
  the Turkish-only addendum codes (e.g. some `U`-block entries, the chapter
  and block headers themselves) have no WHO English description.
- One code, `U04` (Severe acute respiratory syndrome [SARS]), sits outside
  the `U00-U49` and `U50-Y98` ranges in the source spreadsheet and ends up
  as a top-level entry with no parent. Left as-is; it can be patched later
  if needed.
- Source workbook metadata is from 2008/2014; SBSGM has not, to my
  knowledge, published a newer free tabular release. The SKRS web service
  hosts a continuously-updated ICD10 code system (last modified 2026-03-11
  per <https://skrs.saglik.gov.tr/Anasayfa/SkrsCodeSystemList>) but the bulk
  export is only available to registered HBYS/MHRS providers — out of scope
  for this best-effort scrape.

## License

The Turkish source file is published openly on a public-facing T.C. Sağlık
Bakanlığı download endpoint with no stated license. WHO ICD-10 is published
by WHO under their classifications terms (free to use; redistribute with
attribution). No commercial restriction is asserted by either source on the
plain code-and-description data used here. Re-distribution within anamnez
should preserve attribution to T.C. Sağlık Bakanlığı SBSGM (Turkish text)
and WHO (English text).
