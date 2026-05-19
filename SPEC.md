This is the feature-set for the MVP of the app "anamnez".

## Purpose

The purpose of anamnez is to help doctors and medical professionals collect, organize, and analyze data about their patients.

## Tenancy

A deployment supports one or more medical professionals, each with their own account.

Authentication is email + password. Passwords are stored as Argon2id hashes. Successful login opens an `auth_session` — a server-side row bound to one `(user, device)` pair, representing the entire token chain for that login. Access tokens are 15-minute opaque bearer tokens; refresh tokens are 12-hour rotating opaque tokens whose current hash lives on the same `auth_session` row and is one-time-use on each refresh. Each session also carries an `absolute_expires_at` fixed 30 days after login — sliding refresh cannot extend a session past that horizon, and the user must log in again. Revocation (admin logout, password change, user disable, sign-out) sets `auth_session.revoked_at`; every authenticated request rechecks this column, so revocation takes effect on the next request regardless of access-token lifetime. See Wire protocol for the request-time mechanics. The first admin is created during the Mac Studio first-boot wizard (see Deployment); the admin then provisions everyone else from the admin UI.

User roles distinguish deployment administration (`admin`) from clinical work (`provider`). The same person can hold both roles. Roles do not by themselves grant access to patient data — clinical access is governed entirely by the `patient_access` table.

A `patient_access` row grants a user one of three levels on a specific patient:

- `owner` — full control over the patient record, including granting and revoking access for other users.
- `collaborator` — read and write observations and source documents.
- `read_only` — read only.

Without a `patient_access` row, a user cannot see or touch that patient. The user who creates a patient is automatically inserted at the `owner` level — `patient_access` is the single source of truth for both data-access control and primary-provider designation, and exactly one `owner` row exists per patient (enforced by a partial unique index on `(patient_id) WHERE level = 'owner'`). Sharing happens explicitly: an `owner` adds another user as `collaborator` or `read_only` from the patient's page; ownership transfer demotes the previous owner to `collaborator` and promotes the designated successor to `owner` in one transaction.

Every authentication event, every patient record access, every observation create or amend, and every change to access grants is recorded in an append-only audit log.

## Features

### Collection

To collect data; I envision 2 ways:

- Manual data entry through some CRM-like UI interface for a patient
- Attaching files + context about the file and letting AI (transcription, OCR, + LLM reasoning) extract and collect data

### Organization

Once raw data is stored (in the form of notes, transcriptions, ocr'ed text documents); we need a way of organizing this data. This data can be qualitative or quantitative. For instance; a patient can have a certain symptom like neck pain, and it could have started 4 weeks ago. A patient can also have a serum LDL concentration of 85 mg/dL. 

There is also a temporal nature to this data as they can have test later on; and they can visit months later with new symptoms, relief etc.

### Data Modelling

The core model: observations as first-class citizens
Steal from FHIR (the healthcare interoperability standard) without adopting it wholesale. The central insight FHIR gets right is that almost everything clinical is an Observation — a single recorded fact about a patient at a point in time. Symptoms, vitals, lab results, imaging findings, even subjective reports all fit this shape.

A minimal observation looks like:

observation {
  id
  patient_id
  recorded_at                   // when the row was written (audit time)
  effective_period_start        // when the fact began (NOT NULL)
  effective_period_end          // nullable; NULL means ongoing. For point-in-time observations (e.g. a lab result), equal to start.
  code                          // structured identifier from the relevant code system
  code_system                   // 'ATC' | 'TITCK' | 'ICD10TM' | 'LOINC' | 'SUT' | 'ANAMNEZ-SYM' — observation-scoped subset (SKRS-VP is encounter-only); see Storage → Code systems
  display_text                  // free text the doctor (or extractor) typed, preserved verbatim
  value_quantity       // {value: 85, unit: "mg/dL"} for numeric
  value_string         // for qualitative
  value_codeable       // for coded answers ("severe", "mild")
  status               // 'preliminary' | 'final' | 'amended' | 'entered_in_error'
  is_problem_list_item // boolean — marks active conditions/diagnoses for the problem-list view
  source_id            // FK to the document/transcript/manual entry it came from
  encounter_id         // FK to the visit/session
  extracted_by         // 'manual' | 'llm' | model version
  confidence           // for AI-extracted data
  version              // optimistic locking
}

The code field is what makes this queryable across patients. "Neck pain started 4 weeks ago" and "LDL 85 mg/dL" both become observations with different codes and different value types, but you can ask "show me all of patient X's data on a timeline" with one query.

Conditions and active problems live in the observation table too, with `code_system = 'ICD10TM'` and `is_problem_list_item = true`. The active problem list is a query for these rows where `effective_period_end IS NULL` and `status = 'final'`. Allergies and medications, by contrast, get their own first-class tables (below) — they are life-critical at-a-glance lists, not historical facts, and their shape (severity and reaction for allergies; dose, route, and frequency for medications) does not fit the observation mold.

Code systems are scoped to the Turkish clinical context: ATC for drug active substances, TİTCK product codes plus GTIN-13 barcodes for drug products on the Turkish market, ICD-10-TM for diagnoses, LOINC for labs, SUT for procedures, the SKRS Başvuru Nedeni vocabulary (`SKRS-VP`) for procedural visit purposes, and a custom curated list (`ANAMNEZ-SYM`) for symptoms and clinical findings. SNOMED CT is out of scope for MVP: Turkey is not a SNOMED International member, the Affiliate License runs ~US$650–1,300/year per clinic deployment in Turkey's income band, and Turkish clinical workflows do not use SNOMED CT in practice (ICD-10-TM and SUT are the national vocabularies). When a doctor types `boyun ağrısı`, autocomplete suggests an `ANAMNEZ-SYM` code; the original free-text stays in a `display_text` field. Structured for querying, original for fidelity.

The full table layout, source-mining plan, and bundle distribution mechanism live in Storage → Code systems.
Source documents stay separate from extracted facts
Don't throw away the raw input. Structure it like:

source_document {
  id, patient_id, type (note|pdf|audio|image),
  raw_content / blob_url, transcription, ocr_text,
  encounter_id,                       // FK, nullable — async-arriving docs may have no encounter
  uploaded_at, context_provided_by_user,
  recorded_by,                        // FK to user
  version
}

extraction {
  id, source_document_id, observation_id,
  text_span,        // character offsets into the source
  confidence, model_version, reviewed_by, reviewed_at
}

patient_analysis {
  id, patient_id,
  generated_at, generated_by,        // FK to user — the clinician who triggered the run
  model_id, prompt_version,
  report_markdown,                   // the full report, Turkish, markdown
  scope_observation_ids              // JSON array — which observation rows fed the prompt
}

The data model also includes the schemas for users, sessions, patient ownership and access, and audit:

user {
  id, email, display_name, role,    // 'admin' | 'provider'
  password_hash,                    // argon2id
  created_at, disabled_at
}

workstation {
  id,                               // the device_id carried in the client cert's CN
  label,                            // human-readable, e.g. 'Front Desk', 'Exam Room 2'
  mode,                             // enum: 'bound' | 'shared'
  bound_user_id,                    // FK to user; non-null iff mode = 'bound'
  cert_serial,                      // serial of the client cert issued at enrollment
  cert_fingerprint,                 // SHA-256 of the issued client cert, for forensics
  enrolled_at,
  enrolled_by,                      // FK to user (admin who issued the enrollment string)
  last_seen_at,
  revoked_at,                       // nullable; presence puts `id` in the in-memory mTLS deny-set
  revoked_reason                    // nullable free text
}

auth_session {
  id,                               // stable across refresh rotations — the unit of revocation
  user_id, device_id,               // FK to workstation
  refresh_token_hash,               // rotates on every refresh, one-time-use
  refresh_expires_at,               // sliding window from last refresh
  absolute_expires_at,              // fixed 30 days after login; refresh fails past this regardless of activity
  created_at, last_seen_at,
  revoked_at                        // nullable; set on logout, password change, user disable, admin revoke
}

patient {
  id,
  mrn,                              // clinic-assigned, nullable; auto-generated as an editable random identifier on creation
  given_names,                      // single string, e.g. "Maria Elena"
  family_name,
  preferred_name,                   // nullable
  date_of_birth,
  sex_assigned_at_birth,            // enum: 'female' | 'male' | 'intersex' | 'unknown'
  gender_identity,                  // nullable free text
  email,                            // nullable
  phone,                            // nullable, single number
  address,                          // nullable single text blob
  emergency_contact_name,
  emergency_contact_phone,
  emergency_contact_relationship,
  created_by,                       // FK to user
  created_at,
  updated_at,
  deceased_at,                      // nullable
  archived_at,                      // nullable — "left the practice"
  suppressed_at,                    // nullable — set on admin-approved KVKK m. 11/e erasure request; row is invisible everywhere except audit and the retention sweep
  suppression_reason,               // nullable free text — justification recorded with suppressed_at
  version
}

encounter {
  id,
  patient_id,
  provider_id,                      // FK to user, the clinician seeing the patient
  kind,                             // enum: 'in_person' | 'phone' | 'video' | 'async_document'
  reason_text,                      // free text chief complaint
  reason_code,                      // required on transition to status 'finished'
  reason_code_system,               // 'ICD10TM' | 'ANAMNEZ-SYM' | 'SKRS-VP'
  started_at,
  ended_at,                         // nullable while in progress
  status,                           // enum: 'in_progress' | 'finished' | 'cancelled'
  created_at,
  version
}

allergy {
  id,
  patient_id,
  code,                             // nullable — ATC for drug allergies (e.g. 'J01C' penicillins); null for non-drug allergens at MVP
  code_system,                      // nullable — 'ATC' for coded drug allergies; null otherwise
  display_text,                     // free text — 'peanuts', 'house dust mites', 'amoxicillin'
  severity,                         // enum: 'mild' | 'moderate' | 'severe' | 'life_threatening'
  reaction_text,                    // free text description of the reaction
  status,                           // enum: 'active' | 'inactive' | 'entered_in_error'
  onset_date,                       // nullable
  recorded_at,
  recorded_by,                      // FK to user
  source_id,                        // FK source_document, nullable
  encounter_id,                     // FK, nullable
  version
}

medication {
  id,
  patient_id,
  code, code_system, display_text,  // 'TITCK' for product, 'ATC' for class-only entries
  dose_quantity, dose_unit,
  frequency_text,                   // 'günde 2 kez', 'Q6H' — coded frequencies are overkill for MVP
  route,                            // enum: 'oral' | 'iv' | 'im' | 'topical' | 'inhaled' | 'other'
  started_at,
  ended_at,                         // nullable
  reason_text,                      // free text indication
  status,                           // enum: 'active' | 'completed' | 'stopped' | 'entered_in_error'
  prescriber_id,                    // FK to user, nullable (external prescribers OK)
  recorded_at,
  recorded_by,                      // FK to user
  source_id,
  encounter_id,
  version
}

patient_consent {
  id,
  patient_id,
  purpose,                          // enum: 'lawyer_transfer' | 'research_non_anonymized' | 'other_clinic_referral'
  granted_at,
  granted_by,                       // FK to user — the clinician who recorded the consent
  evidence_source_id,               // FK source_document, nullable — the signed form, if uploaded
  revoked_at,                       // nullable — KVKK allows withdrawal of consent at any time
  notes,                            // free-text scope of the consent (which records, which recipient, etc.)
  version
}

patient_access {
  patient_id, user_id, level    // 'owner' | 'collaborator' | 'read_only'
}

audit_log {
  id, occurred_at, actor_user_id, auth_session_id,
  action,                  // e.g. 'patient.view', 'patient.update', 'observation.create', 'observation.amend',
                           //      'allergy.create', 'allergy.amend', 'medication.create', 'medication.amend',
                           //      'source_document.create', 'consent.record', 'consent.revoke',
                           //      'encounter.start', 'encounter.finish', 'encounter.cancel',
                           //      'user.login', 'user.create', 'user.modify', 'user.disable',
                           //      'workstation.enroll', 'workstation.revoke',
                           //      'patient_access.grant', 'patient_access.revoke',
                           //      'analysis.generate', 'codesystems.update'
  target_type, target_id,  // what was acted on
  patient_id,              // denormalized for fast "who touched patient X" lookups
  metadata,                // action-specific JSON (e.g., diff for amendments, query text for searches)
  prev_hash, row_hash      // tamper-evidence chain
}

The `audit_log` table is append-only and tamper-evident. Enforcement is layered: no application code path mutates or deletes rows, a SQLite `BEFORE UPDATE/DELETE` trigger aborts attempts to do so, and each row carries `prev_hash` and `row_hash` forming a hash chain that the server verifies on startup. See Storage for the chain construction.

### Analysis

MVP provides per-patient analysis: a single LLM call, clinician-triggered on demand. The model receives the patient's demographics, active problems, allergies, medications, encounters, and full observation history as structured JSON, and returns a concise Turkish markdown report — prose where the data is narrative, tables where it is tabular (lab trends, medication lists). The report is persisted to `patient_analysis`, audited as `analysis.generate`, and rendered in the UI with a visible "decision-support, not a clinical decision" disclaimer.

The system prompt is fixed and versioned (`prompt_version` on the row); the user message is the patient JSON. No chain-of-thought, no tool use, no retrieval beyond the JSON in the prompt.

> You are assisting a Turkish medical professional reviewing a patient's record. You will receive the patient's demographics, active problems, allergies, medications, encounters, and observations as JSON. Produce one concise report, in Turkish, formatted as markdown. Summarize the clinical picture, highlight notable trends and inconsistencies, and flag anything the clinician should pay attention to. Use markdown tables for tabular data (lab trends, medication lists) where they aid clarity; otherwise use prose. Frame every finding as something for the clinician to consider — never as a diagnosis, recommendation, or directive. Do not invent data not present in the input; if the record is sparse, say so and keep the report short. Output Turkish only.

## Privacy

We will have a local first approach; where this will be deployed on their computer, or a computer we provide (probably a Mac Studio to also be able to run inference locally). However, we can also choose to run inference through OpenRouter IN TESTING ENVIRONMENTS. The mode is enforced by a `serde`-validated `Environment` enum (`Production` | `Test`) that defaults to `Production`; OpenRouter model slugs are accepted only when `Environment::Test` is active, and the UI displays a persistent red "TEST" shield whenever the daemon is in test mode.

Two additional safeguards prevent test/production cross-contamination:

- **Production-DB marker.** The DB carries an `environment = 'production' | 'test'` row written at first boot. An `Environment::Test` daemon refuses to open a DB tagged `production` (and vice versa) — startup panic. Prevents "oops, pointed staging tool at prod."
- **`[TEST]` name prefix.** When the daemon is `Environment::Test`, patient creation rejects any `given_names` / `family_name` not prefixed `[TEST]`. Makes test-vs-real obvious in any screenshot or log.

At least the OCR model and the transcription model will run locally ALWAYS. This is non-negotiable. 

The database is stored locally on the Mac Studio. See Storage for the engine, encryption, and backup design.

## Compliance (KVKK)

The full KVKK mapping — statutory roles, lawful basis (m. 6/3 health-services exception), retention policy, breach-response support, the pre-MVP feature checklist — lives in `KVKK.md`. Treat that document as canonical; this section names only the spec-level commitments that show up in features.

- **Zero cross-border data transfer in production.** OCR, transcription, and LLM inference all run on the Mac Studio. OpenRouter is reachable only under `ENV=TEST`, which removes the entire `KVKK m. 9` (yurt dışı aktarım) surface from the production architecture. See Privacy.
- **Two-factor framing without per-user MFA.** The clinic's 2018/10 obligation for special-category data is met by the combination of the workstation's enrolled device credential (something you have, bound by default to one named user — see Deployment) and the user's password (something you know). The framing depends on per-user device binding, idle session lock (see Workstation client → Session security), and step-up reauthentication for high-risk operations (see Wire protocol) all holding — not on any one of them in isolation.
- **Security policy template, shipped with the appliance.** Anamnez ships a fill-in-the-blank clinic security policy template that explicitly documents the two-factor framing above, so a Kurul inspection finds the position written down and consistent with operational reality rather than reconstructed after the fact. The admin UI fills the template (two-factor framing, idle-lock config, physical security custodian, breach response contacts) and renders to PDF for the clinic's compliance file.
- **Patient aydınlatma metni template.** Anamnez ships a fill-in-the-blank KVKK m. 10 + Aydınlatma Tebliği patient-notice template that the clinic admin completes (clinic name, address, VKN, KEP, contact channel, transcription/AI disclosures) and the daemon renders to PDF for patient intake. Acknowledgment is optionally recorded on the patient row at first encounter via `patient.notice_acknowledged_at`.
- **DPA allocation, explicit.** The anamnez–clinic supply / data-processing agreement allocates cleanly: anamnez owns logical software controls (cert pinning, device credential lifecycle, idle-lock implementation, audit logging, encryption at rest); the clinic owns physical and operational security of the Mac Studio and workstations, ensures users do not share accounts, signs personnel under confidentiality agreements before provisioning, and applies compensating controls when shared-device mode is enabled.

### KVKK-derived features

The KVKK mapping in `KVKK.md` translates into the following spec-level features that ship in MVP:

- **Patient dossier export.** Owner / collaborator can generate a PDF dossier of a patient's full record — demographics, problem list, allergies, medications, encounters timeline, observations grouped by encounter, source-document attachments inline. Requires step-up reauthentication; audited as `patient.export`. Satisfies KVKK m. 11/b and Hasta Hakları Yön. m. 42.
- **Erasure-via-suppression workflow.** Admin UI workflow for KVKK m. 11/e requests: select patient, capture justification, mark `patient.suppressed_at`. Suppressed rows are invisible everywhere except audit and the retention sweep, which hard-deletes them when the 20-year clinical horizon passes. See Storage → Retention and destruction.
- **Explicit-consent tracking.** `patient_consent` table records the narrow cases that require açık rıza beyond KVKK m. 6/3 — lawyer transfer (KSV Yön. m. 10), non-anonymized research, external referral. Admin actions in those flows require a present, non-revoked `patient_consent` row of the matching purpose.
- **Breach scope report.** `anamnez admin breach-report` CLI and an admin UI page take `(auth_session_id)` or `(user_id, time_range)` and emit the list of affected patients, observations, and actions taken, both as on-screen tables and a downloadable CSV. Supports KVKK m. 12/5 + 2019/10 (72-hour Kurul notification).
- **Periodic access review.** Admin dashboard widget lists `patient_access` rows whose user has not touched the patient (via `audit_log`) in ≥6 months. A monthly nag banner reminds admin to review; clearing it writes an `access_review.completed` audit entry.
- **Ownership transfer at user disable.** `anamnez admin disable-user` refuses to proceed while the target user is the sole owner of any patient; admin must designate a successor (or bulk-reassign to themselves) before the disable goes through. Audited as `patient.ownership_transfer`.

## Storage

### Engine

SQLite, embedded in the server process. Chosen over DuckDB (built for analytics on top of OLTP, not as OLTP), embedded Postgres (overweight for clinic scale), Sled/redb/fjall (KV stores — no relational queries against clinical data, and reference-data joins are central to autocomplete and reporting), and SurrealDB (too young to be the system of record for medical data).

Non-default settings enforced from day one:

- `journal_mode = WAL` for concurrent reads against a single writer.
- `foreign_keys = ON` (off by default in SQLite — a footgun).
- `STRICT` tables on every table, so types actually mean something.
- One writer connection plus a pool of reader connections.
- Migrations via `refinery`, versioned and forward-only. Schema-version mismatch on startup means the server refuses to boot.

### Encryption at rest

SQLCipher (community edition, BSD-style). FileVault alone is insufficient: it doesn't protect against an attacker who clones the disk while the Mac is unlocked, or pulls the SSD. SQLCipher produces a passphrase-protected DB file that is useless without the key. Key custody — how the passphrase is wrapped and unwrapped at boot — is detailed in Deployment.

### Backups

`anamnez backup --to <path>` wraps SQLite's `sqlite3_backup_*` online backup API to produce an atomic encrypted snapshot without locking the live DB. The target is an encrypted external USB drive shipped with the appliance; the clinic plugs it in and `launchd` schedules snapshots. Restore is the sibling CLI `anamnez restore --from <path>`. No third-party backup service — that would break local-first.

### Source documents

Files on disk, not BLOBs in SQLite. Stored in a content-addressed directory: `…/blobs/<sha256[:2]>/<sha256>`. The SQLite row carries the sha256, original filename, and MIME type. Files are encrypted with AES-GCM, per-file random nonce, key derived from the same root as the SQLCipher passphrase. Keeping blobs out of the DB keeps backups fast, the page cache clean, and the DB introspectable with the sqlite CLI.

### Code systems

Clinical codes — drug substances, drug products, diagnoses, labs, procedures, symptoms — live in lookup tables in the same SQLite DB as the clinical data. Every `observation`, `medication`, and `allergy` row points at one of these tables via `(code_system, code)`. Free-text-only entries are not allowed for these fields; the `display_text` column captures what the doctor typed, the coded fields capture what it maps to.

Enforcement of `(code_system, code)` validity is in `anamnez-core` at every clinical write. SQLite has no native discriminated-FK construct (an FK cannot fan out to one of N tables based on a discriminator column), so the typed Rust API is the single chokepoint: every `observation::create` / `medication::create` / `allergy::create` looks up the pair in the relevant lookup table before insert and returns a typed error on miss. No other code path writes these tables.

**These reference data sets have been mined and live under `code-systems/<system>/` at the repo root.** Each system directory holds `normalized.csv` (UTF-8, columns matching the schemas below), the verbatim upstream downloads under `raw/`, a `source.md` documenting provenance / license / row counts / known gaps, and one or more idempotent Python scripts that rebuild `normalized.csv` from `raw/`. The signed code-systems bundle (see Bundle distribution below) is built from these CSVs. The full layout, refresh procedure, and unfixable gaps are described in Pre-bundle source data below.

#### The stack

| Domain | Code system | Upstream source mined | Status (2026-05-11) |
|---|---|---|---|
| Drug active substance | **ATC** (WHO Anatomical Therapeutic Chemical) | `fabkury/atcd` GitHub mirror of WHOCC, snapshot 2026-04-25 (CC BY-NC-SA 4.0) | 6,996 rows. EN complete; TR 82.8% overall (L5 substances 100%: titck 1,750 / llm 3,928; L1–L4 hierarchy labels intentionally not translated). |
| Drug product (Turkish market) | **TİTCK product code + barcode** (GTIN-13) | `titck.gov.tr` Ruhsatlı (08.05.2026) + SKRS e-Reçete (05.05.2026), plus SGK EK-4/A (17.01.2026) for reimbursable | 29,240 rows. `reimbursable` filled definitively (7,898 true / 21,342 false); `rx_only` true for 17,945, empty for 11,295 (no OTC list exists by law). |
| Diagnoses and conditions | **ICD-10-TM** (Turkish modification) | SBSGM `icd10trd.xls` (vintage 2008/2014, still the current free release) + community WHO mirror for English | 19,046 rows. TR 100%; EN 58%. |
| Labs and measurements | **LOINC** | LOINC 2.81 (2026-02-26) via the `DeeNihl/QdrantLoinc` GitHub mirror | 99,727 rows. EN complete; TR 60.4% overall (CLASSTYPE=1 lab subset 100%: composed 46,866 / llm 13,361; CLASSTYPE 2/3/4 clinical / attachment / survey codes not Turkish-translated in MVP). |
| Procedures | **SUT** (Sağlık Uygulama Tebliği) | SUT amendment 17.01.2026 (Resmî Gazete 33140), EK-2/B + EK-2/C | 6,869 rows. Procedure annexes only — EK-3 (prosthetics), EK-4 (drugs) out of scope for this schema. |
| Procedural visit purpose | **SKRS Başvuru Nedeni** (in code: `SKRS-VP`) | `skrs.saglik.gov.tr` public portal (last upstream update 2020-01-21) | 16 rows. TR canonical; EN glosses are translations. |
| Symptoms and clinical findings | **Custom curated list** (`ANAMNEZ-SYM`) | Curated by us in-house; ICD-10-TM Chapter R as the base, extended with common Turkish primary-care presentations | 585 rows across 14 `body_region` slices. TR canonical; EN provided per row. `icd10_suggestion` filled on 578/585 (98.8%); 7 deliberate blanks where no ICD-10-TM symptom code fits. |
| International clinical findings | **SNOMED CT** | SNOMED International | **Out of scope for MVP.** Turkey is not a SNOMED International member; the Affiliate License is ~US$650–1,300/year per clinic deployment in Turkey's income band; and Turkish clinical workflows do not use SNOMED CT in practice. Revisit only if a specific customer's contract requires it for international interoperability. |

Procedures are informational clinical records, not billing artifacts. SUT is included because it is the procedure vocabulary every Turkish doctor recognizes, regardless of whether anamnez ever touches reimbursement. A procedure in the data model is an `observation` with `code_system = 'SUT'`; outcomes and findings from the procedure are separate observations linked to the same `source_id` (the operative or procedure report).

`SKRS-VP` is the small (16-item) nationally-bound vocabulary published on SKRS for procedural visit purposes — `genel muayene` (general examination), `pansuman` (dressing change), `sütur alınması` (suture removal), and similar. It is used only on `encounter.reason_code` when a visit's purpose is procedural rather than symptom-driven; symptom-driven visits use `ANAMNEZ-SYM` or `ICD-10-TM` on the same field. SKRS does not include a "follow-up" / "kontrol" entry in `Başvuru Nedeni` — follow-up visits are distinguished at the SKRS `Başvuru Türü` axis (visit type) rather than the reason axis, so anamnez does not need a kontrol code here.

ICPC-2 (International Classification of Primary Care) has an unofficial Turkish translation and a small academic audience among Turkish family physicians, but no Turkish AHBS vendor ships an ICPC-2 picker and it is not nationally bound. Including it in MVP would fragment symptom coding between `ANAMNEZ-SYM` and ICPC-2. Deferred to v2 — to be reconsidered only if a specific family-medicine customer requests it.

#### Lookup tables

```
drug_atc {
  atc_code PK,                     // 'A10BA02'
  description_en,
  description_tr                   // 'metformin'
}

drug_titck {
  barcode PK,                      // GTIN-13, what's on the box — one row per package-size barcode
  titck_product_code INDEX,        // TİTCK registration number — one registration can map to N barcoded package sizes
  trade_name,                      // 'Glucophage 500 mg Tablet'
  manufacturer,
  atc_code FK → drug_atc,
  active_substance_tr,
  dosage_form,                     // 'film tablet', 'şurup', ...
  strength_value, strength_unit,   // 500, 'mg'
  strength_text,                   // verbatim label string, e.g. '0.75 MG' — kept for display fidelity when value/unit do not round-trip the box label
  package_size_text,               // '30 tablet'
  rx_only,                         // boolean
  reimbursable,                    // SGK list membership
  retired_at                       // nullable
}

icd10_tm {
  code PK,                         // 'E11.9'
  description_tr,
  description_en,
  parent_code FK → icd10_tm,       // nullable, hierarchy
  is_billable                      // leaf-or-not
}

loinc {
  code PK,                         // '13457-7'
  long_name_en,
  long_name_tr,                    // from LOINC's official translations
  component,                       // 'LDL Cholesterol'
  unit_default,                    // 'mg/dL'
  scale_typ                        // 'Qn' | 'Ord' | 'Nom'
}

procedure_sut {
  sut_code PK,
  description_tr,
  category,                        // 'surgical' | 'diagnostic' | 'therapeutic' | 'imaging' | ...
  retired_at
}

visit_purpose_skrs {               // SKRS Başvuru Nedeni, code_system = 'SKRS-VP'
  code PK,                         // SKRS-assigned identifier
  description_tr,                  // 'Genel muayene'
  description_en,                  // 'General examination'
  retired_at
}

symptom_anamnez {
  code PK,                         // 'ANAMNEZ-SYM-0042'
  display_tr,                      // 'boyun ağrısı'
  display_en,                      // 'neck pain'
  icd10_suggestion FK → icd10_tm,  // nullable
  body_region,                     // 'head_neck' | 'thorax' | ...
  retired_at
}
```

`medication.code_system` is restricted to `'ATC'` or `'TITCK'`. Most prescriptions reference a TİTCK product (the actual box on the shelf); ATC-only is allowed for "patient is on a calcium channel blocker, unknown brand" cases.

#### Pre-bundle source data

The mined data lives at the repo root under `code-systems/<system>/`, one directory per row of the stack table above. Each directory follows the same conventions:

- `normalized.csv` — UTF-8, columns matching the lookup-table schema above, one row per code. This is the input to the bundle build.
- `raw/` — verbatim upstream files (Excel, CSV, PDF, HTML), kept so that re-normalization is reproducible and so that diffs against upstream changes are auditable.
- `source.md` — provenance: upstream URL, fetch date, source-file effective date, license, row count, schema divergences, known gaps. **Canonical for that system; this README is the index.**
- For most systems, one or more idempotent Python scripts — `normalize.py`, `extract.py`, `build_normalized.py`, `extract_llm_inputs.py`, or `merge_llm_translations.py` — derive `normalized.csv` from `raw/`. Re-runnable: drop a fresher file into `raw/`, re-run the script, commit. Some systems chain multiple scripts (ATC: `normalize.py` → `backfill_tr_from_titck.py` → `merge_llm_translations.py`; LOINC: `normalize.py` → `extract_llm_inputs.py` → per-chunk agent runs → `merge_llm_translations.py`); the order is documented in each system's `source.md`. **Exceptions: SUT and SKRS-VP** are small enough that their `normalized.csv` files were hand-curated from the upstream sources; their `source.md` files document the manual procedure, and refresh means redoing the curation by hand.

Two cross-system overlay scripts join one system's data into another and are re-run after either side refreshes:

- `code-systems/atc/backfill_tr_from_titck.py` — joins TİTCK `active_substance_tr` into ATC `description_tr` on `atc_code`, picking the most-frequent substance string per code (lexicographic tiebreaker, NFC-normalized + lowercased).
- `code-systems/titck/merge_overlays.py` — applies the SGK reimbursable list (`code-systems/titck/sgk_reimbursable.csv`) and a placeholder OTC list (`code-systems/titck/raw/otc.csv`) onto TİTCK `normalized.csv`. `reimbursable` is filled definitively (`true` on match, `false` otherwise — SGK publishes a complete list). `rx_only` is only ever flipped to `false` by a non-empty OTC overlay.

Refresh cadence per system is in Bundle distribution below — TİTCK monthly, LOINC quarterly, ICD-10-TM and SUT yearly, ATC annual. The refresh procedure for any system is the same: drop the new upstream file in `raw/`, re-run the script, re-run any cross-system overlays that read from the refreshed system, commit.

Known gaps that cannot be closed without something we don't currently have:

- **TİTCK `rx_only=empty` for 11,295 rows.** TİTCK does not publish an OTC list and cannot under Law 1262/1928, which makes every licensed drug Rx by default; the 2017–2018 reclassification effort never produced a barcoded file. The UI must not assume Rx or OTC for empty `rx_only` rows. `merge_overlays.py` is wired to apply an OTC overlay the day TİTCK ever ships one.
- **ATC commercial license.** The mirror is CC BY-NC-SA 4.0. A paid WHOCC license (~EUR 200/year per release) must be acquired before any commercial distribution, and `atc/normalized.csv` then replaced from the official WHOCC Excel/XML.
- **ATC L1–L4 hierarchy labels intentionally not Turkish-translated.** The autocomplete UX surfaces L5 substances, not hierarchy labels — translating "ALIMENTARY TRACT AND METABOLISM" or "Benzodiazepine derivatives" would not change what doctors see when prescribing. L5 substances themselves are at 100% Turkish coverage via TİTCK (30.8%) plus an LLM-translation pass (69.2%) marked `description_tr_source = 'llm'`; revisit `'llm'` rows on each TİTCK refresh in case a new TİTCK form supersedes the LLM transliteration.
- **ICD-10-TM freshness.** The SBSGM Excel is the latest publicly-free release but is vintage 2008/2014. A newer 2026-03-11 dataset is reachable only through the SKRS provider-credentialed web service; swap in if a customer grants credentials.
- **LOINC clinical / survey / attachment subset not Turkish-translated.** The CLASSTYPE=1 lab subset (63,215 codes) is 100% Turkish-covered via a composed-parts stage and an LLM pass; CLASSTYPE 2/3/4 (clinical observations, attachments, surveys — 36,512 codes) are out of scope for MVP autocomplete and left blank. If the UI later expands to surface those, re-run `extract_llm_inputs.py` with a relaxed filter and dispatch a fresh agent batch. The `'llm'`-sourced lab translations are not TİTCK-verified and should be revisited on each LOINC release in case the official Turkish Translation Group ships a `LONG_COMMON_NAME` for any of those rows.
- **ICD-10-TM symptom-code gaps surfaced during ANAMNEZ-SYM curation.** The Turkish modification does not carry the granular sub-codes the curation needed in several places: `R19.7` (diarrhea) is absent and entries route to `K59.1` instead; `R35.0` / `R35.1` and the `R39.1x` voiding-symptom splits collapse to bare `R35` and `R39.1`; `R06.8x` sleep-breathing sub-codes (apnea, snoring) collapse to `R06.8`; `R11.0` / `R11.1` (nausea / vomiting separately) collapse to bare `R11`. These force coarser `icd10_suggestion` mappings than the canonical R-axis would otherwise allow. Closes only if a future ICD-10-TM release adds them.

Bootstrap from a fresh clone: nothing to do — the CSVs are committed. To re-derive any system's `normalized.csv` from its `raw/` files: `python3 code-systems/<system>/normalize.py` (or `extract.py` / `build_normalized.py` depending on the system — see the directory listing). For ATC the full rebuild is three stages in order — `normalize.py` (from WHOCC mirror), `backfill_tr_from_titck.py` (Turkish names from TİTCK), `merge_llm_translations.py` (Turkish INN transliteration for the L5 substances TİTCK does not carry). LOINC is similarly multi-stage — `normalize.py` (from Regenstrief release + Turkish linguistic variant), then `merge_llm_translations.py` to apply the committed per-chunk agent outputs that cover the CLASSTYPE=1 lab subset; `extract_llm_inputs.py` only needs re-running when expanding the translated subset. Cross-system overlays: re-run `backfill_tr_from_titck.py` and `merge_overlays.py` after their inputs change.

#### Bundle distribution

Reference tables are not editable by clinics. They are populated at first boot from a signed bundle that we produce and ship: `anamnez-codesystems-<YYYYqN>.tar.zst.sig`. The bundle contains all reference tables plus a manifest with version, checksum, and source revision dates (TİTCK as of X, ICD-10-TM revision Y, LOINC version Z, SUT version W, `ANAMNEZ-SYM` revision N).

Bundles are signed with a single long-lived Ed25519 keypair held by anamnez. The public key is compiled into the `anamnez` binary at build time; the daemon verifies every bundle against the embedded pubkey before any DB mutation. Key rotation, if ever required (e.g. suspected key compromise), ships as a new daemon binary version — clinics update both binary and bundle together.

Two delivery paths to the Mac Studio:

1. **Online pull** — `anamnez admin update-codesystems` reaches our distribution host (the one outbound connection allowed) and applies the latest signed bundle.
2. **Offline sideload** — admin downloads the bundle on a separate machine, plugs a USB drive into the Mac Studio, runs `anamnez admin update-codesystems --from /Volumes/USB/bundle.tar.zst.sig`.

Application semantics: signature verified, transaction begins, tables are diff-applied (new rows inserted, changed rows updated, removed rows marked `retired_at`), audit-log entry written, transaction commits. Retired codes stay queryable forever — only new observations are blocked from referencing them.

Cadence target: one bundle per quarter covering all systems. TİTCK changes most often (monthly upstream), LOINC quarterly, ICD-10-TM and SUT yearly, `ANAMNEZ-SYM` as we learn.

#### Autocomplete and LLM extraction

Both the doctor's autocomplete and the LLM extractor query the same reference tables.

- Autocomplete is SQLite FTS5 over the relevant display columns (`display_tr`, `trade_name`, `active_substance_tr`, `description_tr`). Pure local, no model. Turkish casing forces a pre-fold step: writes and queries both pass through Turkish-locale casefold (`İ`→`i`, `I`→`ı`, NFC) before they touch FTS5 — `unicode61`'s default mapping would let `İlaç` and `ilaç` miss each other. Diacritic stripping stays off (`remove_diacritics=0`); `ş`/`s` and `ç`/`c` are distinct letters in Turkish.
- LLM extraction receives the source text plus the relevant rows (or category slice) of the reference tables in its prompt, and returns a JSON list of candidate observations. Each candidate carries `code_system`, `code`, `display_text` (verbatim source span), `text_span` (character offsets into the source, used to populate the `extraction` row), `effective_period_start`, `effective_period_end` (nullable), one of `value_quantity` / `value_string` / `value_codeable` as appropriate to the observation type, and `confidence`. **Every observation always carries a `(code_system, code)` pair — no exceptions.** If the extractor cannot find a match, it routes the candidate to `code_system = 'ANAMNEZ-SYM'` (the catch-all symptom system) and lets a human reviewer reclassify before the observation moves from `preliminary` to `final`. Manual entry follows the same rule: when autocomplete returns no match in the chosen system, the clinician falls back to `ANAMNEZ-SYM`; if even `ANAMNEZ-SYM` lacks the concept, the gap is in code content (extend the curated list) rather than in the UI. Observations recorded by mistake are not deleted — they transition to `status = 'entered_in_error'`, mirroring the soft-delete pattern used by allergies and medications, and are hidden from clinical surfaces while remaining on the audit trail.

### Audit log integrity

The `audit_log` table is append-only at three layers:

1. **Application** — only `audit_log::append()` exists; there is no update or delete function.
2. **SQLite** — a `BEFORE UPDATE/DELETE ON audit_log` trigger that calls `RAISE(ABORT, 'audit immutable')`.
3. **Tamper-evidence** — each row carries `prev_hash` and `row_hash = H(prev_hash, id, occurred_at, actor_user_id, auth_session_id, action, target_type, target_id, patient_id, canonical(metadata))`. The server verifies the chain head on startup and panics with the offending row id on mismatch.

Audit log retention is 10 years from `occurred_at`. The nightly retention sweep (see Retention and destruction) hard-deletes rows past that horizon and writes one `audit_log.retention_sweep` entry per pass recording the high-water mark of swept `occurred_at`. Chain verification on startup runs from the most recent `retention_sweep` row forward, so deleted history does not break verification of the surviving chain.

### Retention and destruction

Anamnez ships with a default retention policy compiled in. Clinics cannot disable retention; the nightly sweep is unconditional.

| Data | Retention | Trigger |
|---|---|---|
| Observation, source_document, extraction, allergy, medication, encounter | 20 years | Patient's last clinical activity, or `deceased_at`, whichever is later |
| `audit_log` | 10 years (fixed) | `occurred_at` |
| `auth_session` | Expiry + 90 days | `refresh_expires_at` |
| Disabled `user` account | 10 years after `disabled_at` | Hard delete at the horizon; `actor_user_id` foreign-key references remain valid up to that point |
| Backups | 1 year rolling (52 weekly + 12 monthly snapshots) | Snapshot time |

A nightly job (`anamnez retention sweep`, scheduled by launchd) hard-deletes rows past their horizon and writes one `audit_log.retention_sweep` row per pass with counts by table. Defaults come from the hekimlik 20-year clinical record obligation and KSV Yön. m. 11; see `KVKK.md §13` for the full legal rationale.

**Erasure requests (KVKK m. 11/e) use suppression, not hard delete.** When an admin approves an erasure request, the affected `patient.suppressed_at` is set with a justification. Suppressed patients are invisible everywhere except `audit_log` and the retention sweep; their clinical rows are not deleted until the 20-year horizon passes. This reconciles the patient's right of erasure with the clinician's record-keeping obligation. Anonymization is not offered in MVP — suppression is the only erasure path.

### Concurrency

Optimistic locking on every mutable clinical row. `observation`, `patient`, `encounter`, `allergy`, `medication`, `source_document`, and `patient_consent` each carry a `version INTEGER NOT NULL` column. Writes are `UPDATE … SET …, version = version + 1 WHERE id = ? AND version = ?`. Zero rows affected means a concurrent edit happened; the conflict surfaces to the client as "record changed, here is the new state, reapply your change." Last-write-wins is not used anywhere in clinical data.

## Deployment

Each deployment is a single Mac Studio on a single clinic's LAN. There will be N of these — independent appliances, not federated. No clinic sees another clinic's data, and no Mac Studio is reachable from outside its clinic's network.

Anamnez ships as two native Rust binaries:

- The **`anamnez` binary**, a multi-tool whose `serve` subcommand is the long-running daemon on the Mac Studio. Other subcommands handle first-boot setup, backups, migrations, audit verification, and admin operations.
- The **`anamnez-workstation` binary**, the Tauri client that runs on Windows or macOS workstations.

Both binaries are signed once by us and distributed to every clinic. The workstation client is the same artifact for every clinic — nothing about it is per-deployment. There is no browser involved in the normal clinical workflow; the OS trust store is never touched.

No inbound surface from outside the clinic LAN is exposed in MVP:

- The Mac Studio gets a stable LAN address (static DHCP reservation, or an mDNS name like `anamnez.local`).
- The server presents a self-signed TLS certificate generated on first boot of that specific Mac Studio.
- The workstation client pins the server's certificate by fingerprint. The fingerprint is delivered to the client at enrollment time, so trust is established without any OS-level CA installation.
- Authentication, authorization, and device identity all live at the anamnez layer.

The bootstrap flow:

1. First boot of the Mac Studio runs a setup wizard: generates a long-lived (25-year) Ed25519 keypair that serves both as the server's TLS identity and as the local CA for signing workstation client certificates, creates the first admin credential, and prints a recovery code on screen for the admin to record physically.
2. The admin uses the anamnez admin UI to "Add workstation". The admin selects which user the workstation is being issued to: by default a workstation is bound to one named user, so that the device credential plus that user's password are the two factors of KVKK 2018/10 (see Compliance). A shared-workstation mode exists as an explicit opt-in toggle for cases like a front desk or a rotating exam room, with an in-UI warning that this weakens the two-factor framing and obligates the clinic to compensate via stricter idle lock and operational controls. The flow then produces a short enrollment string — a URL like `anamnez://enroll?host=10.0.0.5&fingerprint=AB:CD:…&token=…` — that contains everything the workstation client needs: where to find the server, which cert fingerprint to trust, and a one-time token to exchange for a long-lived device credential.
3. The admin sends the enrollment string to the workstation user through whatever channel is convenient — copy/paste, email, internal chat. The user opens the already-installed anamnez client, pastes the string, and is done. The client connects to the server, verifies the pinned fingerprint, exchanges the one-time token for a freshly-issued client certificate (signed by the server's CA, valid 25 years, `CN = device_id`), and stores the cert + private key in the OS secret store.

### Server certificate and CA

The first-boot wizard generates a single long-lived (25-year) Ed25519 keypair on the Mac Studio. The same key serves two roles:

- It is the server's TLS identity: workstations pin its public-key fingerprint at enrollment, and every subsequent connection validates against that pin.
- It is the local CA that signs workstation client certificates at enrollment. There is no external PKI involved — the server is the entire trust root for its own clinic.

Each workstation is issued a freshly-generated client cert at enrollment time, with `CN = device_id` and a 25-year validity matching the server's. Revocation is by row, not CRL: the server keeps an in-memory deny set of revoked `device_id`s loaded from the workstation table at startup, and rejects any TLS handshake whose client cert maps to a revoked or deleted device. mTLS handshakes therefore live or die by the server's own bookkeeping; we do not need OCSP, a CRL distribution point, or an external revocation channel.

**Cert rotation is rare and disruptive.** The expected lifetime of the keypair is the lifetime of the appliance. If rotation becomes necessary — suspected key compromise, hardware migration where the SEP-wrapped passphrase path is no longer viable, or a forced cryptographic upgrade — the procedure is `anamnez admin rotate-server-cert`, which invalidates every workstation enrollment in the same transaction. The admin then re-issues an enrollment string per workstation and the clinic re-enrolls. This is deliberately heavyweight: routine in-band rotation would be an attack surface we'd rather not maintain.

### Key custody

The SQLCipher passphrase is a randomly generated 256-bit secret, created during the first-boot wizard and never displayed. It is stored on disk only in wrapped form, two ways:

- `wrap_sep.bin` — wrapped by a key generated inside the Mac Studio's Secure Enclave (SEP). The SEP key is bound to this specific Mac, never leaves the chip, and has no user-presence gate because the appliance must boot unattended. On every cold boot the anamnez daemon asks the SEP to unwrap the passphrase. This is the normal path.
- `wrap_recovery.bin` — wrapped by `Argon2id(recovery_code)` where the recovery code is the physical string printed by the first-boot wizard. This is the disaster-recovery path: if the Mac Studio dies, the admin restores the encrypted backup onto a fresh Mac Studio, types the recovery code into the new wizard, and the daemon unwraps via Argon2 instead of via the (now-gone) SEP.

FileVault is enabled in unattended-boot mode, with its own volume key sealed to the SEP. This protects the disk at rest from an attacker who steals the SSD alone. The SEP wrap of the SQLCipher passphrase is a second, independent layer.

What this defends against:

- **Stolen SSD or cloned disk** — useless. Both FileVault and SQLCipher keys are SEP-bound to the original Mac.
- **Stolen whole Mac, powered off** — the adversary must power it on; the daemon's hardening (signed binaries, no shell access for clinic users, locked-down launchd, no remote login) is the next perimeter.
- **Adversary with code execution on the running Mac** — can ask the SEP to unwrap. This is unavoidable for any unattended-boot system: if no human is required at boot, no human-held secret can gate startup. We accept this and harden the appliance accordingly.
- **Loss or destruction of the Mac Studio** — recoverable only with the physical recovery code. Without it, data is gone. This is the correct trade-off; the recovery code is therefore the single most important physical artifact the clinic possesses, and the admin is instructed at first boot to store it the way they would store a safe combination.

### Explicitly out of scope for MVP

- Public internet exposure of the Mac Studio.
- Remote access (doctors working from home, multiple clinic sites, roaming devices).
- Mobile apps.
- Mesh VPN / overlay networking between workstations and the Mac Studio.
- Cross-clinic anything.

These are deferred, not abandoned. When remote access becomes a real requirement for a real clinic, the path is to add a self-hosted Headscale instance alongside anamnez on the same Mac Studio — the enrollment flow stays the same, with anamnez additionally minting a Headscale pre-auth key during workstation registration. Tailscale-the-SaaS is rejected as a control plane because it puts coordination metadata for clinic networks into a third party's hands.

## Workstation client

### Stack

Tauri + Leptos. The UI is Leptos components compiled to WebAssembly, running inside Tauri's bundled webview (WKWebView on macOS, WebView2 on Windows). The native side — file dialogs, OS secret store, microphone capture, deep-link handling — is Rust calling Tauri APIs. No JavaScript in the source tree, no Node in the build pipeline.

Picked over iced (component ecosystem too thin for a form-and-table-heavy clinical UI), egui (looks like a debug tool), and Slint (royalty-bearing license for closed-source distribution). Picked over Tauri-with-React because Leptos lets us share `anamnez-protocol` types directly with the WASM client: write the `Serialize`/`Deserialize` derives once, both ends use them.

Costs accepted:

- The Leptos component ecosystem (`thaw`, `leptonic`) is smaller than React's; bespoke widgets (patient timeline, observation scrubber, structured-coding autocomplete) are ours to build.
- WASM-in-webview adds ~10 MB to the installed footprint vs a pure-iced binary.
- TLS to the server goes through `rustls` on the Tauri side, not through the webview's stack — the OS trust store is still never touched.

### Wire protocol

HTTPS + JSON over the pinned-fingerprint TLS connection. Server-Sent Events on a long-lived `GET /events` for server push (observation amended elsewhere, patient access changed, forced logout). No WebSockets, no gRPC, no protobuf toolchain.

Every request is authenticated as `(device, user)`:

- **Device** — mTLS using the client certificate issued during enrollment.
- **User** — bearer token in `Authorization: Bearer …`. Access tokens are short-lived (15 minutes) and held in memory only. Refresh tokens are longer-lived (12 hours), stored in the OS secret store, and rotated on each use — one-time-use, so replay of a leaked refresh fails. Each `auth_session` also carries a 30-day `absolute_expires_at` set at login; once that horizon passes, refresh fails and the user must log in again regardless of recent activity. Admin revocation or password change sets `auth_session.revoked_at`; the server checks this column on every authenticated request, so revocation takes effect immediately regardless of access-token lifetime. The DB read per request is a single indexed lookup — negligible at clinic scale.

**Step-up reauthentication.** Some operations require the user to re-enter their password immediately before the action, regardless of how recently they logged in: creating or modifying a user, granting `patient_access` to a user who did not previously have access to that patient (the creator-as-owner case is exempt), disabling a user or revoking a workstation, exporting a patient dossier or downloading query results above a row threshold, changing retention or destruction policy, and generating a workstation enrollment string. No second factor — just re-entry of the existing password. The point is to defeat "walked-away unlocked screen" and session token replay against the most damaging actions, without adding an MFA dependency.

### What the workstation persists

- Device credential (mTLS client cert + private key) — in the OS secret store.
- Pinned server certificate fingerprint — config file.
- Refresh token — in the OS secret store.
- UI preferences (window size, last filter, etc.) — config file.

**No clinical data, ever, on workstation disk.** No patient records, no observations, no source documents, no audit-log copies. A lost laptop loses nothing PHI-bearing. A wipe of the workstation's data directory loses session and preferences only — re-enroll and continue.

### Session security

The workstation client locks the session after a period of user inactivity (no keyboard, mouse, or focused interaction), requiring password re-entry to resume. Default 10 minutes; the clinic may configure between 5 and 30 minutes, but cannot disable idle lock — the upper bound is hard-coded.

This is what makes the KVKK two-factor framing actually hold in practice (see Compliance). Without idle lock, an unattended logged-in workstation hands a passerby both factors at once: the enrolled device sitting on the desk and an already-authenticated session. The lock turns "physically present at the device" back into a meaningful access boundary instead of a paper one.

High-risk admin operations require step-up reauthentication regardless of session age — see Wire protocol.

### Offline behavior

Thin client. Patient data lives in the workstation's RAM only while a view is open; navigating away frees it. LAN drop is an unmissable banner ("Disconnected from clinic server"), not silent retry. Pending writes at the moment of disconnect are surfaced and discarded on user confirmation — there is no outbox, no sync queue, no CRDT.

Clinic LAN outages are rare, and an offline-edit path introduces an entire class of sync-conflict bugs plus a regulatory question (when did the amendment happen — at the client or at the server?). MVP commits to: online means write, offline means read-only on what is already in RAM, no exceptions.

### Files and audio

- **Documents** (PDFs, images, scans) are drag-dropped onto the client, streamed in chunks to the server's blob store, and `source_document` rows are created server-side. The client never holds a file beyond its upload buffer.
- **Audio** for transcription is captured natively via `cpal` (WASAPI on Windows, CoreAudio on macOS), streamed to the server, transcription runs server-side. The client never stores audio. Native capture rather than the webview's `getUserMedia` avoids per-OS webview quirks and uses the same permission model on both platforms.

### Platform support

Windows is primary; macOS is secondary. Linux is not supported. This collapses code signing to two pipelines — Authenticode for Windows and Apple Developer ID + notarization for macOS — and removes `libsecret`, PulseAudio, and PipeWire from the dependency surface.

### Language

The shipping UI is Turkish only. All interface text, labels, autocomplete output, validation messages, and error states are Turkish. English is used in the codebase, this README, and developer-facing documentation, never in the product surface that a clinician sees.

### Updates

Manual for MVP. We ship a new signed installer; clinic admins update workstations. The server enforces a minimum-client-version check on connect and refuses outdated clients with a clear message. Auto-update through a Mac-Studio-mediated channel is deferred — when added, only the Mac Studio reaches our update host; workstations always stay on the LAN.

## Development

The tech stack is Rust, Rust, and Rust.

We fail loudly. We don't fall-back, unless we absolutely need to in an intentional manner. We don't "default" to things and hide errors and create situations where it's unclear which value is set where and why some behavior is ocurring. A crash is million times better than a hidden bug that is masked and intermittently creates minor problems but never gets fixed.

We will have configuration as a first-class citizen. Invalid configs will throw, panic, crash, and burn. We will have business logic that we write that determines the validity of configs.

### Workspace structure

The codebase is one Cargo workspace. Functionality lives in library crates; binary crates are thin shells.

| Crate | Kind | Builds for | Role |
|---|---|---|---|
| `anamnez-protocol` | lib | native + `wasm32` | Wire types and error envelope. Shared by server, CLI, and client. The one crate that must stay platform-agnostic. |
| `anamnez-core` | lib | native | All server-side functionality: DB access, auth, observation/patient/encounter logic, audit chain, key custody glue, blob store, LLM/OCR/transcription traits and impls. No HTTP, no binary entrypoint. |
| `anamnez` | bin | native | The Mac Studio binary. Multi-tool with subcommands — `serve` (long-running daemon, called by launchd), `init` (first-boot wizard), `migrate`, `backup`, `restore`, `audit verify`, `retention sweep`, `health`, `admin add-user`, `admin disable-user` (refuses to proceed while the target is sole owner of any patient; admin must designate a successor), `admin reset-password`, `admin enroll-workstation`, `admin breach-report`, `admin rotate-server-cert`. Every subcommand drives `anamnez-core` directly; `serve` happens to expose it over HTTP, the rest call the same library in-process. |
| `anamnez-client-core` | lib | native + `wasm32` | Client-side: API client, session and refresh logic, conflict resolution, view state machine. No GUI framework imports. Native build target exists so it can be tested in `cargo nextest`. |
| `anamnez-workstation-ui` | lib | `wasm32` | Leptos components and view logic. |
| `anamnez-workstation` | bin | native | Tauri shell. Hosts the Leptos WASM, exposes native commands (OS secret store, mic capture via `cpal`, file dialogs, `anamnez://` deep-link handler). The only crate that imports a GUI framework. |
| `xtask` | bin | native | Development tasks: `record-fixture` for LLM/OCR/transcription test fixtures, signing scripts, build pipelines. |

CLI concurrency rule: write subcommands (`migrate`, `restore`, `admin …`) refuse to run while `serve` is up, detected by a PID file. Read subcommands (`audit verify`, `backup`, `health`) run alongside `serve` — SQLite WAL handles concurrent readers. One writer, always.

Reasons this split exists:

- `anamnez-core` is the only thing that needs to be tested in isolation at layer 1 of the test cake — no HTTP, no Tauri, no GUI.
- The CLI can do anything the server can — ops, recovery, first-boot — without going through HTTP. The CLI is the recovery path when the server itself is broken.
- The Tauri shell is replaceable: swapping Leptos for iced or anything else touches `anamnez-workstation-ui` + `anamnez-workstation` only.
- `anamnez-protocol` and `anamnez-client-core` build for both native and `wasm32`, which forces them to stay free of filesystem APIs, native-only tokio features, and other things that would break the WASM build.

### Testing

Tests are organized in three layers. Each is hermetic — no test reaches the public network, no test depends on developer-machine state.

| Layer | Where it runs | What it covers |
|---|---|---|
| 1. `cargo nextest` | Dev machine + Linux CI | Business logic, SQLite, in-process server, external systems behind trait fixtures |
| 2. testcontainers-rs | Dev machine + Linux CI | Real `anamnez` and `anamnez-workstation` binaries, real TLS, real TCP, toxiproxy for LAN faults |
| 3. NixOS VM tests | Linux CI only | Multi-node topology, concurrent edits, cert rotation, enrollment dance |

Layer 3 uses NixOS's multi-node test driver: each node is a QEMU/KVM microVM with its own network namespace, the whole topology declared in Nix and driven from a Python test script. Devs do not need Nix locally — layers 1 and 2 give a 95% inner-loop signal on a Mac.

**Mocks live behind trait seams that represent external systems** (LLM, OCR, transcription, OpenRouter, clock, blob store) and nowhere else. No production code path falls back to a mock-shaped default. If the local LLM is down at runtime, the server crashes — it does not silently degrade. The trait abstraction exists for testability, not for runtime swappability.

**LLM, OCR, and transcription calls in tests are fixture-backed.** Each call hashes `(provider_id, model_id, normalized_prompt, params)` (temperature pinned to `0` in test mode) and looks up `tests/fixtures/<provider>/<hash>.json`. A miss is a hard test failure with a message instructing the developer to run `cargo xtask record-fixture <key>` — the only path in the repo that talks to a real model. New prompts produce new committed fixtures, deliberately, not silent CI passes.

Real-model verification (Apple Vision OCR, MLX inference, the actual hosted LLM behaviour) happens out-of-band on a dev Mac before tagging a release. CI does not depend on Apple hardware.
