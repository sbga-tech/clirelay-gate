use serde::Deserialize;

use super::KeeperClient;
use crate::error::AppResult;

const CPA_API_KEY_SETTINGS_PATH: &str = "usage/api-keys/settings";

#[derive(Debug, Deserialize)]
pub struct CPAAPIKeySettingsResponse {
    pub items: Vec<CPAAPIKeySettingsItem>,
}

#[derive(Debug, Deserialize)]
pub struct CPAAPIKeySettingsItem {
    pub id: String,
    #[serde(rename = "apiKey")]
    pub api_key: String,
}

impl KeeperClient {
    pub async fn cpa_api_key_settings(&self) -> AppResult<CPAAPIKeySettingsResponse> {
        self.get_json(
            CPA_API_KEY_SETTINGS_PATH,
            &[],
            "CPA API key settings request",
        )
        .await
    }
}
