//! Deletes old matches (`RETENTION_DAYS`, `server-doc.md`, section 9).
//!
//! Only finished matches (`done`, `failed`) are removed; one that is waiting or being parsed
//! is never touched. The age counts from the later of the upload and the last parse, so a
//! match that was parsed again lives on.

use std::time::Duration;

use crate::state::AppState;
use crate::storage::{now_unix, Status, Storage, StorageError};

/// How often the sweep runs; the first run is right at the start.
pub const SWEEP_EVERY: Duration = Duration::from_secs(3600);
const DAY: u64 = 24 * 60 * 60;

/// Removes finished matches older than `retention_days` and returns their ids.
/// 0 days means "keep forever" and removes nothing.
pub fn sweep(
    storage: &Storage,
    retention_days: u32,
    now: u64,
) -> Result<Vec<String>, StorageError> {
    if retention_days == 0 {
        return Ok(Vec::new());
    }
    let cutoff = now.saturating_sub(u64::from(retention_days) * DAY);
    let mut removed = Vec::new();
    for meta in storage.list()? {
        let finished = matches!(meta.status, Status::Done | Status::Failed);
        let last_touched = meta.uploaded_at.max(meta.parsed_at.unwrap_or(0));
        if finished && last_touched < cutoff && storage.delete(&meta.id)? {
            removed.push(meta.id);
        }
    }
    Ok(removed)
}

/// Starts the background sweep. `None` when `RETENTION_DAYS` is 0. Must be called inside
/// the tokio runtime.
pub fn spawn(state: AppState) -> Option<tokio::task::JoinHandle<()>> {
    let days = state.config.retention_days;
    if days == 0 {
        return None;
    }
    Some(tokio::spawn(async move {
        let mut tick = tokio::time::interval(SWEEP_EVERY);
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            tick.tick().await;
            // an upload that is being finalized must not find a match that is deleted now
            let _lock = state.upload_lock.lock().await;
            let storage = state.storage.clone();
            let now = now_unix();
            match tokio::task::spawn_blocking(move || sweep(&storage, days, now)).await {
                Ok(Ok(removed)) if !removed.is_empty() => {
                    tracing::info!(count = removed.len(), days, "old matches removed");
                }
                Ok(Ok(_)) => {}
                Ok(Err(e)) => tracing::warn!("retention sweep failed: {e}"),
                Err(e) => tracing::warn!("retention sweep crashed: {e}"),
            }
        }
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::Meta;

    const NOW: u64 = 100 * DAY;

    fn add(
        storage: &Storage,
        status: Status,
        uploaded_days_ago: u64,
        parsed_days_ago: Option<u64>,
    ) -> String {
        let mut meta = Meta::new(Storage::new_id(), None, 1);
        meta.status = status;
        meta.uploaded_at = NOW - uploaded_days_ago * DAY;
        meta.parsed_at = parsed_days_ago.map(|d| NOW - d * DAY);
        storage.create(&meta).unwrap();
        meta.id
    }

    fn storage() -> (tempfile::TempDir, Storage) {
        let dir = tempfile::tempdir().unwrap();
        let storage = Storage::open(dir.path().join("data")).unwrap();
        (dir, storage)
    }

    #[test]
    fn removes_only_old_finished_matches() {
        let (_g, storage) = storage();
        let old_done = add(&storage, Status::Done, 40, Some(40));
        let old_failed = add(&storage, Status::Failed, 31, None);
        let fresh_done = add(&storage, Status::Done, 5, Some(5));
        let old_queued = add(&storage, Status::Queued, 90, None);
        let old_parsing = add(&storage, Status::Parsing, 90, None);
        // uploaded long ago but parsed again recently: it stays
        let reparsed = add(&storage, Status::Done, 90, Some(2));

        let mut removed = sweep(&storage, 30, NOW).unwrap();
        removed.sort();
        let mut want = vec![old_done.clone(), old_failed.clone()];
        want.sort();
        assert_eq!(removed, want);

        for gone in [&old_done, &old_failed] {
            assert!(!storage.dir(gone).unwrap().exists());
        }
        for kept in [&fresh_done, &old_queued, &old_parsing, &reparsed] {
            assert!(storage.dir(kept).unwrap().exists(), "{kept}");
        }
    }

    #[test]
    fn the_limit_is_exact_to_the_day() {
        let (_g, storage) = storage();
        let on_the_edge = add(&storage, Status::Done, 30, Some(30));
        let just_over = {
            let mut meta = storage
                .read_meta(&add(&storage, Status::Done, 30, Some(30)))
                .unwrap()
                .unwrap();
            meta.uploaded_at -= 1;
            meta.parsed_at = Some(meta.uploaded_at);
            storage.write_meta(&meta).unwrap();
            meta.id
        };
        let removed = sweep(&storage, 30, NOW).unwrap();
        assert_eq!(removed, vec![just_over]);
        assert!(storage.dir(&on_the_edge).unwrap().exists());
    }

    #[test]
    fn zero_days_keeps_everything() {
        let (_g, storage) = storage();
        let id = add(&storage, Status::Done, 99, Some(99));
        assert!(sweep(&storage, 0, NOW).unwrap().is_empty());
        assert!(storage.dir(&id).unwrap().exists());
    }

    #[tokio::test]
    async fn the_background_task_sweeps_at_start_and_not_without_a_limit() {
        use crate::config::Config;
        let dir = tempfile::tempdir().unwrap();
        let data = dir.path().to_str().unwrap().to_owned();
        let make = |days: &'static str| {
            let data = data.clone();
            let config = Config::from_lookup(move |name| match name {
                "DATA_DIR" => Some(data.clone()),
                "RETENTION_DAYS" => Some(days.to_owned()),
                _ => None,
            })
            .unwrap();
            AppState::new(config).unwrap()
        };

        let off = make("0");
        assert!(spawn(off.clone()).is_none());

        let on = make("30");
        let mut meta = Meta::new(Storage::new_id(), None, 1);
        meta.status = Status::Done;
        meta.uploaded_at = now_unix() - 60 * DAY;
        on.storage.create(&meta).unwrap();
        let recent = Meta::new(Storage::new_id(), None, 1);
        on.storage.create(&recent).unwrap();
        let mut done_recent = recent.clone();
        done_recent.status = Status::Done;
        on.storage.write_meta(&done_recent).unwrap();

        let handle = spawn(on.clone()).expect("started");
        for _ in 0..200 {
            if !on.storage.dir(&meta.id).unwrap().exists() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        handle.abort();
        assert!(!on.storage.dir(&meta.id).unwrap().exists());
        assert!(on.storage.dir(&recent.id).unwrap().exists());
    }
}
