use axum::{
    extract::FromRequestParts,
    http::{HeaderName, HeaderValue, StatusCode, request::Parts},
    response::{IntoResponse, Redirect, Response},
};
use tower_sessions::Session;

use crate::{db::User, services::user::current_user, state::AppState};

pub struct RequiredUser(pub User);

impl FromRequestParts<AppState> for RequiredUser {
    type Rejection = Response;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let session = Session::from_request_parts(parts, state)
            .await
            .map_err(IntoResponse::into_response)?;

        match current_user(state, &session).await {
            Ok(Some(user)) => Ok(Self(user)),
            Ok(None) => Err(unauthenticated_response(parts)),
            Err(error) => Err(error.into_response()),
        }
    }
}

fn unauthenticated_response(parts: &Parts) -> Response {
    if is_htmx_request(parts) {
        let mut response = StatusCode::OK.into_response();
        response.headers_mut().insert(
            HeaderName::from_static("hx-redirect"),
            HeaderValue::from_static("/"),
        );
        return response;
    }

    Redirect::to("/").into_response()
}

fn is_htmx_request(parts: &Parts) -> bool {
    parts
        .headers
        .get("hx-request")
        .and_then(|value| value.to_str().ok())
        == Some("true")
}
