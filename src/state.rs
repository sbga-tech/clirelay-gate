use std::sync::Arc;

use sqlx::SqlitePool;

use crate::{
    admin_api::AdminApiAuth,
    clients::{cpa::CPAClient, github::GitHubOAuthClient, keeper::KeeperClient},
    config::AppConfig,
    crypto::Crypto,
};

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<AppConfig>,
    pub admin_api_auth: Arc<AdminApiAuth>,
    pub crypto: Arc<Crypto>,
    pub db: SqlitePool,
    pub github: Arc<GitHubOAuthClient>,
    pub cpa: Arc<CPAClient>,
    pub keeper: Arc<KeeperClient>,
}
