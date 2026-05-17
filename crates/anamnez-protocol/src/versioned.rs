//! Optimistic-locking wrapper for mutable clinical entities.

use serde::{Deserialize, Serialize};

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
