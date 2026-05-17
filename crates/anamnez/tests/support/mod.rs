//! Shared layer-2 test harness. Each test file does `mod support;` to import.

#![allow(dead_code)] // Tests use a subset; the rest is in place for future suites.

pub mod api;
pub mod bootstrap;
pub mod cert_mint;
pub mod spawn;
pub mod tls;
