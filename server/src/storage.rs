//! Match files on disk (`server-doc.md`, section 9).
//!
//! Layout: `DATA_DIR/<id>/{meta.json, match.dem, match.json.gz}`. `<id>` is a hyphenated
//! UUID v4; any other string is rejected before it can reach a path. Every write goes to a
//! temporary file first and is renamed, so a crash never leaves a half-written `meta.json`.
//!
//! The methods are synchronous: the files are small and calls are short. Handlers that
//! touch large files must run them inside `spawn_blocking`.

use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

const META_FILE: &str = "meta.json";
const DEM_FILE: &str = "match.dem";
const JSON_FILE: &str = "match.json.gz";
const TMP_SUFFIX: &str = ".tmp";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Queued,
    Parsing,
    Done,
    Failed,
}

/// What is persisted about one match. Progress (stage, percent) lives in memory only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Meta {
    pub id: String,
    pub status: Status,
    pub error: Option<String>,
    /// Name sent by the client. Informational only, never used in a path.
    pub original_name: Option<String>,
    pub uploaded_at: u64,
    pub parsed_at: Option<u64>,
    pub size_bytes: u64,
    pub sha256: Option<String>,
    pub map: Option<String>,
    pub schema_version: Option<u32>,
    /// Number of rounds played.
    pub rounds: Option<u32>,
    /// Score `[a, b]`, where `a` is the team that started on the T side.
    pub score: Option<[u32; 2]>,
}

impl Meta {
    pub fn new(id: String, original_name: Option<String>, size_bytes: u64) -> Meta {
        Meta {
            id,
            status: Status::Queued,
            error: None,
            original_name,
            uploaded_at: now_unix(),
            parsed_at: None,
            size_bytes,
            sha256: None,
            map: None,
            schema_version: None,
            rounds: None,
            score: None,
        }
    }
}

#[derive(Debug)]
pub enum StorageError {
    /// The id is not a hyphenated lowercase UUID.
    InvalidId,
    Io(io::Error),
    Json(serde_json::Error),
}

impl std::fmt::Display for StorageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StorageError::InvalidId => write!(f, "invalid match id"),
            StorageError::Io(e) => write!(f, "storage I/O error: {e}"),
            StorageError::Json(e) => write!(f, "invalid meta.json: {e}"),
        }
    }
}

impl std::error::Error for StorageError {}

impl From<io::Error> for StorageError {
    fn from(e: io::Error) -> Self {
        StorageError::Io(e)
    }
}

impl From<serde_json::Error> for StorageError {
    fn from(e: serde_json::Error) -> Self {
        StorageError::Json(e)
    }
}

impl From<StorageError> for crate::error::ApiError {
    fn from(e: StorageError) -> Self {
        match e {
            // an id that cannot exist is simply "not found" for the client
            StorageError::InvalidId => crate::error::ApiError::NotFound,
            other => crate::error::ApiError::Internal(other.to_string()),
        }
    }
}

/// Result of [`Storage::recover`].
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Recovery {
    /// Ids that were `queued` or `parsing` before the restart and still have their demo.
    pub requeue: Vec<String>,
    /// Ids marked `failed` because the demo is gone.
    pub failed: Vec<String>,
    /// Directories without `meta.json` (uploads cut off by a crash) that were removed.
    pub incomplete: usize,
}

#[derive(Debug)]
pub struct Storage {
    root: PathBuf,
}

impl Storage {
    /// Opens (and creates, if needed) the data directory.
    pub fn open(root: impl Into<PathBuf>) -> io::Result<Storage> {
        let root = root.into();
        fs::create_dir_all(&root)?;
        Ok(Storage { root })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn new_id() -> String {
        Uuid::new_v4().to_string()
    }

    /// Directory of one match; the only place where an id becomes a path.
    pub fn dir(&self, id: &str) -> Result<PathBuf, StorageError> {
        match Uuid::parse_str(id) {
            Ok(uuid) if uuid.hyphenated().to_string() == id => Ok(self.root.join(id)),
            _ => Err(StorageError::InvalidId),
        }
    }

    pub fn dem_path(&self, id: &str) -> Result<PathBuf, StorageError> {
        Ok(self.dir(id)?.join(DEM_FILE))
    }

    pub fn json_path(&self, id: &str) -> Result<PathBuf, StorageError> {
        Ok(self.dir(id)?.join(JSON_FILE))
    }

    /// Creates the (empty) directory of a new match. Fails if it already exists.
    /// An upload writes its files here first and adds `meta.json` last, so a directory
    /// without `meta.json` is an unfinished upload.
    pub fn create_dir(&self, id: &str) -> Result<PathBuf, StorageError> {
        let dir = self.dir(id)?;
        fs::create_dir(&dir)?;
        Ok(dir)
    }

    /// Creates the directory of a new match and writes its first `meta.json`.
    pub fn create(&self, meta: &Meta) -> Result<(), StorageError> {
        self.create_dir(&meta.id)?;
        self.write_meta(meta)
    }

    pub fn write_meta(&self, meta: &Meta) -> Result<(), StorageError> {
        let dir = self.dir(&meta.id)?;
        let bytes = serde_json::to_vec_pretty(meta)?;
        write_atomic(&dir.join(META_FILE), &bytes)?;
        Ok(())
    }

    /// `Ok(None)` when the match does not exist.
    pub fn read_meta(&self, id: &str) -> Result<Option<Meta>, StorageError> {
        let path = self.dir(id)?.join(META_FILE);
        match fs::read(&path) {
            Ok(bytes) => Ok(Some(serde_json::from_slice(&bytes)?)),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// All readable matches, oldest first. Broken directories are logged and skipped.
    pub fn list(&self) -> Result<Vec<Meta>, StorageError> {
        let mut metas = Vec::new();
        for entry in fs::read_dir(&self.root)? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }
            let Some(id) = entry.file_name().to_str().map(str::to_owned) else {
                continue;
            };
            match self.read_meta(&id) {
                Ok(Some(meta)) => metas.push(meta),
                Ok(None) => tracing::warn!(%id, "match directory without meta.json, skipped"),
                Err(e) => tracing::warn!(%id, "unreadable match, skipped: {e}"),
            }
        }
        metas.sort_by(|a, b| {
            a.uploaded_at
                .cmp(&b.uploaded_at)
                .then_with(|| a.id.cmp(&b.id))
        });
        Ok(metas)
    }

    /// Number of match directories, finished or still being uploaded.
    pub fn count(&self) -> Result<usize, StorageError> {
        let mut count = 0;
        for entry in fs::read_dir(&self.root)? {
            let entry = entry?;
            if entry.file_type()?.is_dir()
                && entry
                    .file_name()
                    .to_str()
                    .is_some_and(|name| self.dir(name).is_ok())
            {
                count += 1;
            }
        }
        Ok(count)
    }

    /// Removes a match with all its files. `Ok(false)` when it did not exist.
    pub fn delete(&self, id: &str) -> Result<bool, StorageError> {
        match fs::remove_dir_all(self.dir(id)?) {
            Ok(()) => Ok(true),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(false),
            Err(e) => Err(e.into()),
        }
    }

    /// Startup scan: removes unfinished uploads and leftover temporary files, and settles matches that were
    /// interrupted by a restart. A match that was `queued` or `parsing` goes back to
    /// `queued` if its demo is still on disk, otherwise it becomes `failed`.
    pub fn recover(&self) -> Result<Recovery, StorageError> {
        let mut recovery = Recovery::default();
        for entry in fs::read_dir(&self.root)? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }
            let path = entry.path();
            let is_match_dir = entry
                .file_name()
                .to_str()
                .is_some_and(|name| self.dir(name).is_ok());
            if is_match_dir && !path.join(META_FILE).exists() {
                fs::remove_dir_all(&path)?;
                recovery.incomplete += 1;
            } else {
                remove_tmp_files(&path);
            }
        }
        for mut meta in self.list()? {
            if !matches!(meta.status, Status::Queued | Status::Parsing) {
                continue;
            }
            if self.dem_path(&meta.id)?.is_file() {
                if meta.status != Status::Queued {
                    meta.status = Status::Queued;
                    self.write_meta(&meta)?;
                }
                recovery.requeue.push(meta.id);
            } else {
                meta.status = Status::Failed;
                meta.error = Some("the demo file was lost when the server restarted".into());
                self.write_meta(&meta)?;
                recovery.failed.push(meta.id);
            }
        }
        Ok(recovery)
    }
}

pub fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// Writes `bytes` to `<path>.tmp`, flushes to disk and renames over `path`.
fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(TMP_SUFFIX);
    let tmp = PathBuf::from(tmp);
    let result = (|| {
        let mut file = File::create(&tmp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        fs::rename(&tmp, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result
}

fn remove_tmp_files(dir: &Path) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        if entry.file_name().to_string_lossy().ends_with(TMP_SUFFIX) {
            let _ = fs::remove_file(entry.path());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn storage() -> (tempfile::TempDir, Storage) {
        let dir = tempfile::tempdir().unwrap();
        let storage = Storage::open(dir.path().join("data")).unwrap();
        (dir, storage)
    }

    fn make(storage: &Storage, status: Status, with_dem: bool) -> Meta {
        let mut meta = Meta::new(Storage::new_id(), Some("a.dem".into()), 10);
        meta.status = status;
        storage.create(&meta).unwrap();
        if with_dem {
            fs::write(storage.dem_path(&meta.id).unwrap(), b"PBDEMS2\0").unwrap();
        }
        meta
    }

    #[test]
    fn open_creates_the_directory() {
        let (_guard, storage) = storage();
        assert!(storage.root().is_dir());
    }

    #[test]
    fn ids_that_are_not_uuids_never_become_paths() {
        let (_guard, storage) = storage();
        for bad in [
            "",
            "..",
            "../x",
            "a/b",
            "not-a-uuid",
            "6F9619FF-8B86-D011-B42D-00C04FC964FF",
            "6f9619ff8b86d011b42d00c04fc964ff",
            "{6f9619ff-8b86-d011-b42d-00c04fc964ff}",
        ] {
            assert!(
                matches!(storage.dir(bad), Err(StorageError::InvalidId)),
                "{bad:?}"
            );
        }
        assert!(storage.dir(&Storage::new_id()).is_ok());
    }

    #[test]
    fn meta_round_trips_and_leaves_no_temp_file() {
        let (_guard, storage) = storage();
        let mut meta = Meta::new(Storage::new_id(), Some("final.dem".into()), 123);
        storage.create(&meta).unwrap();
        assert_eq!(storage.read_meta(&meta.id).unwrap(), Some(meta.clone()));

        meta.status = Status::Done;
        meta.map = Some("de_ancient".into());
        meta.schema_version = Some(1);
        storage.write_meta(&meta).unwrap();
        assert_eq!(storage.read_meta(&meta.id).unwrap(), Some(meta.clone()));

        let names: Vec<_> = fs::read_dir(storage.dir(&meta.id).unwrap())
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, vec!["meta.json"]);
    }

    #[test]
    fn meta_json_uses_camel_case_and_lowercase_status() {
        let meta = Meta::new(Storage::new_id(), None, 1);
        let text = serde_json::to_string(&meta).unwrap();
        assert!(text.contains("\"status\":\"queued\""));
        assert!(text.contains("\"uploadedAt\""));
        assert!(text.contains("\"sizeBytes\""));
    }

    #[test]
    fn missing_match_reads_as_none() {
        let (_guard, storage) = storage();
        assert_eq!(storage.read_meta(&Storage::new_id()).unwrap(), None);
    }

    #[test]
    fn create_twice_fails() {
        let (_guard, storage) = storage();
        let meta = make(&storage, Status::Queued, false);
        assert!(matches!(storage.create(&meta), Err(StorageError::Io(_))));
    }

    #[test]
    fn list_returns_matches_oldest_first_and_skips_broken_ones() {
        let (_guard, storage) = storage();
        let mut first = Meta::new(Storage::new_id(), None, 1);
        first.uploaded_at = 100;
        let mut second = Meta::new(Storage::new_id(), None, 1);
        second.uploaded_at = 200;
        storage.create(&second).unwrap();
        storage.create(&first).unwrap();

        // a directory without meta.json, a corrupt meta.json and a stray file
        fs::create_dir(storage.root().join(Storage::new_id())).unwrap();
        let corrupt = Storage::new_id();
        fs::create_dir(storage.root().join(&corrupt)).unwrap();
        fs::write(storage.root().join(&corrupt).join("meta.json"), b"{oops").unwrap();
        fs::write(storage.root().join("note.txt"), b"x").unwrap();

        let ids: Vec<_> = storage.list().unwrap().into_iter().map(|m| m.id).collect();
        assert_eq!(ids, vec![first.id, second.id]);
    }

    #[test]
    fn delete_removes_files_and_reports_missing() {
        let (_guard, storage) = storage();
        let meta = make(&storage, Status::Done, true);
        assert!(storage.delete(&meta.id).unwrap());
        assert!(!storage.dir(&meta.id).unwrap().exists());
        assert!(!storage.delete(&meta.id).unwrap());
        assert!(matches!(
            storage.delete("../etc"),
            Err(StorageError::InvalidId)
        ));
    }

    #[test]
    fn recover_requeues_interrupted_matches_that_still_have_a_demo() {
        let (_guard, storage) = storage();
        let queued = make(&storage, Status::Queued, true);
        let parsing = make(&storage, Status::Parsing, true);
        let lost_queued = make(&storage, Status::Queued, false);
        let lost_parsing = make(&storage, Status::Parsing, false);
        let done = make(&storage, Status::Done, false);
        let failed = make(&storage, Status::Failed, false);

        let mut recovery = storage.recover().unwrap();
        recovery.requeue.sort();
        recovery.failed.sort();
        let mut want_requeue = vec![queued.id.clone(), parsing.id.clone()];
        want_requeue.sort();
        let mut want_failed = vec![lost_queued.id.clone(), lost_parsing.id.clone()];
        want_failed.sort();
        assert_eq!(
            recovery,
            Recovery {
                requeue: want_requeue,
                failed: want_failed,
                incomplete: 0
            }
        );

        let status = |id: &str| storage.read_meta(id).unwrap().unwrap();
        assert_eq!(status(&queued.id).status, Status::Queued);
        assert_eq!(status(&parsing.id).status, Status::Queued);
        assert_eq!(status(&lost_queued.id).status, Status::Failed);
        assert!(status(&lost_parsing.id).error.is_some());
        assert_eq!(status(&done.id), done);
        assert_eq!(status(&failed.id), failed);

        // a second pass changes nothing
        let again = storage.recover().unwrap();
        assert_eq!(again.requeue.len(), 2);
        assert!(again.failed.is_empty());
    }

    #[test]
    fn count_counts_match_directories_only() {
        let (_guard, storage) = storage();
        assert_eq!(storage.count().unwrap(), 0);
        make(&storage, Status::Done, false);
        storage.create_dir(&Storage::new_id()).unwrap(); // an upload in progress
        fs::create_dir(storage.root().join("not-a-match")).unwrap();
        fs::write(storage.root().join("note.txt"), b"x").unwrap();
        assert_eq!(storage.count().unwrap(), 2);
    }

    #[test]
    fn recover_removes_unfinished_uploads_only() {
        let (_guard, storage) = storage();
        let id = Storage::new_id();
        let dir = storage.create_dir(&id).unwrap();
        fs::write(dir.join("match.dem.tmp"), b"half").unwrap();
        let stranger = storage.root().join("not-a-match");
        fs::create_dir(&stranger).unwrap();
        let kept = make(&storage, Status::Done, false);

        let recovery = storage.recover().unwrap();
        assert_eq!(recovery.incomplete, 1);
        assert!(!dir.exists());
        assert!(stranger.exists());
        assert_eq!(storage.read_meta(&kept.id).unwrap(), Some(kept));
    }

    #[test]
    fn recover_removes_leftover_temp_files() {
        let (_guard, storage) = storage();
        let meta = make(&storage, Status::Done, false);
        let tmp = storage.dir(&meta.id).unwrap().join("meta.json.tmp");
        fs::write(&tmp, b"half").unwrap();
        storage.recover().unwrap();
        assert!(!tmp.exists());
        assert_eq!(storage.read_meta(&meta.id).unwrap(), Some(meta));
    }
}
