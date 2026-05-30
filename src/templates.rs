use askama::Template;

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
