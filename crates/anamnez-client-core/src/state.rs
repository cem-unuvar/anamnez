//! Target-agnostic view state machine.
//!
//! - `AppMode` — what the workstation should be showing at any moment.
//! - `IdleLock` — last-activity tracker. The WASM side wires DOM events to `note_activity`
//!   and ticks `check`; the native side does the same with platform events if it ever
//!   needs to. The logic is identical and tested on native.
//! - `connection_banner` flag — toggled by the transport result classifier in
//!   `apply_transport_outcome`.

use anamnez_protocol::environment::Environment;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AppMode {
    /// No workstation credential on disk — show enrollment screen.
    #[default]
    Bootstrap,
    /// Workstation credential present, no session — show login screen.
    LoggedOut,
    /// User typed credentials; awaiting server response.
    LoggingIn,
    /// Session active — show shell.
    AppShell,
    /// Idle-lock fired; require password to resume. Same as `LoggedOut` shape-wise,
    /// but distinguished so the UI can show a "session locked" affordance.
    Locked,
}

impl AppMode {
    #[must_use]
    pub fn requires_login(self) -> bool {
        matches!(self, Self::LoggedOut | Self::Locked)
    }
}

/// Inactivity tracker. `last_activity_ms` is a monotonic milliseconds counter (native:
/// `Instant`; wasm: `performance.now()`). Caller supplies the value via `now_ms`; this
/// keeps the type target-agnostic.
#[derive(Debug, Clone, Copy)]
pub struct IdleLock {
    pub last_activity_ms: f64,
    pub timeout_ms: f64,
}

impl IdleLock {
    #[must_use]
    pub fn new(now_ms: f64, minutes: u32) -> Self {
        Self {
            last_activity_ms: now_ms,
            timeout_ms: f64::from(minutes) * 60_000.0,
        }
    }

    pub fn note_activity(&mut self, now_ms: f64) {
        self.last_activity_ms = now_ms;
    }

    /// True if `now_ms - last_activity_ms >= timeout_ms`. Caller transitions
    /// `AppMode::AppShell -> AppMode::Locked` on `true`.
    #[must_use]
    pub fn expired(&self, now_ms: f64) -> bool {
        now_ms - self.last_activity_ms >= self.timeout_ms
    }

    pub fn set_minutes(&mut self, minutes: u32) {
        self.timeout_ms = f64::from(minutes) * 60_000.0;
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppState {
    pub mode: AppMode,
    pub environment: Environment,
    pub idle_lock_minutes: u32,
    /// True iff the last HTTP attempt failed at the transport layer (TLS, DNS, refused).
    /// Cleared on the next successful request.
    pub disconnected: bool,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            mode: AppMode::Bootstrap,
            environment: Environment::Production,
            idle_lock_minutes: 10,
            disconnected: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idle_lock_expires_after_timeout() {
        let mut g = IdleLock::new(0.0, 10);
        assert!(!g.expired(0.0));
        assert!(!g.expired(599_999.0));
        assert!(g.expired(600_000.0));
        g.note_activity(700_000.0);
        assert!(!g.expired(700_001.0));
        assert!(g.expired(1_300_001.0));
    }

    #[test]
    fn app_mode_requires_login() {
        assert!(AppMode::LoggedOut.requires_login());
        assert!(AppMode::Locked.requires_login());
        assert!(!AppMode::AppShell.requires_login());
        assert!(!AppMode::Bootstrap.requires_login());
    }
}
