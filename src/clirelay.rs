use reqwest::{
    Client as HttpClient,
    header::{AUTHORIZATION, HeaderMap, HeaderValue},
};
use secrecy::ExposeSecret;
use serde::Serialize;
use url::Url;

use crate::{
    config::CliRelayConfig,
    error::{AppError, AppResult},
    http,
};

const MANAGEMENT_BASE_PATH: &str = "/v0/management/";
const API_KEY_ENTRIES_PATH: &str = "api-key-entries";

#[derive(Clone)]
pub struct CliRelayClient {
    http: HttpClient,
    base_url: Url,
    default_permission_profile_id: String,
    default_allowed_channel_groups: Vec<String>,
}

impl CliRelayClient {
    pub fn new(config: &CliRelayConfig) -> AppResult<Self> {
        let mut headers = HeaderMap::new();
        let auth = format!("Bearer {}", config.management_key.expose_secret());
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&auth).map_err(|err| {
                AppError::Config(format!("invalid CliRelay management key header: {err}"))
            })?,
        );

        let http = http::client_builder().default_headers(headers).build()?;

        let base_url = config
            .internal_base_url
            .as_ref()
            .join(MANAGEMENT_BASE_PATH)?;
        Ok(Self {
            http,
            base_url,
            default_permission_profile_id: config.default_permission_profile_id.trim().to_owned(),
            default_allowed_channel_groups: config.default_allowed_channel_groups.clone(),
        })
    }

    pub async fn provision_api_key(&self, api_key: &str, name: &str) -> AppResult<()> {
        let body = ApiKeyEntryPatch {
            match_key: api_key,
            value: ApiKeyEntryValue {
                key: api_key,
                name,
                permission_profile_id: &self.default_permission_profile_id,
                allowed_channel_groups: &self.default_allowed_channel_groups,
            },
        };

        let response = self
            .http
            .patch(self.url(API_KEY_ENTRIES_PATH))
            .json(&body)
            .send()
            .await?;

        if response.status().is_success() {
            return Ok(());
        }

        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        Err(AppError::Upstream(format!(
            "CliRelay API key provisioning failed with {status}: {text}"
        )))
    }

    fn url(&self, path: &str) -> Url {
        self.base_url
            .join(path)
            .expect("management API path must be relative")
    }
}

#[derive(Debug, Serialize)]
struct ApiKeyEntryPatch<'a> {
    #[serde(rename = "match")]
    match_key: &'a str,
    value: ApiKeyEntryValue<'a>,
}

#[derive(Debug, Serialize)]
struct ApiKeyEntryValue<'a> {
    key: &'a str,
    name: &'a str,
    #[serde(
        rename = "permission-profile-id",
        skip_serializing_if = "str::is_empty"
    )]
    permission_profile_id: &'a str,
    #[serde(
        rename = "allowed-channel-groups",
        skip_serializing_if = "<[String]>::is_empty"
    )]
    allowed_channel_groups: &'a [String],
}
