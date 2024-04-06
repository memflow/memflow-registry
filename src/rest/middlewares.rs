use axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::Response,
};
use axum_extra::{
    headers::{authorization::Bearer, Authorization},
    TypedHeader,
};
use log::warn;

#[derive(Clone)]
pub struct AuthorizationToken {
    token: Option<String>,
}

impl AuthorizationToken {
    pub fn new(token: Option<String>) -> Self {
        Self { token }
    }
}

pub async fn check_token(
    State(auth_token): State<AuthorizationToken>,
    TypedHeader(authorization): TypedHeader<Authorization<Bearer>>,
    request: Request,
    next: Next,
) -> std::result::Result<Response, StatusCode> {
    if let Some(token) = auth_token.token {
        if authorization.0.token() != token {
            // token is set but it does not match
            warn!(
                "invalid token when uploading plugin: token={}",
                authorization.0.token()
            );
            return Err(StatusCode::UNAUTHORIZED);
        }
    }

    let response = next.run(request).await;
    Ok(response)
}
