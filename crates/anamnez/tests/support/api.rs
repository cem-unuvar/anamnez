//! Thin wrapper around reqwest for layer-2 tests. Each test composes calls.

use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION};
use serde::{de::DeserializeOwned, Serialize};

pub struct Api {
    pub client: reqwest::Client,
    pub base: String,
    pub bearer: Option<String>,
    pub client_version: String,
    pub stepup_password: Option<String>,
}

impl Api {
    pub fn new(client: reqwest::Client, base: String) -> Self {
        Self {
            client,
            base,
            bearer: None,
            client_version: "1.0.0".into(),
            stepup_password: None,
        }
    }

    fn headers(&self) -> HeaderMap {
        let mut h = HeaderMap::new();
        h.insert(
            "x-client-version",
            HeaderValue::from_str(&self.client_version).unwrap(),
        );
        if let Some(b) = &self.bearer {
            h.insert(
                AUTHORIZATION,
                HeaderValue::from_str(&format!("Bearer {b}")).unwrap(),
            );
        }
        if let Some(p) = &self.stepup_password {
            h.insert("x-step-up-password", HeaderValue::from_str(p).unwrap());
        }
        h
    }

    pub async fn post<T: Serialize, R: DeserializeOwned>(
        &self,
        path: &str,
        body: &T,
    ) -> anyhow_lite::Result<(reqwest::StatusCode, Option<R>)> {
        let resp = self
            .client
            .post(format!("{}{}", self.base, path))
            .headers(self.headers())
            .json(body)
            .send()
            .await?;
        let status = resp.status();
        if status.is_success() {
            let v: R = resp.json().await?;
            Ok((status, Some(v)))
        } else {
            Ok((status, None))
        }
    }

    pub async fn post_raw<T: Serialize>(
        &self,
        path: &str,
        body: &T,
    ) -> anyhow_lite::Result<reqwest::Response> {
        let resp = self
            .client
            .post(format!("{}{}", self.base, path))
            .headers(self.headers())
            .json(body)
            .send()
            .await?;
        Ok(resp)
    }

    pub async fn patch_raw<T: Serialize>(
        &self,
        path: &str,
        body: &T,
    ) -> anyhow_lite::Result<reqwest::Response> {
        let resp = self
            .client
            .patch(format!("{}{}", self.base, path))
            .headers(self.headers())
            .json(body)
            .send()
            .await?;
        Ok(resp)
    }

    pub async fn get_raw(&self, path: &str) -> anyhow_lite::Result<reqwest::Response> {
        let resp = self
            .client
            .get(format!("{}{}", self.base, path))
            .headers(self.headers())
            .send()
            .await?;
        Ok(resp)
    }
}

// Tiny inline result alias to avoid pulling in the full `anyhow` dep.
pub mod anyhow_lite {
    pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;
}
