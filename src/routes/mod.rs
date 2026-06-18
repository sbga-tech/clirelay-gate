use axum::{Router, response::Redirect, routing::get};

use crate::state::AppState;

mod auth;
mod dashboard;

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/gate", get(Redirect::to("/")))
        .merge(dashboard::router())
        .merge(auth::router())
        .with_state(state)
}
