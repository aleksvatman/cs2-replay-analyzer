//! List, status, data and delete, with a stand-in for the parser so the tests are fast and
//! control exactly when a job is running.

use std::io::{Read, Write};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use axum::body::Body;
use axum::http::{Request, Response, StatusCode};
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use http_body_util::BodyExt;
use parser_core::{Progress, Stage};
use server::config::Config;
use server::jobs::{Parsed, Runner};
use server::state::AppState;
use server::storage::Status;
use tower::ServiceExt;

/// Writes `payload` gzipped as the result. Reports a stage, then waits for `gate` if given.
/// A demo whose bytes end in `bad` fails.
fn runner(payload: Vec<u8>, gate: Option<Arc<AtomicBool>>) -> Runner {
    Arc::new(move |dem, json, report| {
        report(Progress {
            stage: Stage::Players,
            percent: 30,
        });
        if let Some(gate) = &gate {
            while !gate.load(Ordering::SeqCst) {
                std::thread::sleep(Duration::from_millis(5));
            }
        }
        if std::fs::read(dem)
            .map_err(|e| e.to_string())?
            .ends_with(b"bad")
        {
            return Err("demo parse error: broken".into());
        }
        let mut encoder = GzEncoder::new(Vec::new(), Compression::fast());
        encoder.write_all(&payload).unwrap();
        std::fs::write(json, encoder.finish().unwrap()).unwrap();
        Ok(Parsed {
            map: "de_test".into(),
            schema_version: 1,
            rounds: 24,
            score: Some([11, 13]),
        })
    })
}

fn state(dir: &Path, runner: Runner) -> AppState {
    let data = dir.to_str().unwrap().to_owned();
    let config = Config::from_lookup(|name| (name == "DATA_DIR").then(|| data.clone())).unwrap();
    AppState::with_runner(config, runner).unwrap()
}

async fn call(state: &AppState, request: Request<Body>) -> Response<Body> {
    server::router(state.clone())
        .oneshot(request)
        .await
        .unwrap()
}

async fn body(response: Response<Body>) -> Vec<u8> {
    response
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes()
        .to_vec()
}

async fn json(response: Response<Body>) -> serde_json::Value {
    serde_json::from_slice(&body(response).await).unwrap()
}

async fn upload(state: &AppState, content: &[u8]) -> String {
    let mut bytes = b"PBDEMS2\0".to_vec();
    bytes.extend_from_slice(content);
    let request = Request::post("/matches").body(Body::from(bytes)).unwrap();
    let response = call(state, request).await;
    assert_eq!(response.status(), StatusCode::ACCEPTED);
    json(response).await["id"].as_str().unwrap().to_owned()
}

fn get(path: &str) -> Request<Body> {
    Request::get(path).body(Body::empty()).unwrap()
}

async fn wait_for(state: &AppState, id: &str, want: Status) {
    for _ in 0..500 {
        if state.storage.read_meta(id).unwrap().unwrap().status == want {
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("match {id} did not reach {want:?}");
}

fn gunzip(bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    GzDecoder::new(bytes).read_to_end(&mut out).unwrap();
    out
}

#[tokio::test]
async fn the_list_is_empty_at_first_and_newest_first_later() {
    let data = tempfile::tempdir().unwrap();
    let state = state(data.path(), runner(b"{}".to_vec(), None));
    let response = call(&state, get("/matches")).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(json(response).await, serde_json::json!([]));

    let first = upload(&state, b"one").await;
    tokio::time::sleep(Duration::from_millis(1100)).await; // uploadedAt has second resolution
    let second = upload(&state, b"two").await;
    wait_for(&state, &first, Status::Done).await;
    wait_for(&state, &second, Status::Done).await;

    let list = json(call(&state, get("/matches")).await).await;
    let ids: Vec<_> = list
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, vec![second.as_str(), first.as_str()]);
    let row = &list[0];
    assert_eq!(row["status"], "done");
    assert_eq!(row["map"], "de_test");
    assert_eq!(row["rounds"], 24);
    assert_eq!(row["score"], serde_json::json!([11, 13]));
    assert_eq!(row["sizeBytes"], 11);
    assert!(row["uploadedAt"].is_u64());
}

#[tokio::test]
async fn status_follows_the_job_from_parsing_to_done() {
    let data = tempfile::tempdir().unwrap();
    let gate = Arc::new(AtomicBool::new(false));
    let state = state(data.path(), runner(b"{}".to_vec(), Some(gate.clone())));
    let id = upload(&state, b"slow").await;

    wait_for(&state, &id, Status::Parsing).await;
    // the runner reports a stage right after it starts
    let mut detail = serde_json::Value::Null;
    for _ in 0..200 {
        detail = json(call(&state, get(&format!("/matches/{id}"))).await).await;
        if detail["percent"] == 30 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    assert_eq!(detail["status"], "parsing");
    assert_eq!(detail["stage"], "players");
    assert_eq!(detail["percent"], 30);
    assert_eq!(detail["error"], serde_json::Value::Null);

    gate.store(true, Ordering::SeqCst);
    wait_for(&state, &id, Status::Done).await;
    let detail = json(call(&state, get(&format!("/matches/{id}"))).await).await;
    assert_eq!(detail["status"], "done");
    assert_eq!(detail["stage"], "done");
    assert_eq!(detail["percent"], 100);
    assert_eq!(detail["map"], "de_test");
    assert_eq!(detail["schemaVersion"], 1);
    assert!(detail["parsedAt"].is_u64());
}

#[tokio::test]
async fn a_waiting_match_has_no_stage_and_a_failed_one_shows_the_error() {
    let data = tempfile::tempdir().unwrap();
    let gate = Arc::new(AtomicBool::new(false));
    let state = state(data.path(), runner(b"{}".to_vec(), Some(gate.clone())));
    let running = upload(&state, b"first").await;
    wait_for(&state, &running, Status::Parsing).await;
    let waiting = upload(&state, b"bad").await; // one parse at a time: this one waits

    let detail = json(call(&state, get(&format!("/matches/{waiting}"))).await).await;
    assert_eq!(detail["status"], "queued");
    assert_eq!(detail["stage"], serde_json::Value::Null);
    assert_eq!(detail["percent"], 0);

    gate.store(true, Ordering::SeqCst);
    wait_for(&state, &waiting, Status::Failed).await;
    let detail = json(call(&state, get(&format!("/matches/{waiting}"))).await).await;
    assert_eq!(detail["status"], "failed");
    assert_eq!(detail["error"], "demo parse error: broken");
    assert_eq!(detail["stage"], serde_json::Value::Null);
}

#[tokio::test]
async fn unknown_and_malformed_ids_are_404() {
    let data = tempfile::tempdir().unwrap();
    let state = state(data.path(), runner(b"{}".to_vec(), None));
    let unknown = "6f9619ff-8b86-d011-b42d-00c04fc964ff";
    for path in [
        format!("/matches/{unknown}"),
        format!("/matches/{unknown}/data"),
        "/matches/not-a-uuid".to_owned(),
        "/matches/not-a-uuid/data".to_owned(),
        "/matches/..%2F..%2Fetc".to_owned(),
    ] {
        let response = call(&state, get(&path)).await;
        assert_eq!(response.status(), StatusCode::NOT_FOUND, "{path}");
    }
    let delete = Request::delete(format!("/matches/{unknown}"))
        .body(Body::empty())
        .unwrap();
    assert_eq!(call(&state, delete).await.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn data_is_409_until_the_match_is_done() {
    let data = tempfile::tempdir().unwrap();
    let gate = Arc::new(AtomicBool::new(false));
    let state = state(data.path(), runner(b"{}".to_vec(), Some(gate.clone())));
    let id = upload(&state, b"x").await;
    let response = call(&state, get(&format!("/matches/{id}/data"))).await;
    assert_eq!(response.status(), StatusCode::CONFLICT);
    assert_eq!(json(response).await["error"]["code"], "not_ready");
    gate.store(true, Ordering::SeqCst);
    wait_for(&state, &id, Status::Done).await;

    let failed = upload(&state, b"bad").await;
    wait_for(&state, &failed, Status::Failed).await;
    let response = call(&state, get(&format!("/matches/{failed}/data"))).await;
    assert_eq!(response.status(), StatusCode::CONFLICT);
}

#[tokio::test]
async fn data_is_served_compressed_or_decompressed_depending_on_the_client() {
    let data = tempfile::tempdir().unwrap();
    // a few megabytes, so that both paths use many chunks
    let payload: Vec<u8> = (0..3_000_000u32)
        .flat_map(|i| format!("{},", i % 1000).into_bytes())
        .collect();
    let mut wrapped = b"[".to_vec();
    wrapped.extend_from_slice(&payload);
    wrapped.extend_from_slice(b"0]");
    let state = state(data.path(), runner(wrapped.clone(), None));
    let id = upload(&state, b"big").await;
    wait_for(&state, &id, Status::Done).await;
    let path = format!("/matches/{id}/data");
    let stored = std::fs::read(state.storage.json_path(&id).unwrap()).unwrap();

    // a client that accepts gzip gets the stored file as it is
    let request = Request::get(&path)
        .header("accept-encoding", "gzip")
        .body(Body::empty())
        .unwrap();
    let response = call(&state, request).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["content-encoding"], "gzip");
    assert_eq!(response.headers()["content-type"], "application/json");
    assert_eq!(response.headers()["vary"], "Accept-Encoding");
    assert_eq!(
        response.headers()["content-length"],
        stored.len().to_string().as_str()
    );
    let gzip_tag = response.headers()["etag"].to_str().unwrap().to_owned();
    assert_eq!(body(response).await, stored);

    // any other client gets plain JSON
    let response = call(&state, get(&path)).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert!(response.headers().get("content-encoding").is_none());
    assert_eq!(response.headers()["content-type"], "application/json");
    let plain_tag = response.headers()["etag"].to_str().unwrap().to_owned();
    assert_eq!(body(response).await, wrapped);
    assert_ne!(gzip_tag, plain_tag, "the two variants have different tags");

    // gzip;q=0 means no gzip
    let request = Request::get(&path)
        .header("accept-encoding", "gzip;q=0")
        .body(Body::empty())
        .unwrap();
    let response = call(&state, request).await;
    assert!(response.headers().get("content-encoding").is_none());
    assert_eq!(gunzip(&stored), wrapped);
}

#[tokio::test]
async fn etag_gives_304_and_changes_with_the_variant() {
    let data = tempfile::tempdir().unwrap();
    let state = state(data.path(), runner(b"{\"a\":1}".to_vec(), None));
    let id = upload(&state, b"tag").await;
    wait_for(&state, &id, Status::Done).await;
    let path = format!("/matches/{id}/data");

    let first = call(&state, get(&path)).await;
    let tag = first.headers()["etag"].to_str().unwrap().to_owned();
    assert_eq!(first.headers()["cache-control"], "private, no-cache");

    let same = Request::get(&path)
        .header("if-none-match", &tag)
        .body(Body::empty())
        .unwrap();
    let response = call(&state, same).await;
    assert_eq!(response.status(), StatusCode::NOT_MODIFIED);
    assert_eq!(response.headers()["etag"], tag.as_str());
    assert!(response.headers().get("content-type").is_none());
    assert!(body(response).await.is_empty());

    // the tag of the plain variant does not match the compressed one
    let other = Request::get(&path)
        .header("if-none-match", &tag)
        .header("accept-encoding", "gzip")
        .body(Body::empty())
        .unwrap();
    assert_eq!(call(&state, other).await.status(), StatusCode::OK);
}

#[tokio::test]
async fn delete_removes_a_finished_match() {
    let data = tempfile::tempdir().unwrap();
    let state = state(data.path(), runner(b"{}".to_vec(), None));
    let id = upload(&state, b"gone").await;
    wait_for(&state, &id, Status::Done).await;

    let delete = || {
        Request::delete(format!("/matches/{id}"))
            .body(Body::empty())
            .unwrap()
    };
    assert_eq!(
        call(&state, delete()).await.status(),
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        call(&state, get(&format!("/matches/{id}"))).await.status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(call(&state, delete()).await.status(), StatusCode::NOT_FOUND);
    assert!(!state.storage.dir(&id).unwrap().exists());
    assert_eq!(
        json(call(&state, get("/matches")).await).await,
        serde_json::json!([])
    );

    // the same demo can be uploaded again afterwards
    let again = upload(&state, b"gone").await;
    assert_ne!(again, id);
}

#[tokio::test]
async fn a_match_being_parsed_cannot_be_deleted_but_a_waiting_one_can() {
    let data = tempfile::tempdir().unwrap();
    let gate = Arc::new(AtomicBool::new(false));
    let state = state(data.path(), runner(b"{}".to_vec(), Some(gate.clone())));
    let running = upload(&state, b"running").await;
    wait_for(&state, &running, Status::Parsing).await;
    let waiting = upload(&state, b"waiting").await;

    let delete = |id: &str| {
        Request::delete(format!("/matches/{id}"))
            .body(Body::empty())
            .unwrap()
    };
    let response = call(&state, delete(&running)).await;
    assert_eq!(response.status(), StatusCode::CONFLICT);
    assert_eq!(json(response).await["error"]["code"], "conflict");
    assert!(state.storage.dir(&running).unwrap().exists());

    assert_eq!(
        call(&state, delete(&waiting)).await.status(),
        StatusCode::NO_CONTENT
    );

    // the queue goes on without the deleted match
    gate.store(true, Ordering::SeqCst);
    wait_for(&state, &running, Status::Done).await;
    let next = upload(&state, b"next").await;
    wait_for(&state, &next, Status::Done).await;
    assert_eq!(state.jobs.in_flight(), 0);
}
