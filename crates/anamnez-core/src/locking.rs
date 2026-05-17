//! README §Storage → Concurrency — optimistic locking primitive.
//!
//! Every mutable clinical row carries `version INTEGER NOT NULL`. Writes are
//! `UPDATE … SET …, version = version + 1 WHERE id = ? AND version = ?`.
//! Zero rows affected → `Error::Conflict`.

use serde::{Deserialize, Serialize};

/// Wraps a row with its current version for concurrency control.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Versioned<T> {
    pub value: T,
    pub version: i64,
}

impl<T> Versioned<T> {
    pub fn new(value: T, version: i64) -> Self {
        Self { value, version }
    }
}
