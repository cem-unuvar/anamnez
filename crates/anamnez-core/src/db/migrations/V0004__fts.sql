-- README §Storage → Autocomplete — FTS5 vtables for autocomplete over each code system.
-- Inputs are pre-folded through `code_systems::turkish::casefold` before insert/query.
-- `remove_diacritics=0` keeps `ş`/`ç` distinct from `s`/`c`.

CREATE VIRTUAL TABLE fts_drug_atc USING fts5(
    atc_code UNINDEXED,
    folded_display,
    tokenize = 'unicode61 remove_diacritics 0'
);

CREATE VIRTUAL TABLE fts_drug_titck USING fts5(
    barcode UNINDEXED,
    folded_display,
    tokenize = 'unicode61 remove_diacritics 0'
);

CREATE VIRTUAL TABLE fts_icd10_tm USING fts5(
    code UNINDEXED,
    folded_display,
    tokenize = 'unicode61 remove_diacritics 0'
);

CREATE VIRTUAL TABLE fts_loinc USING fts5(
    code UNINDEXED,
    folded_display,
    tokenize = 'unicode61 remove_diacritics 0'
);

CREATE VIRTUAL TABLE fts_procedure_sut USING fts5(
    sut_code UNINDEXED,
    folded_display,
    tokenize = 'unicode61 remove_diacritics 0'
);

CREATE VIRTUAL TABLE fts_visit_purpose_skrs USING fts5(
    code UNINDEXED,
    folded_display,
    tokenize = 'unicode61 remove_diacritics 0'
);

CREATE VIRTUAL TABLE fts_symptom_anamnez USING fts5(
    code UNINDEXED,
    folded_display,
    tokenize = 'unicode61 remove_diacritics 0'
);
