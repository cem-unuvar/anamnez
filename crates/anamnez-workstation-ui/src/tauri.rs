//! Tauri `invoke` wrapper for WASM. The native Tauri runtime exposes
//! `window.__TAURI_INTERNALS__.invoke(cmd, args)` which returns a Promise.

use serde::de::DeserializeOwned;
use serde::Serialize;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = ["window", "__TAURI_INTERNALS__"], js_name = invoke)]
    fn tauri_invoke(cmd: &str, args: JsValue) -> js_sys::Promise;
}

#[derive(Debug, thiserror::Error)]
pub enum InvokeError {
    #[error("serialize args: {0}")]
    SerializeArgs(String),
    #[error("invoke failed: {0}")]
    Invoke(String),
    #[error("deserialize reply: {0}")]
    Deserialize(String),
}

/// Call a Tauri command. Errors map onto `InvokeError`.
pub async fn invoke<R: DeserializeOwned>(
    cmd: &str,
    args: impl Serialize,
) -> Result<R, InvokeError> {
    let js_args =
        serde_wasm_bindgen::to_value(&args).map_err(|e| InvokeError::SerializeArgs(e.to_string()))?;
    let result = JsFuture::from(tauri_invoke(cmd, js_args))
        .await
        .map_err(|e| InvokeError::Invoke(format!("{e:?}")))?;
    serde_wasm_bindgen::from_value(result).map_err(|e| InvokeError::Deserialize(e.to_string()))
}

/// Read `performance.now()` for the idle-lock timer.
#[must_use]
pub fn now_ms() -> f64 {
    web_sys::window()
        .and_then(|w| w.performance())
        .map_or(0.0, |p| p.now())
}
