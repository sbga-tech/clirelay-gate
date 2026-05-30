use anyhow::Context;
use askama::Template;
use axum::{
    Router,
    extract::{Query, State},
    response::{Html, IntoResponse, Redirect, Response},
    routing::{get, post},
};
use serde::Deserialize;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};
use tower_sessions::Session;

use crate::{
    config::NonEmptyString,
    crypto,
    db::{self, GitHubProfileUpdate, NewUser, User},
    error::{AppError, AppResult},
    github,
    state::AppState,
    templates::{DashboardTemplate, LoginTemplate},
};

const SESSION_USER_ID: &str = "user_id";
const OAUTH_STATE_TTL_SECONDS: i64 = 600;

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/", get(index))
        .route("/auth/github/start", get(github_start))
        .route("/auth/github/callback", get(github_callback))
        .route("/auth/logout", post(logout))
        .with_state(state)
}

async fn index(State(state): State<AppState>, session: Session) -> AppResult<Response> {
    let Some(user_id) = session.get::<i64>(SESSION_USER_ID).await? else {
        return render(LoginTemplate {
            site_name: state.config.server.site_name.as_ref(),
        });
    };

    let Some(user) = db::get_user_by_id(&state.db, user_id).await? else {
        session.flush().await?;
        return render(LoginTemplate {
            site_name: state.config.server.site_name.as_ref(),
        });
    };

    let api_key = state
        .crypto
        .decrypt(&user.api_key_ciphertext, &user.api_key_nonce)?;
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

async fn github_start(State(state): State<AppState>) -> AppResult<Redirect> {
    db::delete_expired_oauth_states(&state.db).await?;
    let start = state.github.build_oauth_start()?;
    db::insert_oauth_state(
        &state.db,
        &start.state,
        &start.pkce_verifier,
        OAUTH_STATE_TTL_SECONDS,
    )
    .await?;
    Ok(Redirect::to(&start.redirect_url))
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum GitHubCallbackQuery {
    Success {
        code: NonEmptyString,
        state: NonEmptyString,
    },
    Error {
        error: NonEmptyString,
        error_description: Option<String>,
    },
}

async fn github_callback(
    State(state): State<AppState>,
    session: Session,
    Query(query): Query<GitHubCallbackQuery>,
) -> AppResult<Redirect> {
    let (code, oauth_state) = match query {
        GitHubCallbackQuery::Success { code, state } => (code.into_inner(), state.into_inner()),
        GitHubCallbackQuery::Error {
            error,
            error_description,
        } => {
            return Err(AppError::BadRequest(
                error_description.unwrap_or_else(|| error.into_inner()),
            ));
        },
    };

    let Some(pkce_verifier) = db::consume_oauth_state(&state.db, &oauth_state).await? else {
        return Err(AppError::BadRequest(
            "invalid or expired OAuth state".into(),
        ));
    };

    let access_token = state.github.exchange_code(&code, &pkce_verifier).await?;
    let identity = state.github.fetch_identity(&access_token).await?;
    let user = find_or_create_user(&state, identity).await?;

    session.insert(SESSION_USER_ID, user.id).await?;
    Ok(Redirect::to("/"))
}

async fn logout(session: Session) -> AppResult<Redirect> {
    session.flush().await?;
    Ok(Redirect::to("/"))
}

async fn find_or_create_user(
    state: &AppState,
    identity: github::GitHubIdentity,
) -> AppResult<User> {
    if db::get_user_by_github_id(&state.db, identity.id)
        .await?
        .is_some()
    {
        let update = GitHubProfileUpdate {
            github_id: identity.id,
            github_login: identity.login,
            github_name: identity.name,
            github_email: identity.email,
            avatar_url: identity.avatar_url,
        };
        return Ok(db::update_existing_user(&state.db, &update).await?);
    }

    let api_key = crypto::generate_api_key(state.config.security.api_key_prefix.as_ref())?;
    let key_name = format!("github:{}#{}", identity.login, identity.id);

    state
        .clirelay
        .provision_api_key(&api_key, &key_name)
        .await?;

    let encrypted = state.crypto.encrypt(&api_key)?;
    let new_user = NewUser {
        github_id: identity.id,
        github_login: identity.login,
        github_name: identity.name,
        github_email: identity.email,
        avatar_url: identity.avatar_url,
        api_key_ciphertext: encrypted.ciphertext,
        api_key_nonce: encrypted.nonce,
    };

    match db::insert_user(&state.db, new_user).await {
        Ok(user) => Ok(user),
        Err(sqlx::Error::Database(err)) if err.is_unique_violation() => {
            db::get_user_by_github_id(&state.db, identity.id)
                .await?
                .ok_or(sqlx::Error::RowNotFound)
                .map_err(AppError::from)
        },
        Err(err) => Err(AppError::from(err)),
    }
}

fn format_unix_timestamp(timestamp: i64) -> AppResult<String> {
    OffsetDateTime::from_unix_timestamp(timestamp)
        .context("stored user timestamp is outside supported range")?
        .format(&Rfc3339)
        .context("format user timestamp")
        .map_err(AppError::from)
}

fn render<T: Template>(template: T) -> AppResult<Response> {
    Ok(Html(template.render()?).into_response())
}
