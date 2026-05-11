# SKRS Başvuru Nedeni — Source Notes

## Upstream source
- **Authority:** T.C. Sağlık Bakanlığı — Sağlık Bilgi Sistemleri Genel Müdürlüğü (SBSGM), SKRS (Sağlık Kodlama Referans Sunucusu)
- **Code system name:** BAŞVURU NEDENİ
- **System GUID:** `0267b955-63b8-49a4-8cc5-62ac9c2d5471`
- **Source URL:** https://skrs.saglik.gov.tr/Anasayfa/SkrsCodeSystemList (public HTML index; the row for this dictionary lists all 16 active entries inline)
- **API (auth-gated, not used):** `http://skrs.saglik.gov.tr/api/SkrsService/GetSkrsObject?skrsCodeSystemGuid=0267b955-63b8-49a4-8cc5-62ac9c2d5471&page=1` — requires `KullaniciAdi` / `Sifre` / `UygulamaKodu` headers tied to a SağlıkNet provider account.

## Fetch metadata
- **Fetch date:** 2026-05-11
- **SKRS dictionary "Son Değişiklik Tarihi":** 21.01.2020 (most recent edit upstream as of fetch)
- **Status upstream:** Aktif
- **Row count:** 16 (15 numbered entries + code 99 "Diğer")

## License
SKRS reference dictionaries are published by T.C. Sağlık Bakanlığı for use by Turkish healthcare information systems. No explicit open-data license is attached to the SKRS portal; the lists are publicly browseable. Treat as official government reference data — verbatim reproduction of code+Turkish-description pairs is normal and expected for any HBYS/EHR integrating with SağlıkNet.

## Code semantics
- `code`: SKRS-assigned numeric identifier (`1`–`15`, `99`). The HTML index displays each entry as `"<code>. <DESCRIPTION>"`. Code `99` is the conventional SKRS "Diğer" (Other) catch-all.
- `description_tr`: Turkish description, normalized from the upstream ALL-CAPS form to sentence case using standard Turkish orthography (e.g. `SUTUR ALINNMASI` → `Sütur alınması`; the upstream string also contains a typo "ALINNMASI" which has been corrected to "alınması"; `SERUM TAKILMASI` → `Serum takılması`; `KAN VE İDRAR TAHLILI YAPILMASI` → `Kan ve idrar tahlili yapılması`; `AĞIZ DIŞ MUAYENESI` → `Ağız diş muayenesi`).
- `retired_at`: empty for all rows (dictionary status is Aktif and no per-row retirement dates are exposed on the public index).

## English glosses
**All `description_en` values in `normalized.csv` are translator glosses produced for this project, not official SKRS translations.** SKRS does not publish English labels for this list. Glosses are aimed at being clinically recognizable to an English-reading clinician rather than literal.

Specific gloss choices worth noting:
- `Pansuman` → "Wound dressing" (rather than literal "Dressing")
- `Sütur alınması` → "Suture removal"
- `Akli meleke raporu` → "Mental capacity report" (administrative report attesting mental competence)
- `Alt bezi ve temizlik` → "Diaper and hygiene care"
- `Havalı yatak raporu` → "Air mattress report" (durable medical equipment justification report)
- `Serum takılması` → "IV serum insertion" (IV fluid administration)
- `INR` left as `INR` (international abbreviation, used identically in Turkish clinical practice)

## Raw
`raw/SkrsCodeSystemList.html` — verbatim HTML of the SKRS public code system index, saved 2026-05-11. The Başvuru Nedeni row (GUID `0267b955-63b8-49a4-8cc5-62ac9c2d5471`) embeds the full 16-entry list in its rightmost cell.

## Blockers
None. The authenticated JSON API was not needed because the public index page renders the complete list. If a future refresh requires per-row metadata (e.g. retirement dates, official English labels if ever added, version revisions), a SağlıkNet provider credential would be required to call `GetSkrsObject`.
