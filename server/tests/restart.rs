//! A restart: a new server on the data directory of an old one.

use std::io::Write;
use std::path::Path;
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
use server::storage::{Meta, Status, Storage};
use tower::ServiceExt;

fn runner() -> Runner {
    Arc::new(|_dem, json, _report| {
        let mut encoder = GzEncoder::new(Vec::new(), Compression::fast());
        encoder.write_all(b"{\"ok\":true}").unwrap();
        std::fs::write(json, encoder.finish().unwrap()).unwrap();
        Ok(Parsed {
            map: "de_test".into(),
            schema_version: 1,
            rounds: 3,
            score: Some([2, 1]),
        })
    })
}

fn start(dir: &Path) -> AppState {
    let data = dir.to_str().unwrap().to_owned();
    let config = Config::from_lookup(|name| (name == "DATA_DIR").then(|| data.clone())).unwrap();
    AppState::with_runner(config, runner()).unwrap()
}

/// Puts a match on disk the way the server leaves it when it dies in a given state.
fn leave(storage: &Storage, status: Status, uploaded_at: u64, with_dem: bool) -> String {
    let mut meta = Meta::new(Storage::new_id(), None, 8);
    meta.status = status;
    meta.uploaded_at = uploaded_at;
    storage.create(&meta).unwrap();
    if with_dem {
        std::fs::write(storage.dem_path(&meta.id).unwrap(), b"PBDEMS2\0").unwrap();
    }
    meta.id
}

async fn get_json(state: &AppState, path: &str) -> (StatusCode, serde_json::Value) {
    let request = Request::get(path).body(Body::empty()).unwrap();
    let response = server::router(state.clone())
        .oneshot(request)
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null),
    )
}

#[tokio::test]
async fn a_restarted_server_finishes_what_the_old_one_left() {
    let data = tempfile::tempdir().unwrap();
    let old = start(data.path());

    // what a crash can leave behind
    let queued = leave(&old.storage, Status::Queued, 1, true);
    let interrupted = leave(&old.storage, Status::Parsing, 2, true);
    let lost = leave(&old.storage, Status::Parsing, 3, false);
    let failed = leave(&old.storage, Status::Failed, 4, false);
    let done = leave(&old.storage, Status::Done, 5, false);
    let half_upload = old.storage.create_dir(&Storage::new_id()).unwrap();
    std::fs::write(half_upload.join("match.dem.tmp"), b"PBDEMS2\0half").unwrap();
    drop(old);

    // the new server
    let state = start(data.path());
    let recovery = state.recover_and_resume().unwrap();
    assert_eq!(recovery.requeue.len(), 2);
    assert_eq!(recovery.failed, vec![lost.clone()]);
    assert_eq!(recovery.incomplete, 1);
    assert!(!half_upload.exists());

    // the two matches that still had a demo are parsed again, the queue limit does not apply
    for id in [&queued, &interrupted] {
        for _ in 0..500 {
            if state.storage.read_meta(id).unwrap().unwrap().status == Status::Done {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        let (_, detail) = get_json(&state, &format!("/matches/{id}")).await;
        assert_eq!(detail["status"], "done", "{id}");
        assert_eq!(detail["score"], serde_json::json!([2, 1]));
    }

    // the others are reported as they were or as lost
    let (_, detail) = get_json(&state, &format!("/matches/{lost}")).await;
    assert_eq!(detail["status"], "failed");
    assert!(detail["error"].as_str().unwrap().contains("lost"));
    let (_, detail) = get_json(&state, &format!("/matches/{failed}")).await;
    assert_eq!(detail["status"], "failed");
    let (_, detail) = get_json(&state, &format!("/matches/{done}")).await;
    assert_eq!(detail["status"], "done");

    // all five are listed, newest first
    let (_, list) = get_json(&state, "/matches").await;
    let ids: Vec<_> = list
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["id"].as_str().unwrap())
        .collect();
    assert_eq!(
        ids,
        vec![
            done.as_str(),
            failed.as_str(),
            lost.as_str(),
            interrupted.as_str(),
            queued.as_str()
        ]
    );
    assert_eq!(state.jobs.in_flight(), 0);
}

#[tokio::test]
async fn a_second_restart_changes_nothing() {
    let data = tempfile::tempdir().unwrap();
    let state = start(data.path());
    let id = leave(&state.storage, Status::Done, 1, false);
    let before = state.storage.read_meta(&id).unwrap();
    let recovery = state.recover_and_resume().unwrap();
    assert!(recovery.requeue.is_empty() && recovery.failed.is_empty());
    assert_eq!(state.storage.read_meta(&id).unwrap(), before);
}
