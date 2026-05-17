//! Per-level capability checks.

use super::AccessLevel;

/// Can this level read patient data?
#[must_use]
pub const fn can_read(level: AccessLevel) -> bool {
    matches!(
        level,
        AccessLevel::Owner | AccessLevel::Collaborator | AccessLevel::ReadOnly
    )
}

/// Can this level write observations / allergies / medications / consents?
#[must_use]
pub const fn can_write_clinical(level: AccessLevel) -> bool {
    matches!(level, AccessLevel::Owner | AccessLevel::Collaborator)
}

/// Can this level grant or revoke `patient_access` rows?
#[must_use]
pub const fn can_manage_access(level: AccessLevel) -> bool {
    matches!(level, AccessLevel::Owner)
}

/// Can this level transfer ownership?
#[must_use]
pub const fn can_transfer_ownership(level: AccessLevel) -> bool {
    matches!(level, AccessLevel::Owner)
}
