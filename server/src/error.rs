//! One error type for the whole API. Every error becomes
//! `{"error": {"code": "...", "message": "..."}}` (see `server-doc.md`, section 5).

use axum::http::{header, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;

#[derive(Debug)]
pub enum ApiError {
    BadRequest(String),
    Unauthorized,
    NotFound,
    /// The match exists but its data is not ready yet.
    NotReady,
    PayloadTooLarge,
    UnsupportedMediaType,
    /// Not a CS2 demo, or the parser failed.
    Unprocessable(String),
    /// The parse queue is full.
    QueueFull,
    /// Details go to the log, the client gets a generic message.
    Internal(String),
}

impl ApiError {
    pub fn status(&self) -> StatusCode {
        match self {
            ApiError::BadRequest(_) => StatusCode::BAD_REQUEST,
            ApiError::Unauthorized => StatusCode::UNAUTHORIZED,
            ApiError::NotFound => StatusCode::NOT_FOUND,
            ApiError::NotReady => StatusCode::CONFLICT,
            ApiError::PayloadTooLarge => StatusCode::PAYLOAD_TOO_LARGE,
            ApiError::UnsupportedMediaType => StatusCode::UNSUPPORTED_MEDIA_TYPE,
            ApiError::Unprocessable(_) => StatusCode::UNPROCESSABLE_ENTITY,
            ApiError::QueueFull => StatusCode::SERVICE_UNAVAILABLE,
            ApiError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    pub fn code(&self) -> &'static str {
        match self {
            ApiError::BadRequest(_) => "bad_request",
            ApiError::Unauthorized => "unauthorized",
            ApiError::NotFound => "not_found",
            ApiError::NotReady => "not_ready",
            ApiError::PayloadTooLarge => "payload_too_large",
            ApiError::UnsupportedMediaType => "unsupported_media_type",
            ApiError::Unprocessable(_) => "unprocessable",
            ApiError::QueueFull => "queue_full",
            ApiError::Internal(_) => "internal",
        }
    }

    fn message(&self) -> String {
        match self {
            ApiError::BadRequest(m) | ApiError::Unprocessable(m) => m.clone(),
            ApiError::Unauthorized => "missing or invalid token".into(),
            ApiError::NotFound => "not found".into(),
            ApiError::NotReady => "match data is not ready yet".into(),
            ApiError::PayloadTooLarge => "file is larger than the upload limit".into(),
            ApiError::UnsupportedMediaType => "unsupported content type".into(),
            ApiError::QueueFull => "the parse queue is full, try again later".into(),
            ApiError::Internal(_) => "internal server error".into(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        if let ApiError::Internal(details) = &self {
            tracing::error!(%details, "internal error");
        }
        let body = Json(json!({ "error": { "code": self.code(), "message": self.message() } }));
        let mut response = (self.status(), body).into_response();
        match self {
            ApiError::QueueFull => {
                response
                    .headers_mut()
                    .insert(header::RETRY_AFTER, HeaderValue::from_static("30"));
            }
            ApiError::Unauthorized => {
                response
                    .headers_mut()
                    .insert(header::WWW_AUTHENTICATE, HeaderValue::from_static("Bearer"));
            }
            _ => {}
        }
        response
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn statuses_and_codes_follow_the_spec() {
        let table = [
            (ApiError::BadRequest("x".into()), 400, "bad_request"),
            (ApiError::Unauthorized, 401, "unauthorized"),
            (ApiError::NotFound, 404, "not_found"),
            (ApiError::NotReady, 409, "not_ready"),
            (ApiError::PayloadTooLarge, 413, "payload_too_large"),
            (
                ApiError::UnsupportedMediaType,
                415,
                "unsupported_media_type",
            ),
            (ApiError::Unprocessable("x".into()), 422, "unprocessable"),
            (ApiError::Internal("x".into()), 500, "internal"),
            (ApiError::QueueFull, 503, "queue_full"),
        ];
        for (err, status, code) in table {
            assert_eq!(err.status().as_u16(), status);
            assert_eq!(err.code(), code);
        }
    }

    #[test]
    fn internal_details_are_not_exposed() {
        assert_eq!(
            ApiError::Internal("disk path /secret".into()).message(),
            "internal server error"
        );
    }
}
