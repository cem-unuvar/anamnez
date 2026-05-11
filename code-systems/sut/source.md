# SUT — Sağlık Uygulama Tebliği procedure codes

## Source

The SUT (Sağlık Uygulama Tebliği) is published by SGK (Sosyal Güvenlik Kurumu).
Amendments are issued in the Resmî Gazete and the official annex files (Excel)
are linked from the SGK announcement page and mirrored by saglikaktuel.com.

The most recent consolidated amendment available at fetch time is:

- **Resmî Gazete: 17.01.2026 / Sayı 33140** — "Sosyal Güvenlik Kurumu Sağlık
  Uygulama Tebliğinde Değişiklik Yapılmasına Dair Tebliğ".
  Most provisions took effect on 1 February 2026; the published EK-2/A and
  EK-2/B point lists were already in force from 1 January 2026 (per the
  preceding 10 December 2025 amendment).

SGK announcement page:
<https://www.sgk.gov.tr/duyuru/detay/17012026-Tarihli-ve-33140-Sayili-Resm-Gazetede-Yayimlanan-Sosyal-Guvenlik-Kurumu-Saglik-Uygulama-Tebliginde-Degisiklik-Yapilmasina-Dair-Teblig-2026-01-20-08-51-32>

Mirror used for direct Excel downloads (SGK serves the same xlsx via a
session-scoped download URL, the mirror serves identical files):

- EK-2/B "Hizmet Başı İşlem Puan Listesi"
  <https://www.saglikaktuel.com/d/file/ek-1-ek-2b-hizmet-basi-islem-puan-listesi.xlsx>
- EK-2/C "Tanıya Dayalı İşlem Puan Listesi"
  <https://www.saglikaktuel.com/d/file/ek-2-ek-2c-taniya-dayali-islem-puan-listesi.xlsx>

Fetch date: **2026-05-11**.

Annex coverage:

- **EK-2/B** — Hizmet Başı İşlem Puan Listesi (fee-for-service procedure list).
  Bare numeric codes (e.g. `510010`, `530001`, `802500`). 4,464 unique codes.
- **EK-2/C** — Tanıya Dayalı İşlem Puan Listesi (bundled/diagnosis-based
  procedure packages). `P`-prefixed codes (e.g. `P550970`, `P600010`).
  2,405 unique codes.

Not covered:

- **EK-2/A** — "Ayaktan Başvurularda Ödeme Listesi". This is a per-specialty
  package fee list (e.g. cardiology outpatient = X TL), not a procedure
  vocabulary; it does not contain individual SUT procedure codes and is out of
  scope for this code-system table.
- **EK-2/Ç** — Diş Tedavileri Puan Listesi (dental). Not pulled for this MVP;
  add when dental flows are scoped.
- **EK-2/D, EK-2/G** and the various EK-3 (prosthetics) / EK-4 (drugs) annexes
  belong to drug + device vocabularies, not procedures, and live in their own
  code-system folders (`titck/`, etc.).

## License / redistribution

The SUT and its annexes are official mevzuat (regulatory text) published in the
Resmî Gazete by SGK. They are public-domain regulatory artefacts under Turkish
law (Resmî Gazete'de yayımlanan mevzuat herkesin erişimine açıktır). No
restrictive licence; the annex Excel files contain no copyright notice. We keep
the original files verbatim in `raw/` for provenance.

## Code format note

Hospital information systems, MEDULA, and the source xlsx files all carry
SUT codes as plain digit strings (`530010`, not `530.010`) or with a leading
`P` for EK-2/C (`P550970`). Some printed materials display them with a dotted
group (`530.010`). We preserve the **digit-only** form as the canonical
`sut_code`, since that is the form transmitted on claims. A consumer that
needs the printed form can re-insert the dot before the last three digits.

## Output

`normalized.csv` — UTF-8, 6,869 rows (plus header).

Schema:
```
sut_code, description_tr, category, retired_at
```

Per-category counts:

| category    | count |
|-------------|-------|
| surgical    | 4,798 |
| therapeutic |   908 |
| laboratory  |   630 |
| imaging     |   356 |
| diagnostic  |   151 |
| other       |    26 |

All `retired_at` values are empty: every code in this snapshot is currently
active. (When SUT retires a code, future snapshots should diff against this
file and back-fill the retirement date.)

## Category mapping rule

`category` is derived heuristically from the *current section context* in the
upstream Excel — never inferred from the bare code number alone.

### EK-2/B (top-level numbered sections)

| EK-2/B section                                       | default category |
|------------------------------------------------------|------------------|
| 1. YATAK PUANLARI                                    | other            |
| 2. HEKİM MUAYENELERİ VE RAPORLAR                     | other            |
| 3. ACİL ... GENEL UYGULAMALAR-GİRİŞİMLER             | therapeutic      |
| 4. AMELİYATHANE ve AMELİYATHANE DIŞI İŞLEMLER        | other (defs only) |
| 5. ANESTEZİ VE REANİMASYON                           | therapeutic      |
| 6. CERRAHİ UYGULAMALAR                               | surgical         |
| 7. TIBBİ UYGULAMALAR                                 | therapeutic*     |
| 8. RADYOLOJİK GÖRÜNTÜLEME VE TEDAVİ                  | imaging*         |
| 9. LABORATUVAR İŞLEMLERİ                             | laboratory       |
| 10. REFİK SAYDAM HIFZISSIHHA REFERANS LABORATUVARI   | laboratory       |

\* Overrides apply inside sections 7 and 8 to split therapy vs. diagnostic vs.
imaging when the chapter mixes both.

### EK-2/B overrides

Inside section 7 (Tıbbi Uygulamalar):

- Sub-sections that are clearly therapeutic procedures keep `therapeutic`:
  Kalp pili / ICD takılması, Tedavi amaçlı kalp kateterizasyonu, Aferez,
  Kemik iliği nakli, Organ transplantasyonu, Diyaliz, Kan bankası / Kan
  bileşenleri, Fizik tedavi uygulamaları, Hidroterapi / balneoterapi,
  Rehabilitasyon uygulamaları, Hiperbarik, Kemoterapi.
- Sub-sections that are clearly diagnostic become `diagnostic`:
  Elektrokardiyografi (EKG), Ekokardiyografi, Tanısal kalp kateterizasyonu,
  Elektrofizyolojik çalışma, Uyku araştırmaları, EEG, EMG, Uyarılmış
  potansiyeller, Değerlendirmeler, Doğum öncesi tetkikler, İnfertilite
  tetkikleri, Psikiyatrik çalışmalar.
  - Exception: any row whose name contains "ablasyon" inside an EFÇ block is
    re-tagged `therapeutic`.
- Generic-name fallback for stray rows in section 7: rows whose own name
  contains "tetkik", "inceleme", "ölçüm", "testi", or "biyopsi" become
  `diagnostic`.

Inside section 8 (Radyolojik Görüntüleme ve Tedavi):

- Sub-sections that are treatments, not pictures, become `therapeutic`:
  Radyasyon onkolojisi, Radyoterapi, Brakiterapi, Stereotaktik radyocerrahi,
  Radyonüklid tedavi, Hipertermi, Girişimsel radyolojik tedavi, Vasküler
  girişimsel, Nonvasküler girişimsel.
- Code-name fallback: rows whose own name contains "tedavi", "ablasyon",
  "embolizasyon", "stent", "dilatasyon", "drenaj", "plasti", "tromboliz",
  "trombektomi", "anjioplasti", "redüksiyon" become `therapeutic` even if the
  sub-header is generic. Catches interventional radiology procedures filed
  under e.g. "Anjiyografik tetkikler".
- Everything else in section 8 stays `imaging`.

### EK-2/C (P-prefixed bundles)

EK-2/C is overwhelmingly the bundled surgical-package list; nearly every row
is a packaged operation. Default: `surgical`. Two small early sections are
exceptions:

- ALGOLOJİ-AĞRI TEDAVİSİ UYGULAMALARI → `therapeutic`
- YOĞUN BAKIM HİZMETLERİ (erişkin + yenidoğan) → `therapeutic`

### Implementation notes on Turkish casing

Section/sub-section keyword matching is done with a Turkish-aware fold
(`unicodedata.normalize("NFC", s).casefold().replace("̇", "")`). Plain
`str.casefold()` expands the dotted capital İ into `i` + COMBINING DOT ABOVE
(U+0307), which would silently miss keywords typed with a plain `i`; the
combining-dot strip makes the comparison symmetric.

## Reproducing

```
cd code-systems/sut/raw
python3 extract.py
# writes ../normalized.csv
```

Inputs in `raw/`:

- `ek-2b-hizmet-basi-islem-puan-listesi.xlsx` (EK-2/B, 17.01.2026 amendment)
- `ek-2c-taniya-dayali-islem-puan-listesi.xlsx` (EK-2/C, 17.01.2026 amendment)
- `extract.py` (this parser)

## Blockers / limitations

- None on the data side — both EK-2/B and EK-2/C parsed cleanly without OCR.
- The category column is a coarse, source-section-driven heuristic, not an
  official SUT taxonomy. A code that is logically diagnostic but lives under
  a surgical sub-header in EK-2/C will be tagged `surgical` here; this is
  acceptable for the appliance's UI faceting and search-result filtering
  use case, but downstream code that needs precise procedure semantics
  should re-derive its own classification.
- We did not pull EK-2/A (ayaktan paket listesi), EK-2/Ç (diş), EK-2/D,
  EK-2/G or any EK-3/EK-4 annex — those are out of scope for "procedure
  codes" in the README schema; see the "Not covered" list above.
