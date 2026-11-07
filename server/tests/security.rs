//! Token, rate limit, parallel upload limit and match count limit.

use std::io::Write;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use axum::body::{Body, Bytes};
use axum::http::{Request, Response, StatusCode};
use flate2::write::GzEncoder;
use flate2::Compression;
use futures_util::{stream, StreamExt};
use http_body_util::BodyExt;
use server::config::Config;
use server::jobs::{Parsed, Runner};
use server::state::AppState;
use tower::ServiceExt;

const TOKEN: &str = "0123456789abcdef0123";

/// Finishes at once and writes a tiny result.
fn quick_runner() -> Runner {
    Arc::new(|_dem, json, _report| {
        let mut encoder = GzEncoder::new(Vec::new(), Compression::fast());
        encoder.write_all(b"{}").unwrap();
        std::fs::write(json, encoder.finish().unwrap()).unwrap();
        Ok(Parsed {
            map: "de_test".into(),
            ..Parsed::default()
        })
    })
}

fn state(dir: &Path, vars: &[(&str, &str)]) -> AppState {
    let data = dir.to_str().unwrap().to_owned();
    let vars: Vec<(String, String)> = vars
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    let config = Config::from_lookup(|name| {
        if name == "DATA_DIR" {
            return Some(data.clone());
        }
        vars.iter().find(|(k, _)| k == name).map(|(_, v)| v.clone())
    })
    .unwrap();
    AppState::with_runner(config, quick_runner()).unwrap()
}

async fn call(state: &AppState, request: Request<Body>) -> Response<Body> {
    server::router(state.clone())
        .oneshot(request)
        .await
        .unwrap()
}

async fn json(response: Response<Body>) -> serde_json::Value {
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null)
}

fn get(path: &str, token: Option<&str>) -> Request<Body> {
    let mut builder = Request::get(path);
    if let Some(token) = token {
        builder = builder.header("authorization", format!("Bearer {token}"));
    }
    builder.body(Body::empty()).unwrap()
}

fn upload(content: &[u8], token: Option<&str>) -> Request<Body> {
    let mut bytes = b"PBDEMS2\0".to_vec();
    bytes.extend_from_slice(content);
    let mut builder = Request::post("/matches");
    if let Some(token) = token {
        builder = builder.header("authorization", format!("Bearer {token}"));
    }
    builder.body(Body::from(bytes)).unwrap()
}

#[tokio::test]
async fn without_a_configured_token_the_server_is_open() {
    let data = tempfile::tempdir().unwrap();
    let state = state(data.path(), &[]);
    assert_eq!(
        call(&state, get("/matches", None)).await.status(),
        StatusCode::OK
    );
}

#[tokio::test]
async fn with_a_token_every_route_but_health_needs_it() {
    let data = tempfile::tempdir().unwrap();
    let state = state(data.path(), &[("API_TOKEN", TOKEN)]);
    let id = "6f9619ff-8b86-d011-b42d-00c04fc964ff";

    // health is public
    let response = call(&state, get("/health", None)).await;
    assert_eq!(response.status(), StatusCode::OK);

    for path in [
        "/matches",
        &format!("/matches/{id}"),
        &format!("/matches/{id}/data"),
        "/nope",
    ] {
        for token in [None, Some("wrong-token-wrong-token")] {
            let response = call(&state, get(path, token)).await;
            assert_eq!(
                response.status(),
                StatusCode::UNAUTHORIZED,
                "{path} {token:?}"
            );
            assert_eq!(response.headers()["www-authenticate"], "Bearer");
            assert_eq!(json(response).await["error"]["code"], "unauthorized");
        }
    }
    let delete = Request::delete(format!("/matches/{id}"))
        .body(Body::empty())
        .unwrap();
    assert_eq!(
        call(&state, delete).await.status(),
        StatusCode::UNAUTHORIZED
    );

    // the right token gets through; a missing match is a plain 404 then
    assert_eq!(
        call(&state, get("/matches", Some(TOKEN))).await.status(),
        StatusCode::OK
    );
    assert_eq!(
        call(&state, get(&format!("/matches/{id}"), Some(TOKEN)))
            .await
            .status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        call(&state, get("/nope", Some(TOKEN))).await.status(),
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn an_upload_without_the_token_is_refused_before_its_body_is_read() {
    let data = tempfile::tempdir().unwrap();
    let state = state(data.path(), &[("API_TOKEN", TOKEN)]);

    // reading this body would fail with 400; the 401 must come first
    let chunks: Vec<Result<Bytes, std::io::Error>> = vec![Err(std::io::Error::other("never read"))];
    let request = Request::post("/matches")
        .body(Body::from_stream(stream::iter(chunks)))
        .unwrap();
    assert_eq!(
        call(&state, request).await.status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(std::fs::read_dir(data.path()).unwrap().count(), 0);

    let response = call(&state, upload(b"ok", Some(TOKEN))).await;
    assert_eq!(response.status(), StatusCode::ACCEPTED);
}

#[tokio::test]
async fn requests_over_the_rate_limit_get_429_and_health_is_exempt() {
    let data = tempfile::tempdir().unwrap();
    // 60 per minute: a burst of 10, then one more per second
    let state = state(
        data.path(),
        &[("API_TOKEN", TOKEN), ("RATE_LIMIT_PER_MINUTE", "60")],
    );
    let mut ok = 0;
    let mut limited = None;
    for _ in 0..13 {
        let response = call(&state, get("/matches", Some(TOKEN))).await;
        match response.status() {
            StatusCode::OK => ok += 1,
            StatusCode::TOO_MANY_REQUESTS => {
                limited = Some(response);
            }
            other => panic!("unexpected {other}"),
        }
    }
    assert!((10..=11).contains(&ok), "{ok} requests passed");
    let limited = limited.expect("some request was limited");
    let retry: u64 = limited.headers()["retry-after"]
        .to_str()
        .unwrap()
        .parse()
        .unwrap();
    assert!(retry >= 1);
    assert_eq!(json(limited).await["error"]["code"], "too_many_requests");

    assert_eq!(
        call(&state, get("/health", None)).await.status(),
        StatusCode::OK
    );
}

#[tokio::test]
async fn strangers_cannot_use_up_the_rate_limit() {
    let data = tempfile::tempdir().unwrap();
    let state = state(
        data.path(),
        &[("API_TOKEN", TOKEN), ("RATE_LIMIT_PER_MINUTE", "60")],
    );
    for _ in 0..50 {
        let response = call(&state, get("/matches", Some("not-the-token-not-the-token"))).await;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }
    for _ in 0..10 {
        assert_eq!(
            call(&state, get("/matches", Some(TOKEN))).await.status(),
            StatusCode::OK
        );
    }
}

#[tokio::test]
async fn rate_limit_zero_means_no_limit() {
    let data = tempfile::tempdir().unwrap();
    let state = state(data.path(), &[("RATE_LIMIT_PER_MINUTE", "0")]);
    for _ in 0..100 {
        assert_eq!(
            call(&state, get("/matches", None)).await.status(),
            StatusCode::OK
        );
    }
}

#[tokio::test]
async fn only_max_uploads_bodies_are_received_at_once() {
    let data = tempfile::tempdir().unwrap();
    let state = state(data.path(), &[("MAX_UPLOADS", "1")]);

    // a client that starts an upload and then goes quiet
    let slow = stream::iter(vec![Ok::<_, std::io::Error>(Bytes::from_static(
        b"PBDEMS2\0x",
    ))])
    .chain(stream::pending());
    let request = Request::post("/matches")
        .body(Body::from_stream(slow))
        .unwrap();
    let app = server::router(state.clone());
    let first = tokio::spawn(async move { app.oneshot(request).await });
    tokio::time::sleep(Duration::from_millis(150)).await;

    let response = call(&state, upload(b"second", None)).await;
    assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    assert!(response.headers().contains_key("retry-after"));
    assert_eq!(json(response).await["error"]["code"], "too_many_requests");

    // when the first client goes away, the slot is free again
    first.abort();
    let _ = first.await;
    let response = call(&state, upload(b"third", None)).await;
    assert_eq!(response.status(), StatusCode::ACCEPTED);
}

#[tokio::test]
async fn the_server_holds_at_most_max_matches() {
    let data = tempfile::tempdir().unwrap();
    let state = state(data.path(), &[("MAX_MATCHES", "2")]);

    let mut ids = Vec::new();
    for content in [b"one".as_slice(), b"two"] {
        let response = call(&state, upload(content, None)).await;
        assert_eq!(response.status(), StatusCode::ACCEPTED);
        ids.push(json(response).await["id"].as_str().unwrap().to_owned());
    }
    let response = call(&state, upload(b"three", None)).await;
    assert_eq!(response.status(), StatusCode::INSUFFICIENT_STORAGE);
    assert_eq!(json(response).await["error"]["code"], "storage_full");
    assert_eq!(state.storage.count().unwrap(), 2, "nothing was left behind");

    // wait for the finished matches, then free a place
    for _ in 0..200 {
        let list = json(call(&state, get("/matches", None)).await).await;
        if list
            .as_array()
            .unwrap()
            .iter()
            .all(|m| m["status"] == "done")
        {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    let delete = Request::delete(format!("/matches/{}", ids[0]))
        .body(Body::empty())
        .unwrap();
    assert_eq!(call(&state, delete).await.status(), StatusCode::NO_CONTENT);
    let response = call(&state, upload(b"three", None)).await;
    assert_eq!(response.status(), StatusCode::ACCEPTED);
}
