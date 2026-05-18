//! Workstation client library. Builds for native (Tauri shell) and wasm32 (Leptos UI).
//!
//! Architecture: the Tauri shell consumes the **native** build, which carries the real
//! HTTP transport (`reqwest` + custom `rustls` server-fingerprint pin verifier + mTLS
//! client identity loaded from the OS secret store). The Leptos UI consumes the
//! **wasm32** build, whose `HttpTransport` impl is a thin forwarder over the Tauri
//! `invoke` IPC channel — SPEC §Workstation client requires TLS-to-server to go through
//! `rustls` on the native side, not through the webview's stack.
//!
//! What lives here:
//! - View state machine (`state`) — `AppMode`, idle-lock timer state. Pure logic; same
//!   code runs on both targets and is exercised on native by `cargo nextest`.
//! - Session helpers (`session`) — `Session` value type + refresh decision logic.
//! - Conflict resolution scaffolding (`conflict`) — wired through even though no
//!   editor UI ships in this slice; keeps the seam ready.
//! - `HttpTransport` trait (`transport`) — the seam between view code and bytes.
//! - Two impls, behind cfg/feature gates:
//!     - `transport_native` (gated on the `native-transport` feature): real HTTP.
//!     - `transport_tauri` (`#[cfg(target_arch = "wasm32")]`): IPC forwarder.

pub mod conflict;
pub mod error;
pub mod session;
pub mod state;
pub mod transport;

#[cfg(all(not(target_arch = "wasm32"), feature = "native-transport"))]
pub mod transport_native;

#[cfg(all(not(target_arch = "wasm32"), feature = "native-transport"))]
pub mod secret_native;

#[cfg(target_arch = "wasm32")]
pub mod transport_tauri;

pub use error::ClientError;
pub use session::Session;
pub use state::AppMode;
pub use transport::{ConnectedEndpoint, EnrollEndpoint, HttpTransport};
