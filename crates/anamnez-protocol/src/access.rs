use crate::ids::{PatientId, UserId};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AccessLevel {
    Owner,
    Collaborator,
    ReadOnly,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatientAccess {
    pub patient_id: PatientId,
    pub user_id: UserId,
    pub level: AccessLevel,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrantAccessRequest {
    pub user_id: UserId,
    pub level: AccessLevel,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferOwnershipRequest {
    pub from: UserId,
    pub to: UserId,
}
