-- README §Storage → Audit log integrity — DB-layer enforcement.
-- Any UPDATE/DELETE on `audit_log` aborts with the message `audit immutable`.

CREATE TRIGGER trg_audit_log_no_update
BEFORE UPDATE ON audit_log
BEGIN
    SELECT RAISE(ABORT, 'audit immutable');
END;

CREATE TRIGGER trg_audit_log_no_delete
BEFORE DELETE ON audit_log
BEGIN
    SELECT RAISE(ABORT, 'audit immutable');
END;
