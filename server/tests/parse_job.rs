//! End to end with the real parser: upload a demo, wait for the job, compare the result with
//! the reference JSON of `parser-core`. Skipped without `samples/test_demo.dem`.

use std::io::Read;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use axum::body::Body;
use axum::http::{Request, StatusCode};
use flate2::read::GzDecoder;
use http_body_util::BodyExt;
use server::config::Config;
use server::state::AppState;
use server::storage::Status;
use tower::ServiceExt;

fn samples() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../samples")
}

fn state(dir: &std::path::Path, extra: &[(&str, &str)]) -> AppState {
    let data = dir.to_str().unwrap().to_owned();
    let extra: Vec<(String, String)> = extra
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    let config = Config::from_lookup(|name| match name {
        "DATA_DIR" => Some(data.clone()),
        other => extra
            .iter()
            .find(|(k, _)| k == other)
            .map(|(_, v)| v.clone()),
    })
    .unwrap();
    AppState::new(config).unwrap()
}

async fn upload(state: &AppState, bytes: Vec<u8>) -> (StatusCode, serde_json::Value) {
    let request = Request::post("/matches")
        .header("content-type", "application/octet-stream")
        .body(Body::from(bytes))
        .unwrap();
    let response = server::router(state.clone())
        .oneshot(request)
        .await
        .unwrap();
    let status = response.status();
    let body = response.into_body().collect().await.unwrap().to_bytes();
    (status, serde_json::from_slice(&body).unwrap())
}

async fn wait_finished(state: &AppState, id: &str) -> Status {
    let deadline = Instant::now() + Duration::from_secs(120);
    loop {
        let status = state.storage.read_meta(id).unwrap().unwrap().status;
        if matches!(status, Status::Done | Status::Failed) {
            return status;
        }
        assert!(Instant::now() < deadline, "the job did not finish");
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

fn gunzip(bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    GzDecoder::new(bytes).read_to_end(&mut out).unwrap();
    out
}

#[tokio::test]
async fn a_real_demo_is_parsed_into_the_reference_json() {
    let demo = samples().join("test_demo.dem");
    let Ok(bytes) = std::fs::read(&demo) else {
        eprintln!("skipped: {} is missing", demo.display());
        return;
    };
    let data = tempfile::tempdir().unwrap();
    let state = state(data.path(), &[]);

    let (status, body) = upload(&state, bytes).await;
    assert_eq!(status, StatusCode::ACCEPTED);
    let id = body["id"].as_str().unwrap().to_owned();

    assert_eq!(wait_finished(&state, &id).await, Status::Done);
    let meta = state.storage.read_meta(&id).unwrap().unwrap();
    assert_eq!(meta.map.as_deref(), Some("de_mirage"));
    assert_eq!(meta.schema_version, Some(1));
    assert!(meta.parsed_at.is_some());
    assert!(
        !state.storage.dem_path(&id).unwrap().exists(),
        "demo removed after parsing"
    );

    let result = gunzip(&std::fs::read(state.storage.json_path(&id).unwrap()).unwrap());
    let reference = gunzip(&std::fs::read(samples().join("test_demo.expected.json.gz")).unwrap());
    assert!(
        result == reference,
        "the server output differs from the reference JSON"
    );

    // no temporary files are left behind
    let mut files: Vec<_> = std::fs::read_dir(state.storage.dir(&id).unwrap())
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    files.sort();
    assert_eq!(files, vec!["match.json.gz", "meta.json"]);
}

#[tokio::test]
async fn a_file_with_the_right_signature_but_no_demo_inside_fails_cleanly() {
    let data = tempfile::tempdir().unwrap();
    let state = state(data.path(), &[]);
    let mut bytes = b"PBDEMS2\0".to_vec();
    bytes.extend_from_slice(&[0xAB; 4096]);

    let (status, body) = upload(&state, bytes).await;
    assert_eq!(status, StatusCode::ACCEPTED);
    let id = body["id"].as_str().unwrap().to_owned();

    assert_eq!(wait_finished(&state, &id).await, Status::Failed);
    let meta = state.storage.read_meta(&id).unwrap().unwrap();
    assert!(meta.error.is_some_and(|e| !e.is_empty()));
    assert!(!state.storage.json_path(&id).unwrap().exists());
    // the server still takes the next upload
    let (status, _) = upload(&state, [b"PBDEMS2\0".as_slice(), &[1; 64]].concat()).await;
    assert_eq!(status, StatusCode::ACCEPTED);
}
