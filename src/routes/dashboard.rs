use anyhow::Context;
use axum::{Router, extract::State, response::Response, routing::get};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};
use tower_sessions::Session;

use crate::{
    error::{AppError, AppResult},
    services::user::{current_user, decrypt_api_key},
    state::AppState,
    templates::{DashboardTemplate, LoginTemplate, render},
};

pub fn router() -> Router<AppState> {
    Router::new().route("/", get(index))
}

async fn index(State(state): State<AppState>, session: Session) -> AppResult<Response> {
    let Some(user) = current_user(&state, &session).await? else {
        return render(LoginTemplate {
            site_name: state.config.server.site_name.as_ref(),
        });
    };

    let api_key = decrypt_api_key(&state, &user)?;
    let created_at = format_unix_timestamp(user.created_at)?;
    let api_base_url = state.config.clirelay.public_base_url.as_str();
    let last_login_at = format_unix_timestamp(user.last_login_at)?;

    render(DashboardTemplate {
        site_name: state.config.server.site_name.as_ref(),
        github_login: &user.github_login,
        github_name: &user.github_name,
        github_email: &user.github_email,
        github_id: user.github_id,
        avatar_url: &user.avatar_url,
        api_key: &api_key,
        api_base_url,
        created_at: &created_at,
        last_login_at: &last_login_at,
    })
}

fn format_unix_timestamp(timestamp: i64) -> AppResult<String> {
    OffsetDateTime::from_unix_timestamp(timestamp)
        .context("stored user timestamp is outside supported range")?
        .format(&Rfc3339)
        .context("format user timestamp")
        .map_err(AppError::from)
}
