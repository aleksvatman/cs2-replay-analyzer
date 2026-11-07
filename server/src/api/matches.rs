//! Reading and deleting matches (`server-doc.md`, sections 5 and 10).
//!
//! `GET /matches`, `GET /matches/{id}`, `GET /matches/{id}/data`, `DELETE /matches/{id}`.
//! The JSON of a match is stored gzipped. A client that accepts gzip gets the file as it is;
//! any other client gets it decompressed on the fly. Neither way holds the file in memory.

use std::io::Read;

use axum::body::{Body, Bytes};
use axum::extract::{Path, State};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use flate2::read::GzDecoder;
use futures_util::stream;
use serde::Serialize;
use tokio::fs::File;
use tokio::io::AsyncReadExt;
use tokio::sync::mpsc;

use crate::error::ApiError;
use crate::state::AppState;
use crate::storage::{Meta, Status};

const CHUNK: usize = 64 * 1024;

/// One row of `GET /matches`.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Item {
    pub id: String,
    pub status: Status,
    pub map: Option<String>,
    pub uploaded_at: u64,
    pub size_bytes: u64,
    pub original_name: Option<String>,
    pub rounds: Option<u32>,
    pub score: Option<[u32; 2]>,
    /// Format of the stored result; compare it with the version your client understands.
    pub schema_version: Option<u32>,
}

/// `GET /matches/{id}`: the row plus progress and the error, if any.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Detail {
    #[serde(flatten)]
    pub item: Item,
    pub parsed_at: Option<u64>,
    /// Current parse stage; `null` while waiting and after a failure.
    pub stage: Option<&'static str>,
    /// 0 while waiting, 100 when done.
    pub percent: u8,
    pub error: Option<String>,
}

impl From<&Meta> for Item {
    fn from(m: &Meta) -> Item {
        Item {
            id: m.id.clone(),
            status: m.status,
            map: m.map.clone(),
            uploaded_at: m.uploaded_at,
            size_bytes: m.size_bytes,
            original_name: m.original_name.clone(),
            rounds: m.rounds,
            score: m.score,
            schema_version: m.schema_version,
        }
    }
}

/// Newest first.
pub async fn list(State(state): State<AppState>) -> Result<Json<Vec<Item>>, ApiError> {
    let storage = state.storage.clone();
    let mut metas = blocking(move || storage.list()).await?;
    metas.reverse();
    Ok(Json(metas.iter().map(Item::from).collect()))
}

pub async fn status(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Detail>, ApiError> {
    let meta = load(&state, &id).await?;
    let (stage, percent) = match meta.status {
        Status::Queued => (None, 0),
        Status::Parsing => match state.jobs.live(&id) {
            Some(live) => (Some(live.stage), live.percent),
            // between the status change and the first progress report
            None => (Some("events"), 0),
        },
        Status::Done => (Some("done"), 100),
        Status::Failed => (None, 0),
    };
    Ok(Json(Detail {
        item: Item::from(&meta),
        parsed_at: meta.parsed_at,
        stage,
        percent,
        error: meta.error.clone(),
    }))
}

pub async fn delete(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let meta = load(&state, &id).await?;
    if meta.status == Status::Parsing {
        return Err(ApiError::Conflict(
            "the match is being parsed, try again when it is finished".into(),
        ));
    }
    let storage = state.storage.clone();
    let removed = blocking(move || storage.delete(&id)).await?;
    if removed {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(ApiError::NotFound)
    }
}

/// Parses a kept demo again, for example after the parser or the JSON format changed.
/// A failed re-parse leaves the previous result in place (see `jobs.rs`).
pub async fn reparse(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<(StatusCode, Json<serde_json::Value>), ApiError> {
    let _lock = state.upload_lock.lock().await;
    let mut meta = load(&state, &id).await?;
    if !matches!(meta.status, Status::Done | Status::Failed) {
        return Err(ApiError::Conflict(
            "the match is already waiting for or being parsed".into(),
        ));
    }
    if !state.storage.dem_path(&id)?.is_file() {
        return Err(ApiError::Conflict(
            "the demo was not kept (start the server with KEEP_DEM=true) or is gone; upload it again".into(),
        ));
    }
    let before = meta.clone();
    meta.status = Status::Queued;
    meta.error = None;
    let storage = state.storage.clone();
    let queued = meta.clone();
    blocking(move || storage.write_meta(&queued)).await?;
    if let Err(e) = state.jobs.submit(id.clone()) {
        // the queue is full: leave the match as it was
        let storage = state.storage.clone();
        blocking(move || storage.write_meta(&before)).await?;
        return Err(e);
    }
    tracing::info!(%id, "re-parse queued");
    Ok((
        StatusCode::ACCEPTED,
        Json(serde_json::json!({ "id": id, "status": meta.status })),
    ))
}

pub async fn data(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let meta = load(&state, &id).await?;
    if meta.status != Status::Done {
        return Err(ApiError::NotReady);
    }
    let path = state.storage.json_path(&id)?;
    let file = File::open(&path)
        .await
        .map_err(|e| ApiError::Internal(format!("result of {id} is unreadable: {e}")))?;
    let len = file
        .metadata()
        .await
        .map_err(|e| ApiError::Internal(format!("result of {id} is unreadable: {e}")))?
        .len();

    let gzip = accepts_gzip(&headers);
    // The tag changes when the match is parsed again (time, size) and differs between the
    // compressed and the plain variant of the same result.
    let etag = format!(
        "\"{}-{}-{}{}\"",
        meta.sha256
            .as_deref()
            .and_then(|h| h.get(..16))
            .unwrap_or("x"),
        meta.parsed_at.unwrap_or(0),
        len,
        if gzip { "-gzip" } else { "" }
    );

    let not_modified = if_none_match(&headers, &etag);
    let mut response = if not_modified {
        StatusCode::NOT_MODIFIED.into_response()
    } else if gzip {
        let mut response = Response::new(Body::from_stream(file_chunks(file)));
        response
            .headers_mut()
            .insert(header::CONTENT_ENCODING, HeaderValue::from_static("gzip"));
        response
            .headers_mut()
            .insert(header::CONTENT_LENGTH, HeaderValue::from(len));
        response
    } else {
        Response::new(Body::from_stream(gunzip_chunks(file.into_std().await)))
    };
    let headers = response.headers_mut();
    if !not_modified {
        headers.insert(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        );
    }
    headers.insert(header::ETAG, HeaderValue::from_str(&etag).expect("ascii"));
    headers.insert(header::VARY, HeaderValue::from_static("Accept-Encoding"));
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("private, no-cache"),
    );
    Ok(response)
}

async fn load(state: &AppState, id: &str) -> Result<Meta, ApiError> {
    let storage = state.storage.clone();
    let id = id.to_owned();
    blocking(move || storage.read_meta(&id))
        .await?
        .ok_or(ApiError::NotFound)
}

/// Runs a short blocking storage call off the async threads.
async fn blocking<T, F>(f: F) -> Result<T, ApiError>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, crate::storage::StorageError> + Send + 'static,
{
    match tokio::task::spawn_blocking(f).await {
        Ok(result) => result.map_err(ApiError::from),
        Err(e) => Err(ApiError::Internal(format!("storage task failed: {e}"))),
    }
}

/// The file as it is, in chunks.
fn file_chunks(file: File) -> impl futures_util::Stream<Item = std::io::Result<Bytes>> {
    stream::unfold(Some(file), |state| async move {
        let mut file = state?;
        let mut buf = vec![0u8; CHUNK];
        match file.read(&mut buf).await {
            Ok(0) => None,
            Ok(n) => {
                buf.truncate(n);
                Some((Ok(Bytes::from(buf)), Some(file)))
            }
            // report the error once and stop
            Err(e) => Some((Err(e), None)),
        }
    })
}

/// The file decompressed. The decoder is synchronous, so it runs in a blocking task and hands
/// chunks over a small channel; a client that goes away closes the channel and stops it.
fn gunzip_chunks(file: std::fs::File) -> impl futures_util::Stream<Item = std::io::Result<Bytes>> {
    let (tx, rx) = mpsc::channel::<std::io::Result<Bytes>>(4);
    tokio::task::spawn_blocking(move || {
        let mut decoder = GzDecoder::new(std::io::BufReader::new(file));
        let mut buf = vec![0u8; CHUNK];
        loop {
            let message = match decoder.read(&mut buf) {
                Ok(0) => return,
                Ok(n) => Ok(Bytes::copy_from_slice(&buf[..n])),
                Err(e) => Err(e),
            };
            let failed = message.is_err();
            if tx.blocking_send(message).is_err() || failed {
                return;
            }
        }
    });
    stream::unfold(rx, |mut rx| async move {
        rx.recv().await.map(|item| (item, rx))
    })
}

/// `Accept-Encoding` allows gzip: it is listed (or `*` is) with a quality above zero.
fn accepts_gzip(headers: &HeaderMap) -> bool {
    let Some(value) = headers
        .get(header::ACCEPT_ENCODING)
        .and_then(|v| v.to_str().ok())
    else {
        return false;
    };
    let mut star = None;
    for part in value.split(',') {
        let mut pieces = part.split(';');
        let name = pieces.next().unwrap_or("").trim().to_ascii_lowercase();
        let quality = pieces
            .filter_map(|p| p.trim().strip_prefix("q="))
            .find_map(|q| q.trim().parse::<f32>().ok())
            .unwrap_or(1.0);
        match name.as_str() {
            "gzip" | "x-gzip" => return quality > 0.0,
            "*" => star = Some(quality > 0.0),
            _ => {}
        }
    }
    star.unwrap_or(false)
}

/// `If-None-Match` names this tag (weak or strong comparison) or is `*`.
fn if_none_match(headers: &HeaderMap, etag: &str) -> bool {
    let Some(value) = headers
        .get(header::IF_NONE_MATCH)
        .and_then(|v| v.to_str().ok())
    else {
        return false;
    };
    value.split(',').map(str::trim).any(|candidate| {
        candidate == "*" || candidate.strip_prefix("W/").unwrap_or(candidate) == etag
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with(name: &'static str, value: &str) -> HeaderMap {
        let mut map = HeaderMap::new();
        map.insert(name, HeaderValue::from_str(value).unwrap());
        map
    }

    #[test]
    fn accept_encoding() {
        assert!(!accepts_gzip(&HeaderMap::new()));
        assert!(accepts_gzip(&with("accept-encoding", "gzip")));
        assert!(accepts_gzip(&with("accept-encoding", "br, gzip;q=0.5")));
        assert!(accepts_gzip(&with("accept-encoding", "GZIP, deflate")));
        assert!(accepts_gzip(&with("accept-encoding", "*")));
        assert!(!accepts_gzip(&with("accept-encoding", "gzip;q=0")));
        assert!(!accepts_gzip(&with("accept-encoding", "identity")));
        assert!(!accepts_gzip(&with("accept-encoding", "*;q=0")));
        // an explicit gzip entry wins over the wildcard
        assert!(!accepts_gzip(&with("accept-encoding", "*, gzip;q=0")));
    }

    #[test]
    fn if_none_match() {
        let tag = "\"abc-1-2\"";
        assert!(!super::if_none_match(&HeaderMap::new(), tag));
        assert!(super::if_none_match(&with("if-none-match", tag), tag));
        assert!(super::if_none_match(
            &with("if-none-match", "W/\"abc-1-2\""),
            tag
        ));
        assert!(super::if_none_match(
            &with("if-none-match", "\"x\", \"abc-1-2\""),
            tag
        ));
        assert!(super::if_none_match(&with("if-none-match", "*"), tag));
        assert!(!super::if_none_match(
            &with("if-none-match", "\"other\""),
            tag
        ));
    }
}
