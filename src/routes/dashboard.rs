use anyhow::Context;
use axum::{
    Router,
    extract::{Form, State},
    response::{Redirect, Response},
    routing::{get, post},
};
use serde::Deserialize;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};
use tower_sessions::Session;

use super::auth_guard::RequiredUser;
use crate::{
    crypto,
    error::{AppError, AppResult},
    services::user::{
        create_api_key, current_user, decrypt_api_key, delete_api_key, reconcile_api_key,
        rotate_api_key,
    },
    state::AppState,
    templates::{DashboardTemplate, LoginTemplate, render},
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(index))
        .route("/gate/api-key", post(update_api_key))
}

async fn index(State(state): State<AppState>, session: Session) -> AppResult<Response> {
    let Some(user) = current_user(&state, &session).await? else {
        return render(LoginTemplate {
            site_name: state.config.server.site_name.as_ref(),
        });
    };

    let api_key = decrypt_api_key(&state, &user)?.unwrap_or_default();
    let created_at = format_unix_timestamp(user.created_at)?;
    let api_base_url = state.config.clirelay.public_base_url.as_str();
    let last_login_at = format_unix_timestamp(user.last_login_at)?;
    let api_key_hash = if api_key.is_empty() {
        String::new()
    } else {
        crypto::hash_secret(&api_key)
    };

    render(DashboardTemplate {
        site_name: state.config.server.site_name.as_ref(),
        github_login: &user.github_login,
        github_name: &user.github_name,
        github_email: &user.github_email,
        github_id: user.github_id,
        avatar_url: &user.avatar_url,
        api_key: &api_key,
        api_key_hash: &api_key_hash,
        api_base_url,
        created_at: &created_at,
        last_login_at: &last_login_at,
    })
}

#[derive(Debug, Deserialize)]
struct ApiKeyActionForm {
    action: ApiKeyAction,
    #[serde(default)]
    expected_key_hash: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
enum ApiKeyAction {
    Create,
    Rotate,
    Delete,
}

async fn update_api_key(
    State(state): State<AppState>,
    RequiredUser(user): RequiredUser,
    Form(form): Form<ApiKeyActionForm>,
) -> AppResult<Redirect> {
    let _mutation_guard = state.api_key_mutations.lock().await;
    let user = reconcile_api_key(&state, user).await?;

    match form.action {
        ApiKeyAction::Create => {
            create_api_key(&state, user).await?;
        },
        ApiKeyAction::Rotate => {
            rotate_api_key(&state, user, &form.expected_key_hash).await?;
        },
        ApiKeyAction::Delete => {
            if decrypt_api_key(&state, &user)?.is_some() {
                delete_api_key(&state, user, &form.expected_key_hash).await?;
            }
        },
    }

    Ok(Redirect::to("/"))
}

fn format_unix_timestamp(timestamp: i64) -> AppResult<String> {
    OffsetDateTime::from_unix_timestamp(timestamp)
        .context("stored user timestamp is outside supported range")?
        .format(&Rfc3339)
        .context("format user timestamp")
        .map_err(AppError::from)
}
