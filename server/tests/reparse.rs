//! `POST /matches/{id}/reparse`, with a parser stand-in that counts its runs.

use std::io::Write;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use flate2::write::GzEncoder;
use flate2::Compression;
use http_body_util::BodyExt;
use server::config::Config;
use server::jobs::{Parsed, Runner};
use server::state::AppState;
use tower::ServiceExt;

/// Controls the stand-in parser from the test.
#[derive(Clone)]
struct Knobs {
    runs: Arc<AtomicUsize>,
    fail: Arc<AtomicBool>,
    /// While `false`, a run waits.
    gate: Arc<AtomicBool>,
}

impl Knobs {
    fn new() -> Knobs {
        Knobs {
            runs: Arc::new(AtomicUsize::new(0)),
            fail: Arc::new(AtomicBool::new(false)),
            gate: Arc::new(AtomicBool::new(true)),
        }
    }

    /// Writes `{"run":N}` as the result of the N-th run, or fails while `fail` is set.
    fn runner(&self) -> Runner {
        let k = self.clone();
        Arc::new(move |_dem, json, _report| {
            let run = k.runs.fetch_add(1, Ordering::SeqCst) + 1;
            while !k.gate.load(Ordering::SeqCst) {
                std::thread::sleep(Duration::from_millis(5));
            }
            if k.fail.load(Ordering::SeqCst) {
                return Err(format!("parser crashed on run {run}"));
            }
            let mut encoder = GzEncoder::new(Vec::new(), Compression::fast());
            encoder
                .write_all(format!("{{\"run\":{run}}}").as_bytes())
                .unwrap();
            std::fs::write(json, encoder.finish().unwrap()).unwrap();
            Ok(Parsed {
                map: "de_test".into(),
                schema_version: 1,
                rounds: 3,
                score: Some([2, 1]),
            })
        })
    }
}

fn state(dir: &Path, knobs: &Knobs, vars: &[(&str, &str)]) -> AppState {
    let data = dir.to_str().unwrap().to_owned();
    let vars: Vec<(String, String)> = vars
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    let config = Config::from_lookup(|name| {
        if name == "DATA_DIR" {
            return Some(data.clone());
        }
        if name == "KEEP_DEM" && !vars.iter().any(|(k, _)| k == "KEEP_DEM") {
            return Some("true".into());
        }
        vars.iter().find(|(k, _)| k == name).map(|(_, v)| v.clone())
    })
    .unwrap();
    AppState::with_runner(config, knobs.runner()).unwrap()
}

async fn call(state: &AppState, request: Request<Body>) -> (StatusCode, Vec<u8>) {
    let response = server::router(state.clone())
        .oneshot(request)
        .await
        .unwrap();
    let status = response.status();
    (
        status,
        response
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes()
            .to_vec(),
    )
}

async fn json(state: &AppState, request: Request<Body>) -> (StatusCode, serde_json::Value) {
    let (status, bytes) = call(state, request).await;
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null),
    )
}

fn get(path: &str) -> Request<Body> {
    Request::get(path).body(Body::empty()).unwrap()
}

fn reparse(id: &str) -> Request<Body> {
    Request::post(format!("/matches/{id}/reparse"))
        .body(Body::empty())
        .unwrap()
}

async fn upload(state: &AppState, content: &[u8]) -> String {
    let mut bytes = b"PBDEMS2\0".to_vec();
    bytes.extend_from_slice(content);
    let request = Request::post("/matches").body(Body::from(bytes)).unwrap();
    let (status, body) = json(state, request).await;
    assert_eq!(status, StatusCode::ACCEPTED);
    body["id"].as_str().unwrap().to_owned()
}

/// Waits until the jobs are idle and returns the detail of the match.
async fn settled(state: &AppState, id: &str) -> serde_json::Value {
    for _ in 0..500 {
        let (_, detail) = json(state, get(&format!("/matches/{id}"))).await;
        if matches!(detail["status"].as_str(), Some("done" | "failed"))
            && state.jobs.in_flight() == 0
        {
            return detail;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("match {id} did not settle");
}

async fn result(state: &AppState, id: &str) -> String {
    let (status, bytes) = call(state, get(&format!("/matches/{id}/data"))).await;
    assert_eq!(status, StatusCode::OK);
    String::from_utf8(bytes).unwrap()
}

#[tokio::test]
async fn a_finished_match_is_parsed_again_and_the_result_is_replaced() {
    let data = tempfile::tempdir().unwrap();
    let knobs = Knobs::new();
    let state = state(data.path(), &knobs, &[]);
    let id = upload(&state, b"again").await;
    settled(&state, &id).await;
    assert_eq!(result(&state, &id).await, "{\"run\":1}");

    let (status, body) = json(&state, reparse(&id)).await;
    assert_eq!(status, StatusCode::ACCEPTED);
    assert_eq!(body["id"], id.as_str());
    assert_eq!(body["status"], "queued");

    let detail = settled(&state, &id).await;
    assert_eq!(detail["status"], "done");
    assert_eq!(detail["error"], serde_json::Value::Null);
    assert_eq!(result(&state, &id).await, "{\"run\":2}");
    assert!(
        state.storage.dem_path(&id).unwrap().is_file(),
        "the demo is still there"
    );
}

#[tokio::test]
async fn a_failed_reparse_keeps_the_previous_result() {
    let data = tempfile::tempdir().unwrap();
    let knobs = Knobs::new();
    let state = state(data.path(), &knobs, &[]);
    let id = upload(&state, b"keep").await;
    settled(&state, &id).await;

    knobs.fail.store(true, Ordering::SeqCst);
    assert_eq!(json(&state, reparse(&id)).await.0, StatusCode::ACCEPTED);
    let detail = settled(&state, &id).await;
    assert_eq!(detail["status"], "done", "the match is still usable");
    let error = detail["error"].as_str().unwrap();
    assert!(
        error.contains("re-parse failed") && error.contains("parser crashed on run 2"),
        "{error}"
    );
    assert_eq!(result(&state, &id).await, "{\"run\":1}");

    // the next successful run clears the error
    knobs.fail.store(false, Ordering::SeqCst);
    assert_eq!(json(&state, reparse(&id)).await.0, StatusCode::ACCEPTED);
    let detail = settled(&state, &id).await;
    assert_eq!(detail["error"], serde_json::Value::Null);
    assert_eq!(result(&state, &id).await, "{\"run\":3}");
}

#[tokio::test]
async fn a_failed_match_can_be_tried_again() {
    let data = tempfile::tempdir().unwrap();
    let knobs = Knobs::new();
    knobs.fail.store(true, Ordering::SeqCst);
    let state = state(data.path(), &knobs, &[]);
    let id = upload(&state, b"retry").await;
    assert_eq!(settled(&state, &id).await["status"], "failed");

    knobs.fail.store(false, Ordering::SeqCst);
    assert_eq!(json(&state, reparse(&id)).await.0, StatusCode::ACCEPTED);
    let detail = settled(&state, &id).await;
    assert_eq!(detail["status"], "done");
    assert_eq!(detail["rounds"], 3);
    assert_eq!(result(&state, &id).await, "{\"run\":2}");
}

#[tokio::test]
async fn a_match_that_is_waiting_or_being_parsed_cannot_be_parsed_again() {
    let data = tempfile::tempdir().unwrap();
    let knobs = Knobs::new();
    knobs.gate.store(false, Ordering::SeqCst);
    let state = state(data.path(), &knobs, &[]);
    let id = upload(&state, b"busy").await;
    for _ in 0..200 {
        if json(&state, get(&format!("/matches/{id}"))).await.1["status"] == "parsing" {
            break;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    let (status, body) = json(&state, reparse(&id)).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(body["error"]["code"], "conflict");
    knobs.gate.store(true, Ordering::SeqCst);
    settled(&state, &id).await;
    assert_eq!(
        knobs.runs.load(Ordering::SeqCst),
        1,
        "no second run was started"
    );
}

#[tokio::test]
async fn without_a_kept_demo_there_is_nothing_to_parse_again() {
    let data = tempfile::tempdir().unwrap();
    let knobs = Knobs::new();
    let state = state(data.path(), &knobs, &[("KEEP_DEM", "false")]);
    let id = upload(&state, b"gone").await;
    settled(&state, &id).await;
    let (status, body) = json(&state, reparse(&id)).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert!(body["error"]["message"]
        .as_str()
        .unwrap()
        .contains("KEEP_DEM"));
    assert_eq!(
        result(&state, &id).await,
        "{\"run\":1}",
        "the result is untouched"
    );
}

#[tokio::test]
async fn unknown_matches_are_404() {
    let data = tempfile::tempdir().unwrap();
    let state = state(data.path(), &Knobs::new(), &[]);
    for id in ["6f9619ff-8b86-d011-b42d-00c04fc964ff", "not-a-uuid"] {
        assert_eq!(json(&state, reparse(id)).await.0, StatusCode::NOT_FOUND);
    }
}

#[tokio::test]
async fn a_full_queue_refuses_the_reparse_and_changes_nothing() {
    let data = tempfile::tempdir().unwrap();
    let knobs = Knobs::new();
    // room for one job at a time and nothing waiting
    let state = state(data.path(), &knobs, &[("MAX_QUEUE", "0")]);
    let finished = upload(&state, b"finished").await;
    settled(&state, &finished).await;

    knobs.gate.store(false, Ordering::SeqCst);
    let busy = upload(&state, b"busy").await;
    for _ in 0..200 {
        if json(&state, get(&format!("/matches/{busy}"))).await.1["status"] == "parsing" {
            break;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }

    let (status, body) = json(&state, reparse(&finished)).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(body["error"]["code"], "queue_full");
    let (_, detail) = json(&state, get(&format!("/matches/{finished}"))).await;
    assert_eq!(detail["status"], "done", "restored after the refusal");
    assert_eq!(result(&state, &finished).await, "{\"run\":1}");

    knobs.gate.store(true, Ordering::SeqCst);
    settled(&state, &busy).await;
}

#[tokio::test]
async fn the_list_shows_the_schema_version() {
    let data = tempfile::tempdir().unwrap();
    let state = state(data.path(), &Knobs::new(), &[]);
    let id = upload(&state, b"schema").await;
    settled(&state, &id).await;
    let (_, list) = json(&state, get("/matches")).await;
    assert_eq!(list[0]["schemaVersion"], 1);
    let (_, detail) = json(&state, get(&format!("/matches/{id}"))).await;
    assert_eq!(detail["schemaVersion"], 1);
}
