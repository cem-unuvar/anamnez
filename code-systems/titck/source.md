# TİTCK İlaç Listesi — source notes

## Result
**Complete** — 29,240 rows normalized to the anamnez schema.

## Upstream sources

Two complementary public files from titck.gov.tr were combined:

1. **Ruhsatlı Beşeri Tıbbi Ürünler Listesi** (Licensed Human Medicinal Products List)
   — used as the spine because it has the richest schema coverage.
   - Index page: https://www.titck.gov.tr/dinamikmodul/85
   - File: `RuhsatlBeeriTbbirnlerListesi08.05.2026_e3963db4-c158-4353-aa3b-fcec018ee8c8.xlsx`
   - URL: https://titck.gov.tr/storage/Archive/2026/dynamicModulesAttachment/RuhsatlBeeriTbbirnlerListesi08.05.2026_e3963db4-c158-4353-aa3b-fcec018ee8c8.xlsx
   - File stamp on TİTCK: **08.05.2026** (form ref. İRD-LST-15/31.12.2019/Rev.01/17.11.2023)
   - Sheet used: `RUHSATLI ÜRÜNLER LİSTESİ` (22,861 data rows)
   - Columns: SIRA NO, BARKOD, ÜRÜN ADI, ETKİN MADDE, ATC KODU, RUHSAT SAHİBİ, RUHSAT TARİHİ, RUHSAT NUMARASI, DEĞİŞİKLİK, DEĞİŞİKLİK TARİHİ, ASKI DURUMU (0=clean, 1=Madde-23, 2=pharmacovigilance, 3=Madde-22), ASKIYA ALINMA TARİHİ.
   - Other sheets (`Geçici İzinli Alerjenler`, `Tescil Belgeli Radyofarmasötik`) were **not** ingested — different schema, very small, can be added later if needed.

2. **SKRS E-Reçete İlaç ve Diğer Farmasötik Ürünler Listesi** (e-Prescription List)
   — joined on barcode to provide `rx_only` (Reçete Türü) and `retired_at` (Pasife alındığı tarih).
   - Index page: https://www.titck.gov.tr/dinamikmodul/43
   - File: `AKLST0205.05.2026skrserecetilacvedigerfarmasotikurunler_2d82c586-fa0e-49b0-a6f3-f4c1dca5f4e1.xlsx`
   - URL: https://titck.gov.tr/storage/Archive/2026/dynamicModulesAttachment/AKLST0205.05.2026skrserecetilacvedigerfarmasotikurunler_2d82c586-fa0e-49b0-a6f3-f4c1dca5f4e1.xlsx
   - File stamp on TİTCK: **05.05.2026** (covers changes 28.04.2026–04.05.2026)
   - Sheets used: `AKTİF ÜRÜNLER LİSTESİ` (7,915 active rows), `PASİF ÜRÜNLER LİSTESİ` (10,033 retired rows).
   - Other sheets (`PASİFE ALINACAK ÜRÜNLER` — 47 rows, `LİSTEYE YENİ EKLENEN ÜRÜNLER` — 16 rows, `DEĞİŞİKLİK YAPILAN ÜRÜNLER` — 4 rows) describe weekly deltas and were intentionally skipped — their products already appear in the active/passive sheets.

## Fetch date
2026-05-11

## License / redistribution
Both files are publicly published by TİTCK (a Turkish government agency) on titck.gov.tr without any download gating or registration. No explicit license is attached to the files. They are pricing/regulatory reference data — equivalent to a government register and not subject to copyright in any meaningful sense (TR copyright law excludes official acts and regulations). Redistribution for clinical software use is the documented intent of the publication (e-prescription system integration). Safe to bundle into anamnez as reference data.

## Schema mapping

| anamnez column        | source                                                                                  |
| --------------------- | --------------------------------------------------------------------------------------- |
| `barcode`             | Ruhsatlı `BARKOD`, fallback SKRS `Barkod`                                               |
| `titck_product_code`  | Ruhsatlı `RUHSAT NUMARASI` (e.g. "55/44", "2016/546"). Empty for SKRS-only rows.        |
| `trade_name`          | Ruhsatlı `ÜRÜN ADI`, fallback SKRS `İlaç Adı`                                           |
| `manufacturer`        | Ruhsatlı `RUHSAT SAHİBİ`, fallback SKRS `Firma Adı`                                     |
| `atc_code`            | Ruhsatlı `ATC KODU`, fallback SKRS `ATC Kodu`                                           |
| `active_substance_tr` | Ruhsatlı `ETKİN MADDE`. Empty for SKRS-only rows (SKRS only has English ATC name).      |
| `rx_only`             | derived from SKRS `Reçete Türü` (any value → true, since all SKRS entries are Rx)       |
| `reimbursable`        | **always empty** — not available in either source (see divergence note below)           |
| `retired_at`          | SKRS `Pasif Ürünler Listesine Alındığı Tarih`, OR Ruhsatlı `ASKIYA ALINMA TARİHİ` if `ASKI DURUMU` ≠ 0 |

## Schema divergences

- **`reimbursable` (Geri Ödeme Durumu) — not present in either source.** The SGK reimbursement list (Sağlık Uygulama Tebliği EK-4/A) is published by the Sosyal Güvenlik Kurumu, not TİTCK, and would need a separate ingest from sgk.gov.tr. This column is left empty for all rows. **Follow-up:** add a `code-systems/sgk/` mining pass against `https://gss.sgk.gov.tr/SaglikTitck/pages/atcEsdegerSorgu.faces` or the SGK Bedeli Ödenecek İlaçlar (EK-4/A) PDF/Excel.
- **`dosage_form`, `strength_value`, `strength_unit`, `package_size_text` — not separate columns in the source.** The Ruhsatlı list packs everything into one `ÜRÜN ADI` string (e.g. `"ONADRON 0.75 MG TABLET, 100 TABLET"`). The normalizer runs a best-effort regex pass:
  - Dosage form is matched against a controlled list of ~70 Turkish pharmaceutical form keywords (tablet, film kaplı tablet, çözelti, kapsül, krem, …) and canonicalized to lowercase Turkish spelling (e.g. both `"FİLM KAPLI TABLET"` and `"FILM KAPLI TABLET"` collapse to `film kaplı tablet`). 4,235 rows (~14%) did not match any form keyword — `dosage_form` is empty for those.
  - Strength is extracted via regex matching `<number> <unit>` where unit ∈ {mg, mcg, g, ml, mg/ml, mg/g, mcg/ml, iu, iu/ml, %, mikrogram}. Comma decimal separators are normalized to dot. Where no clean match was found, `strength_value` and `strength_unit` are empty.
  - `strength_text` echoes the original strength expression as found in the trade name (preserves the source's "%5", "0,75 MG" etc.) for downstream re-parsing.
  - `package_size_text` is the trailing comma-separated chunk of the trade name when it contains a digit or a packaging keyword (TABLET, KAPSÜL, ML, FLAKON, AMPUL, ADET, …). For names without a comma, this is empty.
- **`rx_only` is `'true'` or `''` — never `'false'`.** The SKRS e-Reçete list only contains *prescription* drugs by definition; OTC products (Reçetesiz) do not appear at all. So a Ruhsatlı row with no SKRS hit could be either OTC or simply missing from the SKRS snapshot — we cannot distinguish, hence empty. 11,295 rows (~39%) have empty `rx_only`. **Follow-up:** the OTC list is published separately at https://www.titck.gov.tr/dinamikmodul/70 ("Reçetesiz Satılabilecek Ürünler"); ingest that to fill in `rx_only=false`.
- **Reçete Türü subcategory is collapsed to a single boolean.** Source distinguishes Normal (17,339), Yeşil (221), Mor (179), Turuncu (120), Kırmızı (85). All map to `rx_only=true`. If anamnez ever needs the subcategory (e.g. controlled-substance flagging), it should be re-derived from the raw SKRS file.

## Counts

| metric                                                       | value  |
| ------------------------------------------------------------ | ------ |
| Total normalized rows                                        | 29,240 |
| From Ruhsatlı (licensed products)                            | 22,778 |
| From SKRS only (in e-prescription list but not in Ruhsatlı)  |  6,462 |
| Joined SKRS hits on Ruhsatlı                                 | 11,483 |
| Rows with non-empty `retired_at`                             | 10,541 |
| Rows with non-empty `dosage_form`                            | 25,005 |
| Rows with non-empty `strength_value`                         |  ~22k  |
| Rows with `rx_only=true`                                     | 17,945 |
| Rows with `rx_only=''` (unknown / probably OTC)              | 11,295 |
| Distinct `dosage_form` values after canonicalization         |     47 |

## Data quality issues found

- **109 rows have malformed barcodes** (not 13 digits, or contain non-digit characters like lowercase L, or have spurious extra digits). These rows are still emitted with the source value verbatim — downstream code should treat them as soft-matched-only. Full list in `raw/diagnostics.txt`. Examples:
  - `86995870l3286` (lowercase L instead of digit 1) — PARAKS 40 MG TABLET
  - `86997115011801` (14 digits) — MESULID 100 MG TABLET
  - `868130827101` (12 digits) — ERAXIS 100 MG
  - `699844776060` (no `868`/`869` Turkey prefix) — Propofol Vem
- **57 duplicate barcodes within Ruhsatlı** — same GTIN appears on multiple registration entries (typically a re-registration or split SKU). The normalizer keeps only the **first** occurrence and logs the rest. Full list in `raw/diagnostics.txt`.
- 4,235 Ruhsatlı rows have a trade name with no recognizable dosage form keyword. These are mostly unusual or compound product names (radiopharmaceuticals, multi-component kits, surgical solutions) — acceptable.

## Files

- `raw/ruhsatli_urunler_2026-05-08.xlsx` — verbatim Ruhsatlı list (2.15 MB)
- `raw/skrs_erecete_2026-05-05.xlsx` — verbatim SKRS e-Reçete list (2.00 MB)
- `raw/normalize.py` — conversion script (Python 3, openpyxl)
- `raw/diagnostics.txt` — counts and per-row issue lists
- `normalized.csv` — anamnez-schema output, UTF-8, 4.8 MB, 29,240 data rows
- `merge_overlays.py` — applies the SGK reimbursable side file (and the
  empty-today OTC side file) onto `normalized.csv`. Idempotent and safe to
  re-run after either side file refreshes.

## Merge step (added 2026-05-11)

After mining and normalizing the base TİTCK lists, the SGK reimbursable
overlay (below) and the OTC overlay (further below) are applied to
`normalized.csv` by `merge_overlays.py`:

- `reimbursable` is filled definitively: `true` for the 7,898 rows whose
  barcode appears on the SGK EK-4/A list; `false` for the remaining 21,342
  rows. SGK publishes a complete list of what is reimbursed, so anything not
  on it is, by definition, not reimbursed.
- `rx_only` is left as the merge picked it up from the base SKRS ingest
  (true for 17,945 rows, empty for 11,295). The OTC overlay would flip
  matched rows to `false`, but no OTC list exists today — see the OTC
  section below for the legal reason.

Post-merge counts:

| Column     | Value       | Rows     |
|:-----------|:------------|---------:|
| reimbursable | true      | 7,898    |
| reimbursable | false     | 21,342   |
| rx_only    | true        | 17,945   |
| rx_only    | (empty)     | 11,295   |
| rx_only    | false       | 0        |

## SGK reimbursable list (added 2026-05-11)

### Source

The `reimbursable` flag in the master schema is owned by SGK, not TİTCK
(see the "Schema divergences" note above). We mined SGK's
**Bedeli Ödenecek İlaçlar Listesi (EK-4/A)** to produce a side file that the
master normalize step (or any downstream consumer) can left-join on barcode.

- SGK announcement: *17/01/2026 Tarihli ve 33140 Sayılı Resmî Gazete'de
  Yayımlanan Sosyal Güvenlik Kurumu Sağlık Uygulama Tebliğinde Değişiklik
  Yapılmasına Dair Tebliğ*. Same Resmî Gazete amendment that
  `code-systems/sut/source.md` already uses for EK-2/B and EK-2/C.
- Announcement page:
  <https://www.sgk.gov.tr/Duyuru/Detay/17012026-Tarihli-ve-33140-Sayili-Resm-Gazetede-Yayimlanan-Sosyal-Guvenlik-Kurumu-Saglik-Uygulama-Tebliginde-Degisiklik-Yapilmasina-Dair-Teblig-2026-01-20-08-51-32>
- Direct xlsx URL:
  <https://www.sgk.gov.tr/Download/DownloadFile?f=0ec1109c-a3fb-4723-867e-20567d7a67f5.xlsx&d=fa049c02-7d15-412e-8fb8-430c4f4f8694>
- Source-file effective date: **17.01.2026** (consolidated base list at the
  17/01/2026 SUT amendment).
- Fetch date: **2026-05-11**.

### License / redistribution

Identical to `code-systems/sut/source.md`: the SUT and its annexes are
Resmî Gazete'de yayımlanan mevzuat eki, kamuya açık, no copyright notice,
redistribution as reference data is the documented intent of publication.

### Source schema

Single sheet `EK-4A`, 8,432 rows. Header is on row 3:

```
Kamu No | Güncel Barkod | İlaç Adı | Eski Barkodlar | Eşdeğer İlaç Grubu |
Terapötik Referans Grubu | Listeye Giriş Tarihi | Aktiflenme Tarihi |
Pasiflenme Tarihi | Uygulanan İndirim Oranlarına Esas Durumu |
Depocuya Satış Fiyatı (4 discount bands) | Özel İskonto | Eczacı İskonto Oranı
```

`Kamu No` (e.g. `A15367`) is an **SGK-internal identifier**, not a TİTCK
ruhsat number. The source carries no `Ruhsat Numarası` column, so the side
file's `titck_product_code` column is **always empty**; the join key is
barcode only.

### Extraction rules

- Skip rows with non-empty `Pasiflenme Tarihi` (these are products retired
  from reimbursement; 654 rows in this file).
- For each remaining row, emit one CSV row per barcode found in either
  `Güncel Barkod` or `Eski Barkodlar` (`Eski Barkodlar` is a newline-,
  comma-, or semicolon-separated list of legacy GTINs for the same
  reimbursed product; emitting them lets the merge flag legacy SKUs that may
  still appear under the same `titck_product_code` in the master).
- Barcodes must be 13-digit numeric strings; anything else is dropped and
  logged. None were dropped on this run.
- Deduplicate within the side file (a few legacy barcodes appear on multiple
  product entries).

### Output

- `raw/sgk_reimbursable_2026-01-17.xlsx` — verbatim download (1.20 MB).
- `raw/extract_reimbursable.py` — parser (openpyxl).
- `sgk_reimbursable.csv` — two columns `barcode,titck_product_code`,
  UTF-8, LF, RFC-4180 quoted. **7,954 rows.**

### Counts

| metric                                            | value |
|---------------------------------------------------|-------|
| Total EK-4/A data rows                            | 8,429 |
| Skipped (Pasiflenme Tarihi set — retired)         |   654 |
| Skipped (no usable barcode — magistral placeholder) | 1   |
| Distinct barcodes emitted                         | 7,954 |
| Of which intersect TİTCK `normalized.csv`         | 7,898 |
| SGK-only barcodes (not in TİTCK master)           |    56 |

GS1 prefix distribution of emitted barcodes:

| prefix | count | issuer    |
|--------|-------|-----------|
| 869    | 6,512 | Türkiye   |
| 868    | 1,441 | Türkiye   |
| 340    |     1 | France (imported) |

No malformed barcodes in this snapshot.

### Caveats

- **Consolidated base only.** SGK issues incremental "Bedeli Ödenecek
  İlaçlar Listesinde Yapılan Düzenlemeler" duyurus (2026/1, 2026/2, … at
  least 2026/18 by Q2 2026). These deltas are **not merged**; on the order
  of a few hundred products' status will drift between 17.01.2026 and the
  fetch date. Acceptable for MVP; revisit if reimbursement-status precision
  becomes load-bearing.
- **56 SGK barcodes are not in `normalized.csv`.** Likely products that
  appear on the SGK reimbursement list but have since been retired from
  TİTCK or carry a different barcode in the master. The merge step should
  log these as unmatched rather than fail.
- **`titck_product_code` is always empty in this side file.** EK-4/A does
  not publish ruhsat numbers. The barcode column is the only join key.
- **Reimbursement status is a per-package SKU fact, not a per-molecule
  fact.** The same trade name may have some package sizes reimbursed and
  others not (e.g. AUGMENTIN: 9 of 15 SKUs in the master are reimbursed).
  Downstream code must not generalize from one SKU to a whole product line.

## OTC (Reçetesiz İlaçlar) list (added 2026-05-11)

### Result

**Empty.** TİTCK does **not** publish a downloadable list of OTC
(reçetesiz satılabilecek) products. No barcoded xlsx/csv/pdf list exists on
the public site as of the 2026-05-11 fetch. `raw/otc.csv` is therefore
written with the header row only (zero data rows). `rx_only=false` cannot be
filled from a TİTCK source at this time.

### Fetch date
2026-05-11

### Audit trail — where we looked

1. **README hint: `titck.gov.tr/dinamikmodul/70`.** Hint is wrong. Module 70
   is *Kurum Hizmetleri Fiyat Tarifesi* (TİTCK's own service-fee schedule),
   not an OTC product list. Verified by fetching the page and reading the
   `<h1>` ("KURUM HİZMETLERİ FİYAT TARİFESİ") and all linked attachments,
   which are pricing-tariff xlsx files for 2016–2026, plus unrelated
   PDFs/docs.

2. **All Dinamik Modüller (1–138).** Pulled `titck.gov.tr/dinamikmoduller`
   and enumerated every module title. Drug-list modules are: 43 (SKRS
   e-Reçete — already ingested, Rx only), 45 (Endikasyon Dışı İlaç —
   off-label), 57 (Ek İzlemeye Tabi — additional monitoring), 68 (Madde 23
   Uyg.), 76 (Ruhsat İptal — cancellations), 85 (Ruhsatlı — already
   ingested), 88 (Detaylı İlaç Fiyat — **login-gated**), 100 (Referans
   Bazlı Fiyat — reference pricing), 108 (Etkin Madde — active-substance
   register), 110 (İlaç Güvenlik İzlem Formları), 112 (Tıbbi Bitki —
   medicinal plants), 125 (Tedarik Takipli), 126 (Yurt Dışı Etkin Madde),
   132 (Özel Tıbbi Amaçlı Gıda), 137 (Seri Serbest Bırakma), 138 (Araç
   Uyarı Üçgenleri ATC). **None is an OTC list.**

3. **Site-wide search and Google `site:titck.gov.tr "reçetesiz"`.** The
   only OTC-themed TİTCK URLs are:
   - `haber/recetesiz-ilac-satisi-genelgesi-hakkinda-15-05-13-27122018173942`
     — 2013 press release reiterating that Rx drugs cannot be sold OTC
     under Law 1262/1928. No product list.
   - `duyuru/receteye-tabi-olmayan-beseri-tibbi-urunlerin-hakkinda-duyuru-27122018172632`
     — procedural announcement on how to apply for OTC registration. No
     product list; describes required documents (expert report, CTD
     modules).
   - `storage/Archive/2022/legislation/31_2b0c3338-1a2c-470b-8ffd-5e96447da6c1.pdf`
     — 2017-12-07 *Beşeri Tıbbi Ürünlerin Sınıflandırılmasına Dair Kılavuz*
     (Classification Guideline). 7 pages of classification criteria,
     **no product table.** Saved as
     `raw/otc_classification_kilavuz_2017-12-07.pdf` for evidentiary
     completeness.

4. **External press (medikalakademi.com.tr, T24, beo.org.tr).** In 2017–2018
   TİTCK reportedly prepared a list of "~241 products / 62 active
   substances / 87 products" (numbers vary by source) to move from Rx to
   OTC. We found no evidence that the per-barcode list was ever published
   publicly; the existing reportage describes only the *plan* and the
   scientific-commission process.

5. **SKRS e-Reçete xlsx (module 43).** Sheet inventory: AKTİF ÜRÜNLER,
   PASİF ÜRÜNLER, PASİFE ALINACAK, LİSTEYE YENİ EKLENEN, DEĞİŞİKLİK
   YAPILAN. No OTC tab. By construction this file lists prescription
   products only.

### Legal context (why no list exists)

Turkey's drug-sales regime under **Law 1262/1928 (İspençiyari ve Tıbbi
Müstahzarlar Kanunu)** historically prohibits OTC sale of licensed
medicinal products outright — every ruhsatlı beşeri tıbbi ürün is in
principle reçeteye tabi unless TİTCK explicitly reclassifies it under the
*Beşeri Tıbbi Ürünlerin Sınıflandırılmasına Dair Yönetmelik* (R.G.
17/02/2005, no. 25730). Reclassification has happened for very few
products. The 2017 *Sınıflandırma Kılavuzu* describes the application
procedure but not which products have actually moved across. Pharmacist
chambers (TEB, İEO) publicly opposed broad OTC expansion in 2017–2018, and
the planned ~241-product list never appeared on titck.gov.tr as a
downloadable table.

### Caveats

- **`rx_only=false` remains unfillable from public TİTCK data.** The
  master `normalized.csv` keeps 11,295 rows with empty `rx_only` (Ruhsatlı
  hits with no SKRS match). These could in principle be OTC, retired, or
  simply omitted from the SKRS snapshot; we cannot distinguish.
- **Future paths if this becomes load-bearing:**
  - File a TİTCK e-İşlemler/BİMER information request asking for the OTC
    product list (`https://e-islemler.titck.gov.tr/`).
  - Scrape per-product KÜB/KT pages (`titck.gov.tr/kubkt`) — each KÜB
    contains a *Reçete ile Satılma Şekli* field. Possible but expensive
    (tens of thousands of HTTP fetches).
  - Mirror a third-party catalogue (`ilacrehberi.com`,
    `hipokratist.com/recetesiz-satilan-ilaclar`) — non-authoritative;
    license unclear.
  - Use the planned EU OTC equivalence cross-walk (paracetamol, ibuprofen,
    domperidone, omeprazole low-dose, cetirizine, loratadine, etc.) plus
    pharmacist-curated overrides. Likely the right MVP move once the rest
    of anamnez is alive.

### Files

- `raw/otc_classification_kilavuz_2017-12-07.pdf` — saved as evidence of
  what TİTCK *does* publish on OTC (the classification guideline). 141 KB.
- `raw/extract_otc.py` — placeholder script. Scans `raw/` for any
  `otc_*.xlsx|xls|csv` file; if one is dropped in later by hand or by a
  follow-up scrape, the script will produce `raw/otc.csv` with
  `barcode,titck_product_code` rows automatically.
- `raw/otc.csv` — header-only (`barcode,titck_product_code\n`). 0 data
  rows. Present so the file layout is locked and the merge step can detect
  "no OTC marks available" without a missing-file error.

### License / redistribution

The classification guideline PDF is a TİTCK-published regulatory document,
public, no copyright notice. Bundling for reference is fine. No barcoded
product data was obtained.
