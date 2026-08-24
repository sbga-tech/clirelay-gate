use axum::{
    Json,
    extract::FromRequestParts,
    http::{
        HeaderValue, StatusCode,
        header::{AUTHORIZATION, CACHE_CONTROL, WWW_AUTHENTICATE},
        request::Parts,
    },
    response::{IntoResponse, Response},
};
use secrecy::ExposeSecret;
use serde::Serialize;
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

use crate::{
    config::AdminApiConfig,
    error::{AppError, AppResult},
    state::AppState,
};

const MIN_ADMIN_TOKEN_BYTES: usize = 32;

pub struct AdminApiAuth {
    token_hash: [u8; 32],
}

impl AdminApiAuth {
    pub fn new(config: &AdminApiConfig) -> AppResult<Self> {
        let token = config.token.expose_secret();
        if token.len() < MIN_ADMIN_TOKEN_BYTES {
            return Err(AppError::Config(format!(
                "admin API token must contain at least {MIN_ADMIN_TOKEN_BYTES} bytes"
            )));
        }
        Ok(Self {
            token_hash: hash_token(token),
        })
    }

    fn verify(&self, candidate: &str) -> bool {
        self.token_hash.ct_eq(&hash_token(candidate)).into()
    }
}

pub struct AdminApiAuthenticated;

impl FromRequestParts<AppState> for AdminApiAuthenticated {
    type Rejection = AdminApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let candidate = parts
            .headers
            .get(AUTHORIZATION)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.strip_prefix("Bearer "))
            .filter(|value| !value.is_empty())
            .ok_or_else(AdminApiError::unauthorized)?;

        if !state.admin_api_auth.verify(candidate) {
            return Err(AdminApiError::unauthorized());
        }
        Ok(Self)
    }
}

pub struct AdminApiError {
    status: StatusCode,
    code: &'static str,
    message: &'static str,
    authenticate: bool,
}

impl AdminApiError {
    pub fn unauthorized() -> Self {
        Self {
            status: StatusCode::UNAUTHORIZED,
            code: "unauthorized",
            message: "A valid Bearer token is required.",
            authenticate: true,
        }
    }

    pub fn bad_request(code: &'static str, message: &'static str) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            code,
            message,
            authenticate: false,
        }
    }

    pub fn internal(
        code: &'static str,
        message: &'static str,
        operation: &'static str,
        error: impl std::fmt::Display,
    ) -> Self {
        tracing::error!(operation, error = %error, "admin API request failed");
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            code,
            message,
            authenticate: false,
        }
    }

    pub fn upstream(
        code: &'static str,
        message: &'static str,
        operation: &'static str,
        error: impl std::fmt::Display,
    ) -> Self {
        tracing::error!(operation, error = %error, "admin API upstream request failed");
        Self {
            status: StatusCode::BAD_GATEWAY,
            code,
            message,
            authenticate: false,
        }
    }
}

#[derive(Serialize)]
struct AdminApiErrorEnvelope {
    error: AdminApiErrorBody,
}

#[derive(Serialize)]
struct AdminApiErrorBody {
    code: &'static str,
    message: &'static str,
}

impl IntoResponse for AdminApiError {
    fn into_response(self) -> Response {
        let mut response = (
            self.status,
            Json(AdminApiErrorEnvelope {
                error: AdminApiErrorBody {
                    code: self.code,
                    message: self.message,
                },
            }),
        )
            .into_response();
        response
            .headers_mut()
            .insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
        if self.authenticate {
            response
                .headers_mut()
                .insert(WWW_AUTHENTICATE, HeaderValue::from_static("Bearer"));
        }
        response
    }
}

fn hash_token(token: &str) -> [u8; 32] {
    Sha256::digest(token.as_bytes()).into()
}
