use axum::{
    Router,
    extract::{Query, State},
    response::Redirect,
    routing::{get, post},
};
use serde::Deserialize;
use tower_sessions::Session;

use crate::{
    config::NonEmptyString,
    db,
    error::{AppError, AppResult},
    services::user::{find_or_create_user, remember_current_user},
    state::AppState,
};

const OAUTH_STATE_TTL_SECONDS: i64 = 600;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/gate/auth/github/start", get(github_start))
        .route("/gate/auth/github/callback", get(github_callback))
        .route("/gate/auth/logout", post(logout))
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

    remember_current_user(&session, user.id).await?;
    Ok(Redirect::to("/"))
}

async fn logout(session: Session) -> AppResult<Redirect> {
    session.flush().await?;
    Ok(Redirect::to("/"))
}
