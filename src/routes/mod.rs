use axum::Router;

use crate::state::AppState;

mod auth;
mod dashboard;
mod ranking;

pub fn router(state: AppState) -> Router {
    Router::new()
        .merge(dashboard::router())
        .merge(auth::router())
        .merge(ranking::router())
        .with_state(state)
}
