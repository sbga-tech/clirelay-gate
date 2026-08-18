use axum::Router;

use crate::state::AppState;

mod auth;
mod dashboard;

pub fn router(state: AppState) -> Router {
    Router::new()
        .merge(dashboard::router())
        .merge(auth::router())
        .with_state(state)
}
