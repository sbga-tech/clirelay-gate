use askama::Template;
use axum::response::{Html, IntoResponse, Response};

use crate::{error::AppResult, services::leaderboard::LeaderboardView};

#[derive(Debug, Template)]
#[template(path = "login.html")]
pub struct LoginTemplate<'a> {
    pub site_name: &'a str,
}

#[derive(Debug, Template)]
#[template(path = "dashboard.html")]
pub struct DashboardTemplate<'a> {
    pub site_name: &'a str,
    pub github_login: &'a str,
    pub github_name: &'a str,
    pub github_email: &'a str,
    pub github_id: i64,
    pub avatar_url: &'a str,
    pub api_key: &'a str,
    pub api_base_url: &'a str,
    pub created_at: &'a str,
    pub last_login_at: &'a str,
}

#[derive(Debug, Template)]
#[template(path = "leaderboard.html")]
pub struct LeaderboardTemplate<'a> {
    pub site_name: &'a str,
}

#[derive(Debug, Template)]
#[template(path = "leaderboard_table.html")]
pub struct LeaderboardTableTemplate {
    pub view: LeaderboardView,
}

pub(crate) fn render<T: Template>(template: T) -> AppResult<Response> {
    Ok(Html(template.render()?).into_response())
}
