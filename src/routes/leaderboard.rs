use axum::{
    Router,
    extract::{Query, State},
    response::Response,
    routing::get,
};

use super::auth_guard::RequiredUser;
use crate::{
    error::AppResult,
    services::leaderboard::{LeaderboardQuery, load_leaderboard},
    state::AppState,
    templates::{LeaderboardTableTemplate, LeaderboardTemplate, render},
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/gate/leaderboard", get(page))
        .route("/gate/leaderboard/table", get(table))
}

async fn page(
    State(state): State<AppState>,
    RequiredUser(_user): RequiredUser,
) -> AppResult<Response> {
    let query = LeaderboardQuery::default();
    let view = load_leaderboard(&state, query.period, query.metric).await?;

    render(LeaderboardTemplate {
        site_name: state.config.server.site_name.as_ref(),
        view,
    })
}

async fn table(
    State(state): State<AppState>,
    RequiredUser(_user): RequiredUser,
    Query(query): Query<LeaderboardQuery>,
) -> AppResult<Response> {
    let view = load_leaderboard(&state, query.period, query.metric).await?;

    render(LeaderboardTableTemplate { view })
}
