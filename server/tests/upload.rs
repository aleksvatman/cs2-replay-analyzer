use std::path::Path;

use axum::body::{Body, Bytes};
use axum::http::{Request, StatusCode};
use futures_util::{stream, StreamExt};
use http_body_util::BodyExt;
use server::config::Config;
use server::state::AppState;
use server::storage::{Status, Storage};
use sha2::{Digest, Sha256};
use tower::ServiceExt;

fn state(data_dir: &Path, max_upload: u64) -> AppState {
    let dir = data_dir.to_str().unwrap().to_owned();
    let max = max_upload.to_string();
    let config = Config::from_lookup(|name| match name {
        "DATA_DIR" => Some(dir.clone()),
        "MAX_UPLOAD_BYTES" => Some(max.clone()),
        _ => None,
    })
    .unwrap();
    AppState::new(config).unwrap()
}

fn demo(extra: &[u8]) -> Vec<u8> {
    let mut bytes = b"PBDEMS2\0".to_vec();
    bytes.extend_from_slice(extra);
    bytes
}

async fn send(state: &AppState, request: Request<Body>) -> (StatusCode, serde_json::Value) {
    let response = server::router(state.clone())
        .oneshot(request)
        .await
        .unwrap();
    let status = response.status();
    let body = response.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&body).unwrap_or(serde_json::Value::Null),
    )
}

fn post(body: Vec<u8>) -> Request<Body> {
    Request::post("/matches")
        .header("content-type", "application/octet-stream")
        .body(Body::from(body))
        .unwrap()
}

fn match_dirs(root: &Path) -> Vec<String> {
    std::fs::read_dir(root)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect()
}

#[tokio::test]
async fn stores_a_demo_and_creates_a_queued_match() {
    let data = tempfile::tempdir().unwrap();
    let state = state(data.path(), 1 << 20);
    let bytes = demo(b"payload");
    let (status, body) = send(&state, post(bytes.clone())).await;

    assert_eq!(status, StatusCode::ACCEPTED);
    assert_eq!(body["status"], "queued");
    let id = body["id"].as_str().unwrap();

    let meta = state.storage.read_meta(id).unwrap().unwrap();
    assert_eq!(meta.status, Status::Queued);
    assert_eq!(meta.size_bytes, bytes.len() as u64);
    assert_eq!(
        meta.sha256.as_deref(),
        Some(format!("{:x}", Sha256::digest(&bytes)).as_str())
    );
    assert_eq!(
        std::fs::read(state.storage.dem_path(id).unwrap()).unwrap(),
        bytes
    );

    let mut files: Vec<_> = std::fs::read_dir(state.storage.dir(id).unwrap())
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    files.sort();
    assert_eq!(files, vec!["match.dem", "meta.json"]);
}

#[tokio::test]
async fn accepts_a_request_without_content_type_and_keeps_the_file_name() {
    let data = tempfile::tempdir().unwrap();
    let state = state(data.path(), 1 << 20);
    let request = Request::post("/matches")
        .header("x-file-name", "navi-vs-falcons.dem")
        .body(Body::from(demo(b"a")))
        .unwrap();
    let (status, body) = send(&state, request).await;
    assert_eq!(status, StatusCode::ACCEPTED);
    let meta = state
        .storage
        .read_meta(body["id"].as_str().unwrap())
        .unwrap()
        .unwrap();
    assert_eq!(meta.original_name.as_deref(), Some("navi-vs-falcons.dem"));
}

#[tokio::test]
async fn the_same_demo_twice_returns_the_same_match() {
    let data = tempfile::tempdir().unwrap();
    let state = state(data.path(), 1 << 20);
    let (first_status, first) = send(&state, post(demo(b"same"))).await;
    let (second_status, second) = send(&state, post(demo(b"same"))).await;

    assert_eq!(first_status, StatusCode::ACCEPTED);
    assert_eq!(second_status, StatusCode::OK);
    assert_eq!(first["id"], second["id"]);
    assert_eq!(match_dirs(data.path()).len(), 1, "no copy is left behind");

    // a different demo is a different match
    let (status, third) = send(&state, post(demo(b"other"))).await;
    assert_eq!(status, StatusCode::ACCEPTED);
    assert_ne!(first["id"], third["id"]);
}

#[tokio::test]
async fn a_failed_match_does_not_block_a_new_upload() {
    let data = tempfile::tempdir().unwrap();
    let state = state(data.path(), 1 << 20);
    let (_, first) = send(&state, post(demo(b"retry"))).await;
    let id = first["id"].as_str().unwrap();
    let mut meta = state.storage.read_meta(id).unwrap().unwrap();
    meta.status = Status::Failed;
    state.storage.write_meta(&meta).unwrap();

    let (status, second) = send(&state, post(demo(b"retry"))).await;
    assert_eq!(status, StatusCode::ACCEPTED);
    assert_ne!(second["id"], first["id"]);
}

#[tokio::test]
async fn concurrent_identical_uploads_make_one_match() {
    let data = tempfile::tempdir().unwrap();
    let state = state(data.path(), 1 << 20);
    let bytes = demo(&[7u8; 4096]);
    let (a, b, c) = tokio::join!(
        send(&state, post(bytes.clone())),
        send(&state, post(bytes.clone())),
        send(&state, post(bytes.clone())),
    );
    let accepted = [&a, &b, &c]
        .iter()
        .filter(|r| r.0 == StatusCode::ACCEPTED)
        .count();
    assert_eq!(accepted, 1);
    assert_eq!(match_dirs(data.path()).len(), 1);
}

#[tokio::test]
async fn rejects_files_that_are_not_demos_and_leaves_nothing() {
    let data = tempfile::tempdir().unwrap();
    let state = state(data.path(), 1 << 20);
    for body in [
        Vec::new(),
        b"PBDEMS".to_vec(),
        b"hello world, not a demo".to_vec(),
    ] {
        let (status, json) = send(&state, post(body)).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(json["error"]["code"], "unprocessable");
    }
    assert!(match_dirs(data.path()).is_empty());
}

#[tokio::test]
async fn a_wrong_signature_is_rejected_before_the_rest_is_read() {
    let data = tempfile::tempdir().unwrap();
    let state = state(data.path(), 1 << 20);
    // the stream fails after the first chunk: the signature error must come first
    let chunks: Vec<Result<Bytes, std::io::Error>> = vec![
        Ok(Bytes::from_static(b"NOTADEMO!")),
        Err(std::io::Error::other("never read")),
    ];
    let request = Request::post("/matches")
        .body(Body::from_stream(stream::iter(chunks)))
        .unwrap();
    let (status, _) = send(&state, request).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(match_dirs(data.path()).is_empty());
}

#[tokio::test]
async fn the_signature_may_arrive_in_pieces() {
    let data = tempfile::tempdir().unwrap();
    let state = state(data.path(), 1 << 20);
    let chunks: Vec<Result<Bytes, std::io::Error>> = vec![
        Ok(Bytes::from_static(b"PBD")),
        Ok(Bytes::from_static(b"EMS")),
        Ok(Bytes::from_static(b"2\0rest")),
    ];
    let request = Request::post("/matches")
        .body(Body::from_stream(stream::iter(chunks)))
        .unwrap();
    let (status, _) = send(&state, request).await;
    assert_eq!(status, StatusCode::ACCEPTED);
}

#[tokio::test]
async fn rejects_too_large_uploads_by_content_length() {
    let data = tempfile::tempdir().unwrap();
    let state = state(data.path(), 100);
    let (status, json) = send(&state, post(demo(&[0u8; 200]))).await;
    assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
    assert_eq!(json["error"]["code"], "payload_too_large");
    assert!(match_dirs(data.path()).is_empty());
}

#[tokio::test]
async fn rejects_too_large_uploads_without_content_length() {
    let data = tempfile::tempdir().unwrap();
    let state = state(data.path(), 100);
    // chunked body: the size is only known while reading
    let chunks: Vec<Result<Bytes, std::io::Error>> = (0..5)
        .map(|i| {
            Ok(Bytes::from(if i == 0 {
                demo(&[0u8; 52])
            } else {
                vec![0u8; 60]
            }))
        })
        .collect();
    let request = Request::post("/matches")
        .body(Body::from_stream(stream::iter(chunks)))
        .unwrap();
    let (status, _) = send(&state, request).await;
    assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
    assert!(
        match_dirs(data.path()).is_empty(),
        "the partial file is removed"
    );
}

#[tokio::test]
async fn an_interrupted_upload_leaves_nothing() {
    let data = tempfile::tempdir().unwrap();
    let state = state(data.path(), 1 << 20);
    let chunks: Vec<Result<Bytes, std::io::Error>> = vec![
        Ok(Bytes::from(demo(b"first half"))),
        Err(std::io::Error::other("connection reset")),
    ];
    let request = Request::post("/matches")
        .body(Body::from_stream(stream::iter(chunks)))
        .unwrap();
    let (status, json) = send(&state, request).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(json["error"]["code"], "bad_request");
    assert!(match_dirs(data.path()).is_empty());
}

#[tokio::test]
async fn a_dropped_request_leaves_nothing() {
    let data = tempfile::tempdir().unwrap();
    let state = state(data.path(), 1 << 20);
    // a body that never finishes: the client "disconnects" when the future is dropped
    let pending = stream::iter(vec![Ok::<_, std::io::Error>(Bytes::from(demo(b"x")))])
        .chain(stream::pending());
    let request = Request::post("/matches")
        .body(Body::from_stream(pending))
        .unwrap();
    let app = server::router(state.clone());
    let result =
        tokio::time::timeout(std::time::Duration::from_millis(300), app.oneshot(request)).await;
    assert!(result.is_err(), "the upload is still waiting for data");
    assert!(
        match_dirs(data.path()).is_empty(),
        "the guard removed the directory"
    );
}

#[tokio::test]
async fn rejects_other_content_types() {
    let data = tempfile::tempdir().unwrap();
    let state = state(data.path(), 1 << 20);
    let request = Request::post("/matches")
        .header("content-type", "application/x-www-form-urlencoded")
        .body(Body::from(demo(b"x")))
        .unwrap();
    let (status, json) = send(&state, request).await;
    assert_eq!(status, StatusCode::UNSUPPORTED_MEDIA_TYPE);
    assert_eq!(json["error"]["code"], "unsupported_media_type");
    assert!(match_dirs(data.path()).is_empty());
}

#[tokio::test]
async fn ids_are_valid_uuids_for_the_storage() {
    let data = tempfile::tempdir().unwrap();
    let state = state(data.path(), 1 << 20);
    let (_, body) = send(&state, post(demo(b"id"))).await;
    assert!(state.storage.dir(body["id"].as_str().unwrap()).is_ok());
    let _ = Storage::new_id();
}
