//! Token check (`server-doc.md`, section 11). With `API_TOKEN` set, every request except
//! `GET /health` needs `Authorization: Bearer <token>`. Without a token (allowed on loopback
//! only, see `config.rs`) the server is open.

use axum::extract::{Request, State};
use axum::http::{header, HeaderMap};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use sha2::{Digest, Sha256};

use crate::error::ApiError;
use crate::state::AppState;

pub const PUBLIC_PATH: &str = "/health";

pub async fn require_token(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Response {
    if let Some(token) = &state.config.api_token {
        if request.uri().path() != PUBLIC_PATH && !authorized(request.headers(), token) {
            return ApiError::Unauthorized.into_response();
        }
    }
    next.run(request).await
}

/// `Authorization: Bearer <token>` with the right token. The scheme name is case-insensitive.
pub fn authorized(headers: &HeaderMap, token: &str) -> bool {
    let Some(value) = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
    else {
        return false;
    };
    let Some((scheme, given)) = value.split_once(' ') else {
        return false;
    };
    scheme.eq_ignore_ascii_case("bearer") && same_secret(given.trim(), token)
}

/// Compares two secrets without stopping at the first difference. Both are hashed first,
/// so the length of the real token does not leak either.
fn same_secret(a: &str, b: &str) -> bool {
    let (a, b) = (Sha256::digest(a.as_bytes()), Sha256::digest(b.as_bytes()));
    a.iter()
        .zip(b.iter())
        .fold(0u8, |diff, (x, y)| diff | (x ^ y))
        == 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    fn with(value: &str) -> HeaderMap {
        let mut map = HeaderMap::new();
        map.insert(header::AUTHORIZATION, HeaderValue::from_str(value).unwrap());
        map
    }

    #[test]
    fn accepts_only_the_right_bearer_token() {
        let token = "0123456789abcdef";
        assert!(authorized(&with("Bearer 0123456789abcdef"), token));
        assert!(authorized(&with("bearer 0123456789abcdef"), token));
        assert!(authorized(&with("BEARER   0123456789abcdef  "), token));
        assert!(!authorized(&HeaderMap::new(), token));
        assert!(!authorized(&with("Bearer wrong"), token));
        assert!(!authorized(&with("Bearer 0123456789abcde"), token));
        assert!(!authorized(&with("Bearer 0123456789abcdefX"), token));
        assert!(!authorized(&with("Bearer "), token));
        assert!(!authorized(&with("Basic 0123456789abcdef"), token));
        assert!(!authorized(&with("0123456789abcdef"), token));
    }
}
