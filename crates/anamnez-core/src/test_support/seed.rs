//! `TestWorld` builder — seeds a known set of users, patients, and encounters
//! into a `TempDb` so individual tests can focus on what they're asserting.

use super::harness::TempDb;
use crate::error::Result;
use crate::ids::{EncounterId, PatientId, UserId};

pub struct TestWorld {
    pub db: TempDb,
    pub admin: UserId,
    pub provider1: UserId,
    pub provider2: UserId,
    pub patient_a: PatientId,
    pub encounter_a: EncounterId,
}

impl TestWorld {
    /// Default builder: one admin, two providers, one patient owned by provider1,
    /// one in-progress encounter on patient_a by provider1.
    pub fn new() -> Result<Self> {
        todo!("TestWorld::new — Step 1.3")
    }
}
