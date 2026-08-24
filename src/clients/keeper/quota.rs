use serde::Serialize;
use serde_json::Value;

use super::KeeperClient;
use crate::error::AppResult;

const QUOTA_CACHE_PATH: &str = "quota/cache";
const AUTO_REFRESH_SETTINGS_PATH: &str = "quota/auto-refresh/settings";
const VERSION_PATH: &str = "version";

#[derive(Debug, Serialize)]
struct QuotaCacheRequest<'a> {
    auth_indexes: &'a [String],
}

impl KeeperClient {
    pub async fn quota_cache(&self, auth_indexes: &[String]) -> AppResult<Value> {
        self.post_json(
            QUOTA_CACHE_PATH,
            &QuotaCacheRequest { auth_indexes },
            "quota cache request",
        )
        .await
    }

    pub async fn quota_auto_refresh_settings(&self) -> AppResult<Value> {
        self.get_json(
            AUTO_REFRESH_SETTINGS_PATH,
            &[],
            "quota auto-refresh settings request",
        )
        .await
    }

    pub async fn version(&self) -> AppResult<Value> {
        self.get_json(VERSION_PATH, &[], "version request").await
    }
}
