-- README §Privacy — singleton environment marker.
-- Exactly one row, written at first boot, never updated or deleted thereafter.

CREATE TABLE environment_marker (
    -- `singleton` pins the row to id=1 so a second insert fails.
    singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
    environment TEXT NOT NULL CHECK (environment IN ('production','test')),
    written_at TEXT NOT NULL
) STRICT;

CREATE TRIGGER trg_environment_marker_no_update
BEFORE UPDATE ON environment_marker
BEGIN
    SELECT RAISE(ABORT, 'environment marker immutable');
END;

CREATE TRIGGER trg_environment_marker_no_delete
BEFORE DELETE ON environment_marker
BEGIN
    SELECT RAISE(ABORT, 'environment marker immutable');
END;
