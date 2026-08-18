use std::sync::Arc;

use sqlx::SqlitePool;

use crate::{
    clients::{cpa::CPAClient, github::GitHubOAuthClient},
    config::AppConfig,
    crypto::Crypto,
};

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<AppConfig>,
    pub crypto: Arc<Crypto>,
    pub db: SqlitePool,
    pub github: Arc<GitHubOAuthClient>,
    pub cpa: Arc<CPAClient>,
}
