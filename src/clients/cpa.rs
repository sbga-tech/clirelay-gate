use reqwest::{
    Client as HttpClient,
    header::{AUTHORIZATION, HeaderMap, HeaderValue},
};
use secrecy::ExposeSecret;
use serde::Serialize;
use url::Url;

use crate::{
    clients::http,
    config::CPAConfig,
    error::{AppError, AppResult},
};

const MANAGEMENT_BASE_PATH: &str = "/v0/management/";
const API_KEYS_PATH: &str = "api-keys";

#[derive(Clone)]
pub struct CPAClient {
    http: HttpClient,
    base_url: Url,
}

impl CPAClient {
    pub fn new(config: &CPAConfig) -> AppResult<Self> {
        let mut headers = HeaderMap::new();
        let auth = format!("Bearer {}", config.management_key.expose_secret());
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&auth).map_err(|err| {
                AppError::Config(format!("invalid CPA management key header: {err}"))
            })?,
        );

        let http = http::client_builder().default_headers(headers).build()?;
        let base_url = config
            .internal_base_url
            .as_ref()
            .join(MANAGEMENT_BASE_PATH)?;

        Ok(Self { http, base_url })
    }

    pub async fn provision_api_key(&self, api_key: &str) -> AppResult<()> {
        let body = ApiKeyPatch {
            old_key: "",
            new_key: api_key,
        };

        let response = self
            .http
            .patch(self.url(API_KEYS_PATH))
            .json(&body)
            .send()
            .await?;

        if response.status().is_success() {
            return Ok(());
        }

        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        Err(AppError::Upstream(format!(
            "CPA API key creation failed with {status}: {text}"
        )))
    }

    fn url(&self, path: &str) -> Url {
        self.base_url
            .join(path)
            .expect("management API path must be relative")
    }
}

#[derive(Debug, Serialize)]
struct ApiKeyPatch<'a> {
    #[serde(rename = "old")]
    old_key: &'a str,
    #[serde(rename = "new")]
    new_key: &'a str,
}
