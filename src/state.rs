use std::sync::Arc;

use sqlx::SqlitePool;

use crate::{
    clirelay::CliRelayClient, config::AppConfig, crypto::Crypto, github::GitHubOAuthClient,
};

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<AppConfig>,
    pub crypto: Arc<Crypto>,
    pub db: SqlitePool,
    pub github: Arc<GitHubOAuthClient>,
    pub clirelay: Arc<CliRelayClient>,
}
