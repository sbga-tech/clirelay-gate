use reqwest::{
    Client as HttpClient,
    header::{AUTHORIZATION, HeaderMap, HeaderValue},
};
use secrecy::ExposeSecret;
use serde::{Deserialize, Serialize};
use url::Url;

use crate::{
    clients::http,
    config::CliRelayConfig,
    error::{AppError, AppResult},
};

const MANAGEMENT_BASE_PATH: &str = "/v0/management/";
const API_KEYS_PATH: &str = "api-keys";
const API_KEY_ENTRIES_PATH: &str = "api-key-entries";
const USAGE_CHART_DATA_PATH: &str = "usage/chart-data";

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

    pub async fn api_key_entries(&self) -> AppResult<Vec<CliRelayApiKeyEntry>> {
        let response = self.http.get(self.url(API_KEY_ENTRIES_PATH)).send().await?;
        if !response.status().is_success() {
            return Err(upstream_response_error(response, "API key list").await);
        }

        Ok(response
            .json::<ApiKeyEntriesResponse>()
            .await?
            .api_key_entries)
    }

    pub async fn provision_api_key(
        &self,
        api_key: &str,
        name: &str,
    ) -> AppResult<CliRelayApiKeyEntry> {
        self.create_api_key(api_key).await?;
        if let Err(error) = self.update_api_key_metadata(api_key, name).await {
            if let Err(cleanup_error) = self.delete_api_key_by_key(api_key).await {
                tracing::warn!(
                    error = %cleanup_error,
                    "failed to roll back CliRelay API key after metadata update failure"
                );
            }
            return Err(error);
        }

        let mut matches = self
            .api_key_entries()
            .await?
            .into_iter()
            .filter(|entry| entry.key == api_key);
        let entry = matches.next().ok_or_else(|| {
            AppError::Upstream("CliRelay did not return the newly created API key".into())
        })?;
        if matches.next().is_some() || entry.id.trim().is_empty() {
            return Err(AppError::Upstream(
                "CliRelay returned an invalid API key identity".into(),
            ));
        }
        Ok(entry)
    }

    pub async fn rotate_api_key(&self, id: &str, api_key: &str) -> AppResult<()> {
        let body = ApiKeyEntryKeyPatch {
            id,
            value: ApiKeyEntryKeyValue { key: api_key },
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
        Err(upstream_response_error(response, "API key rotation").await)
    }

    pub async fn delete_api_key(&self, id: &str) -> AppResult<()> {
        let mut url = self.url(API_KEY_ENTRIES_PATH);
        url.query_pairs_mut()
            .append_pair("id", id)
            .append_pair("delete_logs", "false");
        let response = self.http.delete(url).send().await?;
        if response.status().is_success() {
            return Ok(());
        }
        Err(upstream_response_error(response, "API key deletion").await)
    }

    async fn create_api_key(&self, api_key: &str) -> AppResult<()> {
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
        Err(upstream_response_error(response, "API key creation").await)
    }

    async fn update_api_key_metadata(&self, api_key: &str, name: &str) -> AppResult<()> {
        let body = ApiKeyEntryMetadataPatch {
            match_key: api_key,
            value: ApiKeyEntryMetadataValue {
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
        Err(upstream_response_error(response, "API key metadata update").await)
    }

    async fn delete_api_key_by_key(&self, api_key: &str) -> AppResult<()> {
        let mut url = self.url(API_KEY_ENTRIES_PATH);
        url.query_pairs_mut()
            .append_pair("key", api_key)
            .append_pair("delete_logs", "false");
        let response = self.http.delete(url).send().await?;
        if response.status().is_success() {
            return Ok(());
        }
        Err(upstream_response_error(response, "API key rollback").await)
    }

    pub async fn usage_chart_data(&self, days: u16) -> AppResult<CliRelayUsageChartData> {
        let mut url = self.url(USAGE_CHART_DATA_PATH);
        url.query_pairs_mut().append_pair("days", &days.to_string());

        let response = self.http.get(url).send().await?;

        if response.status().is_success() {
            return Ok(response.json().await?);
        }

        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        Err(AppError::Upstream(format!(
            "CliRelay usage query failed with {status}: {text}"
        )))
    }

    fn url(&self, path: &str) -> Url {
        self.base_url
            .join(path)
            .expect("management API path must be relative")
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct CliRelayApiKeyEntry {
    pub id: String,
    pub key: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub disabled: bool,
}

#[derive(Debug, Deserialize)]
struct ApiKeyEntriesResponse {
    #[serde(rename = "api-key-entries", default)]
    api_key_entries: Vec<CliRelayApiKeyEntry>,
}

#[derive(Debug, Deserialize)]
pub struct CliRelayUsageChartData {
    #[serde(default)]
    pub apikey_distribution: Vec<CliRelayApiKeyDistribution>,
}

#[derive(Debug, Deserialize)]
pub struct CliRelayApiKeyDistribution {
    pub api_key: String,
    #[serde(default)]
    pub requests: i64,
    #[serde(default)]
    pub tokens: i64,
}

#[derive(Debug, Serialize)]
struct ApiKeyPatch<'a> {
    #[serde(rename = "old")]
    old_key: &'a str,
    #[serde(rename = "new")]
    new_key: &'a str,
}

#[derive(Debug, Serialize)]
struct ApiKeyEntryMetadataPatch<'a> {
    #[serde(rename = "match")]
    match_key: &'a str,
    value: ApiKeyEntryMetadataValue<'a>,
}

#[derive(Debug, Serialize)]
struct ApiKeyEntryMetadataValue<'a> {
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

#[derive(Debug, Serialize)]
struct ApiKeyEntryKeyPatch<'a> {
    id: &'a str,
    value: ApiKeyEntryKeyValue<'a>,
}

#[derive(Debug, Serialize)]
struct ApiKeyEntryKeyValue<'a> {
    key: &'a str,
}

async fn upstream_response_error(response: reqwest::Response, operation: &str) -> AppError {
    let status = response.status();
    let text = response.text().await.unwrap_or_default();
    AppError::Upstream(format!("CliRelay {operation} failed with {status}: {text}"))
}
