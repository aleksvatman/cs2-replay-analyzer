//! `POST /matches`: receives a demo and stores it (`server-doc.md`, sections 5, 8, 9).
//!
//! The body is streamed to disk chunk by chunk, so a 400 MB demo never sits in memory.
//! Checks happen as early as possible: the size limit from `Content-Length`, the demo
//! signature in the first 8 bytes, the size limit again while reading, and an idle timeout
//! for clients that stop sending. Whatever goes wrong, the half-written directory is removed.

use std::path::PathBuf;
use std::time::Duration;

use axum::body::Body;
use axum::extract::State;
use axum::http::{header, HeaderMap, StatusCode};
use axum::Json;
use futures_util::StreamExt;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tokio::fs::File;
use tokio::io::AsyncWriteExt;

use crate::error::ApiError;
use crate::state::AppState;
use crate::storage::{Meta, Status, Storage};

/// First bytes of every CS2 demo (`parser-core` checks the same signature).
const DEMO_MAGIC: &[u8; 8] = b"PBDEMS2\0";
/// A client that sends nothing for this long is cut off.
const IDLE_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_NAME_CHARS: usize = 255;
const NAME_HEADER: &str = "x-file-name";

pub async fn upload(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Body,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    check_content_type(&headers)?;
    let max = state.config.max_upload_bytes;
    if let Some(length) = content_length(&headers) {
        if length > max {
            return Err(ApiError::PayloadTooLarge);
        }
    }
    let name = original_name(&headers);

    let id = Storage::new_id();
    let dir = state.storage.create_dir(&id)?;
    // removes the directory on every early return and when the client disconnects
    let mut guard = DirGuard(Some(dir.clone()));

    let tmp = dir.join("match.dem.tmp");
    let (size, sha256) = receive(body, &tmp, max).await?;

    let _lock = state.upload_lock.lock().await;
    let duplicate = state
        .storage
        .list()?
        .into_iter()
        .find(|m| m.sha256.as_deref() == Some(&sha256) && m.status != Status::Failed);
    if let Some(existing) = duplicate {
        // `guard` drops here and deletes the copy we just received
        tracing::info!(id = %existing.id, "duplicate upload");
        return Ok((
            StatusCode::OK,
            Json(json!({ "id": existing.id, "status": existing.status })),
        ));
    }

    tokio::fs::rename(&tmp, state.storage.dem_path(&id)?)
        .await
        .map_err(|e| ApiError::Internal(format!("cannot move the demo into place: {e}")))?;
    let mut meta = Meta::new(id.clone(), name, size);
    meta.sha256 = Some(sha256);
    state.storage.write_meta(&meta)?;
    guard.disarm();

    tracing::info!(%id, size, "demo uploaded");
    Ok((
        StatusCode::ACCEPTED,
        Json(json!({ "id": id, "status": meta.status })),
    ))
}

/// Streams the body into `path`. Returns the size and the SHA-256 (hex) of what was written.
async fn receive(body: Body, path: &PathBuf, max: u64) -> Result<(u64, String), ApiError> {
    let mut file = File::create(path)
        .await
        .map_err(internal("cannot create the file"))?;
    let mut stream = body.into_data_stream();
    let mut hasher = Sha256::new();
    let mut head: Vec<u8> = Vec::with_capacity(DEMO_MAGIC.len());
    let mut total: u64 = 0;

    loop {
        let next = tokio::time::timeout(IDLE_TIMEOUT, stream.next())
            .await
            .map_err(|_| ApiError::RequestTimeout)?;
        let Some(chunk) = next else { break };
        let chunk = chunk.map_err(|_| ApiError::BadRequest("the upload was interrupted".into()))?;

        total += chunk.len() as u64;
        if total > max {
            return Err(ApiError::PayloadTooLarge);
        }
        if head.len() < DEMO_MAGIC.len() {
            let take = (DEMO_MAGIC.len() - head.len()).min(chunk.len());
            head.extend_from_slice(&chunk[..take]);
            if head.len() == DEMO_MAGIC.len() && head != DEMO_MAGIC {
                return Err(not_a_demo());
            }
        }
        hasher.update(&chunk);
        file.write_all(&chunk)
            .await
            .map_err(internal("cannot write the file"))?;
    }

    if head.len() < DEMO_MAGIC.len() {
        return Err(not_a_demo());
    }
    file.flush()
        .await
        .map_err(internal("cannot write the file"))?;
    file.sync_all()
        .await
        .map_err(internal("cannot write the file"))?;
    Ok((total, format!("{:x}", hasher.finalize())))
}

fn not_a_demo() -> ApiError {
    ApiError::Unprocessable("the file is not a CS2 demo".into())
}

fn internal(what: &'static str) -> impl Fn(std::io::Error) -> ApiError {
    move |e| ApiError::Internal(format!("{what}: {e}"))
}

/// Accepts no content type, `application/octet-stream` and `application/x-www-form-urlencoded`
/// (what `curl --data-binary` sends by default). The body is never parsed, only checked for the
/// demo signature, so the last one is harmless. Anything else (multipart, JSON, text) is `415`.
fn check_content_type(headers: &HeaderMap) -> Result<(), ApiError> {
    let Some(value) = headers.get(header::CONTENT_TYPE) else {
        return Ok(());
    };
    let essence = value
        .to_str()
        .ok()
        .and_then(|v| v.split(';').next())
        .map(|v| v.trim().to_ascii_lowercase());
    match essence.as_deref() {
        Some("application/octet-stream" | "application/x-www-form-urlencoded") => Ok(()),
        _ => Err(ApiError::UnsupportedMediaType),
    }
}

fn content_length(headers: &HeaderMap) -> Option<u64> {
    headers
        .get(header::CONTENT_LENGTH)?
        .to_str()
        .ok()?
        .parse()
        .ok()
}

/// Optional `X-File-Name`. Kept only as information (it never becomes part of a path),
/// without control characters and cut to a sane length.
fn original_name(headers: &HeaderMap) -> Option<String> {
    let raw = headers.get(NAME_HEADER)?.to_str().ok()?;
    let clean: String = raw
        .chars()
        .filter(|c| !c.is_control())
        .take(MAX_NAME_CHARS)
        .collect();
    let clean = clean.trim().to_owned();
    (!clean.is_empty()).then_some(clean)
}

/// Deletes an unfinished match directory unless disarmed.
struct DirGuard(Option<PathBuf>);

impl DirGuard {
    fn disarm(&mut self) {
        self.0 = None;
    }
}

impl Drop for DirGuard {
    fn drop(&mut self) {
        if let Some(dir) = self.0.take() {
            if let Err(e) = std::fs::remove_dir_all(&dir) {
                tracing::warn!("cannot remove {}: {e}", dir.display());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    fn headers(pairs: &[(&'static str, &str)]) -> HeaderMap {
        let mut map = HeaderMap::new();
        for (k, v) in pairs {
            map.insert(*k, HeaderValue::from_str(v).unwrap());
        }
        map
    }

    #[test]
    fn content_type_rules() {
        assert!(check_content_type(&headers(&[])).is_ok());
        assert!(
            check_content_type(&headers(&[("content-type", "application/octet-stream")])).is_ok()
        );
        assert!(check_content_type(&headers(&[(
            "content-type",
            "Application/Octet-Stream; x=1"
        )]))
        .is_ok());
        assert!(check_content_type(&headers(&[(
            "content-type",
            "application/x-www-form-urlencoded"
        )]))
        .is_ok());
        for bad in [
            "text/plain",
            "application/json",
            "multipart/form-data; boundary=x",
        ] {
            assert!(
                matches!(
                    check_content_type(&headers(&[("content-type", bad)])),
                    Err(ApiError::UnsupportedMediaType)
                ),
                "{bad}"
            );
        }
    }

    #[test]
    fn file_name_is_cleaned() {
        assert_eq!(original_name(&headers(&[])), None);
        assert_eq!(original_name(&headers(&[("x-file-name", "  ")])), None);
        assert_eq!(
            original_name(&headers(&[("x-file-name", "../../final.dem")])).as_deref(),
            Some("../../final.dem"),
            "kept as text, it is never used as a path"
        );
        let long = "a".repeat(1000);
        assert_eq!(
            original_name(&headers(&[("x-file-name", &long)]))
                .unwrap()
                .chars()
                .count(),
            MAX_NAME_CHARS
        );
    }
}
