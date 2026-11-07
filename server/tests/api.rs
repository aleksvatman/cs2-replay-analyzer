use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use server::config::Config;
use server::state::AppState;
use tower::ServiceExt;

fn app(data_dir: &std::path::Path) -> axum::Router {
    let dir = data_dir.to_str().unwrap().to_owned();
    let config = Config::from_lookup(|name| (name == "DATA_DIR").then(|| dir.clone())).unwrap();
    server::router(AppState::new(config).unwrap())
}

async fn get(path: &str) -> (StatusCode, String, Option<String>) {
    let data = tempfile::tempdir().unwrap();
    let response = app(data.path())
        .oneshot(Request::get(path).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let content_type = response
        .headers()
        .get("content-type")
        .map(|v| v.to_str().unwrap().to_string());
    let body = response.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        String::from_utf8(body.to_vec()).unwrap(),
        content_type,
    )
}

#[tokio::test]
async fn health_is_ok() {
    let (status, body, _) = get("/health").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, "ok");
}

#[tokio::test]
async fn unknown_route_returns_a_json_error() {
    let (status, body, content_type) = get("/nope").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(content_type.as_deref(), Some("application/json"));
    let json: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert_eq!(json["error"]["code"], "not_found");
    assert!(json["error"]["message"].is_string());
}
