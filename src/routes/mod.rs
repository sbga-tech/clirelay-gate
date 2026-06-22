use axum::{Router, response::Redirect, routing::get};

use crate::state::AppState;

mod auth;
mod auth_guard;
mod dashboard;
mod leaderboard;

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/gate", get(Redirect::to("/")))
        .merge(dashboard::router())
        .merge(auth::router())
        .merge(leaderboard::router())
        .with_state(state)
}
