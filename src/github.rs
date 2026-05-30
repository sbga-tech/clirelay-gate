use oauth2::{CsrfToken, PkceCodeChallenge};
use octocrab::{Octocrab, OctocrabBuilder};
use reqwest::{Client as HttpClient, header::ACCEPT};
use secrecy::ExposeSecret;
use serde::Deserialize;
use url::Url;

use crate::{
    config::GitHubConfig,
    error::{AppError, AppResult},
    http,
};

const AUTHORIZE_URL: &str = "https://github.com/login/oauth/authorize";
const TOKEN_URL: &str = "https://github.com/login/oauth/access_token";
const OAUTH_SCOPE: &str = "read:user user:email";

#[derive(Clone)]
pub struct GitHubOAuthClient {
    http: HttpClient,
    client_id: String,
    client_secret: String,
    callback_url: String,
}

impl GitHubOAuthClient {
    pub fn new(config: &GitHubConfig, callback_url: String) -> AppResult<Self> {
        Ok(Self {
            http: http::client_builder().build()?,
            client_id: config.client_id.as_ref().to_owned(),
            client_secret: config.client_secret.expose_secret().to_owned(),
            callback_url,
        })
    }

    pub fn build_oauth_start(&self) -> AppResult<OAuthStart> {
        let state = CsrfToken::new_random();
        let (challenge, verifier) = PkceCodeChallenge::new_random_sha256();

        let mut url = Url::parse(AUTHORIZE_URL)?;
        url.query_pairs_mut()
            .append_pair("client_id", &self.client_id)
            .append_pair("redirect_uri", &self.callback_url)
            .append_pair("scope", OAUTH_SCOPE)
            .append_pair("state", state.secret())
            .append_pair("code_challenge", challenge.as_str())
            .append_pair("code_challenge_method", "S256");

        Ok(OAuthStart {
            redirect_url: url.to_string(),
            state: state.secret().to_owned(),
            pkce_verifier: verifier.secret().to_owned(),
        })
    }

    pub async fn exchange_code(&self, code: &str, pkce_verifier: &str) -> AppResult<String> {
        let response = self
            .http
            .post(TOKEN_URL)
            .header(ACCEPT, "application/json")
            .form(&[
                ("client_id", self.client_id.as_str()),
                ("client_secret", self.client_secret.as_str()),
                ("code", code),
                ("redirect_uri", self.callback_url.as_str()),
                ("code_verifier", pkce_verifier),
            ])
            .send()
            .await?;

        let status = response.status();
        let payload = response.json::<TokenResponse>().await?;
        if !status.is_success() {
            let message = payload
                .error_description
                .or(payload.error)
                .unwrap_or_else(|| format!("GitHub token exchange failed with {status}"));
            return Err(AppError::Upstream(message));
        }

        payload
            .access_token
            .filter(|token| !token.trim().is_empty())
            .ok_or_else(|| {
                AppError::Upstream("GitHub token response did not include access_token".into())
            })
    }

    pub async fn fetch_identity(&self, access_token: &str) -> AppResult<GitHubIdentity> {
        let github = OctocrabBuilder::new()
            .user_access_token(access_token.to_owned())
            .build()
            .map_err(|err| AppError::Upstream(format!("build GitHub client: {err}")))?;

        fetch_identity_with_octocrab(&github).await
    }
}

#[derive(Debug, Clone)]
pub struct OAuthStart {
    pub redirect_url: String,
    pub state: String,
    pub pkce_verifier: String,
}

#[derive(Debug, Clone)]
pub struct GitHubIdentity {
    pub id: i64,
    pub login: String,
    pub name: String,
    pub email: String,
    pub avatar_url: String,
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: Option<String>,
    error: Option<String>,
    error_description: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GitHubEmailResponse {
    email: String,
    primary: bool,
    verified: bool,
}

async fn fetch_identity_with_octocrab(github: &Octocrab) -> AppResult<GitHubIdentity> {
    let user = github.current().user().await.map_err(github_error)?;
    let emails = github
        .get::<Vec<GitHubEmailResponse>, _, _>("/user/emails", None::<&()>)
        .await
        .map_err(github_error)?;

    let id = i64::try_from(user.id.into_inner())
        .map_err(|_| AppError::Upstream("GitHub user id is too large".into()))?;
    let email = select_email(&emails).or(user.email).unwrap_or_default();

    Ok(GitHubIdentity {
        id,
        login: user.login,
        name: user.name.unwrap_or_default(),
        email,
        avatar_url: user.avatar_url.to_string(),
    })
}

fn select_email(emails: &[GitHubEmailResponse]) -> Option<String> {
    emails
        .iter()
        .find(|email| email.primary && email.verified)
        .or_else(|| emails.iter().find(|email| email.verified))
        .or_else(|| emails.first())
        .map(|email| email.email.clone())
}

fn github_error(error: octocrab::Error) -> AppError {
    AppError::Upstream(format!("GitHub API error: {error}"))
}
