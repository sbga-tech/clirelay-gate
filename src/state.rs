use std::sync::Arc;

use sqlx::SqlitePool;
use tokio::sync::Mutex;

use crate::{
    clients::{clirelay::CliRelayClient, github::GitHubOAuthClient},
    config::AppConfig,
    crypto::Crypto,
};

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<AppConfig>,
    pub crypto: Arc<Crypto>,
    pub db: SqlitePool,
    pub github: Arc<GitHubOAuthClient>,
    pub clirelay: Arc<CliRelayClient>,
    pub api_key_mutations: Arc<Mutex<()>>,
}
