-- SPEC §Deployment — pending workstation enrollments.
--
-- `anamnez admin enroll-workstation` mints a one-time token and inserts a
-- pending row here. The workstation client posts the token to the daemon's
-- `/v1/enroll/exchange` route; the daemon mints a client cert, inserts a
-- matching `workstation` row, and marks `claimed_at` here. A `workstation`
-- row never exists for a pending or unclaimed enrollment.

CREATE TABLE workstation_enrollment (
    id                      TEXT PRIMARY KEY NOT NULL,
    label                   TEXT NOT NULL,
    mode                    TEXT NOT NULL CHECK (mode IN ('bound', 'shared')),
    bound_user_id           TEXT REFERENCES user(id) ON DELETE RESTRICT,
    token_hash              BLOB NOT NULL UNIQUE,
    created_by              TEXT NOT NULL REFERENCES user(id) ON DELETE RESTRICT,
    created_at              TEXT NOT NULL,
    expires_at              TEXT NOT NULL,
    claimed_at              TEXT,
    claimed_workstation_id  TEXT REFERENCES workstation(id) ON DELETE RESTRICT,
    CHECK ((mode = 'bound') = (bound_user_id IS NOT NULL)),
    CHECK ((claimed_at IS NULL) = (claimed_workstation_id IS NULL))
) STRICT;

CREATE INDEX idx_workstation_enrollment_unclaimed
    ON workstation_enrollment(expires_at) WHERE claimed_at IS NULL;
