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
    /// The request clashes with the state of the match (for example deleting one being parsed).
    Conflict(String),
    /// The match exists but its data is not ready yet.
    NotReady,
    /// The client stopped sending the body.
    RequestTimeout,
    PayloadTooLarge,
    UnsupportedMediaType,
    /// Not a CS2 demo, or the parser failed.
    Unprocessable(String),
    /// The parse queue is full.
    QueueFull,
    /// Too many requests or uploads at once; try again after `retry_after` seconds.
    TooManyRequests {
        message: String,
        retry_after: u64,
    },
    /// The server already holds `MAX_MATCHES` matches.
    StorageFull,
    /// Details go to the log, the client gets a generic message.
    Internal(String),
}

impl ApiError {
    pub fn status(&self) -> StatusCode {
        match self {
            ApiError::BadRequest(_) => StatusCode::BAD_REQUEST,
            ApiError::Unauthorized => StatusCode::UNAUTHORIZED,
            ApiError::NotFound => StatusCode::NOT_FOUND,
            ApiError::Conflict(_) | ApiError::NotReady => StatusCode::CONFLICT,
            ApiError::RequestTimeout => StatusCode::REQUEST_TIMEOUT,
            ApiError::PayloadTooLarge => StatusCode::PAYLOAD_TOO_LARGE,
            ApiError::UnsupportedMediaType => StatusCode::UNSUPPORTED_MEDIA_TYPE,
            ApiError::Unprocessable(_) => StatusCode::UNPROCESSABLE_ENTITY,
            ApiError::QueueFull => StatusCode::SERVICE_UNAVAILABLE,
            ApiError::TooManyRequests { .. } => StatusCode::TOO_MANY_REQUESTS,
            ApiError::StorageFull => StatusCode::INSUFFICIENT_STORAGE,
            ApiError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    pub fn code(&self) -> &'static str {
        match self {
            ApiError::BadRequest(_) => "bad_request",
            ApiError::Unauthorized => "unauthorized",
            ApiError::NotFound => "not_found",
            ApiError::Conflict(_) => "conflict",
            ApiError::NotReady => "not_ready",
            ApiError::RequestTimeout => "request_timeout",
            ApiError::PayloadTooLarge => "payload_too_large",
            ApiError::UnsupportedMediaType => "unsupported_media_type",
            ApiError::Unprocessable(_) => "unprocessable",
            ApiError::QueueFull => "queue_full",
            ApiError::TooManyRequests { .. } => "too_many_requests",
            ApiError::StorageFull => "storage_full",
            ApiError::Internal(_) => "internal",
        }
    }

    fn message(&self) -> String {
        match self {
            ApiError::BadRequest(m) | ApiError::Conflict(m) | ApiError::Unprocessable(m) => m.clone(),
            ApiError::Unauthorized => "missing or invalid token".into(),
            ApiError::NotFound => "not found".into(),
            ApiError::NotReady => "match data is not ready yet".into(),
            ApiError::RequestTimeout => "the upload stalled".into(),
            ApiError::PayloadTooLarge => "file is larger than the upload limit".into(),
            ApiError::UnsupportedMediaType => {
                "send the file as the raw request body (Content-Type: application/octet-stream), not as multipart, JSON or text".into()
            }
            ApiError::QueueFull => "the parse queue is full, try again later".into(),
            ApiError::TooManyRequests { message, .. } => message.clone(),
            ApiError::StorageFull => {
                "the server holds the maximum number of matches, delete some first".into()
            }
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
        match &self {
            ApiError::TooManyRequests { retry_after, .. } => {
                response
                    .headers_mut()
                    .insert(header::RETRY_AFTER, HeaderValue::from(*retry_after));
            }
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
            (ApiError::Conflict("x".into()), 409, "conflict"),
            (ApiError::NotReady, 409, "not_ready"),
            (ApiError::RequestTimeout, 408, "request_timeout"),
            (ApiError::PayloadTooLarge, 413, "payload_too_large"),
            (
                ApiError::UnsupportedMediaType,
                415,
                "unsupported_media_type",
            ),
            (ApiError::Unprocessable("x".into()), 422, "unprocessable"),
            (ApiError::Internal("x".into()), 500, "internal"),
            (ApiError::QueueFull, 503, "queue_full"),
            (
                ApiError::TooManyRequests {
                    message: "x".into(),
                    retry_after: 1,
                },
                429,
                "too_many_requests",
            ),
            (ApiError::StorageFull, 507, "storage_full"),
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
