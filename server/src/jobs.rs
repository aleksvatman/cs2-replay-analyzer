//! The parse queue (`server-doc.md`, sections 6 and 7).
//!
//! A job is one match id. At most `MAX_PARALLEL_PARSES` jobs parse at the same time (a
//! semaphore; tokio hands permits out in arrival order), the rest wait. At most
//! `MAX_PARALLEL_PARSES + MAX_QUEUE` jobs exist at once: a new upload beyond that is refused
//! with `503` before its body is read. The parser itself is CPU-heavy and synchronous, so it
//! runs inside `spawn_blocking`.
//!
//! Status changes are written to `meta.json` (`queued -> parsing -> done | failed`); the
//! stage and percent of a running job live in memory only.

use std::collections::HashMap;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use flate2::write::GzEncoder;
use flate2::Compression;
use parser_core::{Progress, Stage};
use tokio::sync::Semaphore;

use crate::error::ApiError;
use crate::storage::{now_unix, Status, Storage};

/// What a finished parse tells the server about the match.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Parsed {
    pub map: String,
    pub schema_version: u32,
    pub rounds: u32,
    /// `[a, b]`: `a` is the team that started on the T side. `None` if it cannot be told.
    pub score: Option<[u32; 2]>,
}

/// Parses `dem` and writes the gzip-compressed JSON to `json`. Errors are plain text for
/// `meta.json`. A function value so that tests can replace the heavy parser.
pub type Runner =
    Arc<dyn Fn(&Path, &Path, &mut dyn FnMut(Progress)) -> Result<Parsed, String> + Send + Sync>;

/// Stage and percent of a running job.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Live {
    pub stage: &'static str,
    pub percent: u8,
}

pub fn stage_name(stage: Stage) -> &'static str {
    match stage {
        Stage::Events => "events",
        Stage::Players => "players",
        Stage::Grenades => "grenades",
        Stage::Assemble => "assemble",
        Stage::Done => "done",
    }
}

struct Inner {
    storage: Arc<Storage>,
    runner: Runner,
    keep_dem: bool,
    permits: Arc<Semaphore>,
    /// Jobs allowed at once: running plus waiting.
    limit: usize,
    in_flight: AtomicUsize,
    live: Mutex<HashMap<String, Live>>,
}

#[derive(Clone)]
pub struct Jobs {
    inner: Arc<Inner>,
}

impl std::fmt::Debug for Jobs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Jobs")
            .field("limit", &self.inner.limit)
            .field("in_flight", &self.in_flight())
            .finish_non_exhaustive()
    }
}

impl Jobs {
    pub fn new(
        storage: Arc<Storage>,
        max_parallel: usize,
        max_queue: usize,
        keep_dem: bool,
        runner: Runner,
    ) -> Jobs {
        Jobs {
            inner: Arc::new(Inner {
                storage,
                runner,
                keep_dem,
                permits: Arc::new(Semaphore::new(max_parallel)),
                limit: max_parallel + max_queue,
                in_flight: AtomicUsize::new(0),
                live: Mutex::new(HashMap::new()),
            }),
        }
    }

    /// The real parser: `parser-core` and a gzip file.
    pub fn parser_runner() -> Runner {
        Arc::new(parse_to_gzip)
    }

    /// Jobs that are running or waiting right now.
    pub fn in_flight(&self) -> usize {
        self.inner.in_flight.load(Ordering::SeqCst)
    }

    /// Whether a new job would be accepted. Only a hint (an upload checks it before reading
    /// the body); [`Jobs::submit`] is the authoritative check.
    pub fn has_capacity(&self) -> bool {
        self.in_flight() < self.inner.limit
    }

    /// Stage and percent of a running job; `None` if it is waiting or finished.
    pub fn live(&self, id: &str) -> Option<Live> {
        self.inner.live.lock().unwrap().get(id).cloned()
    }

    /// Queues a match for parsing. `Err(QueueFull)` when too many jobs exist already.
    /// Must be called inside the tokio runtime.
    pub fn submit(&self, id: String) -> Result<(), ApiError> {
        let reserved = self
            .inner
            .in_flight
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |n| {
                (n < self.inner.limit).then_some(n + 1)
            });
        if reserved.is_err() {
            return Err(ApiError::QueueFull);
        }
        self.spawn(id);
        Ok(())
    }

    /// Queues a match that was interrupted by a restart. It was accepted before, so the
    /// queue limit does not apply.
    pub fn resume(&self, id: String) {
        self.inner.in_flight.fetch_add(1, Ordering::SeqCst);
        self.spawn(id);
    }

    fn spawn(&self, id: String) {
        let inner = self.inner.clone();
        tokio::spawn(async move {
            let _slot = Slot(inner.clone());
            let Ok(_permit) = inner.permits.clone().acquire_owned().await else {
                return;
            };
            inner.process(&id).await;
        });
    }
}

/// Gives the in-flight slot back when the job ends, however it ends.
struct Slot(Arc<Inner>);

impl Drop for Slot {
    fn drop(&mut self) {
        self.0.in_flight.fetch_sub(1, Ordering::SeqCst);
    }
}

impl Inner {
    fn set_live(&self, id: &str, progress: Progress) {
        let live = Live {
            stage: stage_name(progress.stage),
            percent: progress.percent,
        };
        self.live.lock().unwrap().insert(id.to_owned(), live);
    }

    async fn process(self: &Arc<Self>, id: &str) {
        let storage = self.storage.clone();
        let mut meta = match storage.read_meta(id) {
            Ok(Some(meta)) => meta,
            other => {
                tracing::warn!(%id, "job skipped, the match is gone or unreadable: {other:?}");
                return;
            }
        };
        let (Ok(dem), Ok(json)) = (storage.dem_path(id), storage.json_path(id)) else {
            return;
        };

        meta.status = Status::Parsing;
        meta.error = None;
        if let Err(e) = storage.write_meta(&meta) {
            tracing::warn!(%id, "cannot mark the match as parsing: {e}");
            return;
        }
        self.set_live(
            id,
            Progress {
                stage: Stage::Events,
                percent: 0,
            },
        );
        tracing::info!(%id, "parsing started");
        let started = std::time::Instant::now();

        let this = self.clone();
        let job_id = id.to_owned();
        let (dem_for_job, json_for_job) = (dem.clone(), json.clone());
        let joined = tokio::task::spawn_blocking(move || {
            let mut report = |p: Progress| this.set_live(&job_id, p);
            (this.runner)(&dem_for_job, &json_for_job, &mut report)
        })
        .await;
        let result = joined.unwrap_or_else(|e| Err(format!("internal error: {e}")));
        self.live.lock().unwrap().remove(id);

        match result {
            Ok(parsed) => {
                meta.status = Status::Done;
                meta.error = None;
                meta.map = Some(parsed.map);
                meta.schema_version = Some(parsed.schema_version);
                meta.rounds = Some(parsed.rounds);
                meta.score = parsed.score;
                meta.parsed_at = Some(now_unix());
                tracing::info!(%id, seconds = started.elapsed().as_secs_f32(), "parsing done");
            }
            Err(message) if meta.parsed_at.is_some() && json.is_file() => {
                // a failed re-parse must not destroy the result that already exists
                meta.status = Status::Done;
                meta.error = Some(format!("the last re-parse failed: {message}"));
                tracing::warn!(%id, %message, "re-parse failed, the previous result is kept");
            }
            Err(message) => {
                meta.status = Status::Failed;
                meta.error = Some(message.clone());
                let _ = std::fs::remove_file(&json);
                tracing::warn!(%id, %message, "parsing failed");
            }
        }
        if let Err(e) = storage.write_meta(&meta) {
            // for example the match was deleted while it was being parsed
            tracing::warn!(%id, "cannot save the result: {e}");
        }
        if !self.keep_dem {
            let _ = std::fs::remove_file(&dem);
        }
    }
}

/// The real [`Runner`]: parse, then stream the JSON through gzip into `json`.
fn parse_to_gzip(
    dem: &Path,
    json: &Path,
    report: &mut dyn FnMut(Progress),
) -> Result<Parsed, String> {
    let parsed = parser_core::parse_file(dem, Some(report)).map_err(|e| e.to_string())?;

    let mut tmp = json.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
    let written = (|| -> Result<(), String> {
        let file =
            std::fs::File::create(&tmp).map_err(|e| format!("cannot write the result: {e}"))?;
        let mut encoder = GzEncoder::new(BufWriter::new(file), Compression::default());
        parser_core::write_json(&parsed, &mut encoder).map_err(|e| e.to_string())?;
        let mut writer = encoder
            .finish()
            .map_err(|e| format!("cannot write the result: {e}"))?;
        writer
            .flush()
            .map_err(|e| format!("cannot write the result: {e}"))?;
        writer
            .get_ref()
            .sync_all()
            .map_err(|e| format!("cannot write the result: {e}"))?;
        std::fs::rename(&tmp, json).map_err(|e| format!("cannot write the result: {e}"))
    })();
    if let Err(e) = written {
        let _ = std::fs::remove_file(&tmp);
        return Err(e);
    }
    Ok(Parsed {
        score: score(&parsed),
        rounds: parsed.rounds.len() as u32,
        map: parsed.map,
        schema_version: parsed.schema_version,
    })
}

/// Score of the match: `[a, b]`, `a` being the team that started on the T side. Sides swap at
/// halftime, so the team is followed through the players it had in round 1.
fn score(m: &parser_core::Match) -> Option<[u32; 2]> {
    use parser_core::models::Team;
    let first = m.rounds.first()?;
    let anchor = first.teams.iter().position(|t| *t == Some(Team::T))?;
    let mut score = [0, 0];
    for round in &m.rounds {
        let side = (*round.teams.get(anchor)?)?;
        let winner = round.winner?;
        score[usize::from(winner != side)] += 1;
    }
    Some(score)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::Meta;
    use std::sync::atomic::AtomicBool;
    use std::time::Duration;

    fn setup(
        parallel: usize,
        queue: usize,
        keep_dem: bool,
        runner: Runner,
    ) -> (tempfile::TempDir, Arc<Storage>, Jobs) {
        let dir = tempfile::tempdir().unwrap();
        let storage = Arc::new(Storage::open(dir.path().join("data")).unwrap());
        let jobs = Jobs::new(storage.clone(), parallel, queue, keep_dem, runner);
        (dir, storage, jobs)
    }

    fn add_match(storage: &Storage) -> String {
        let meta = Meta::new(Storage::new_id(), None, 8);
        storage.create(&meta).unwrap();
        std::fs::write(storage.dem_path(&meta.id).unwrap(), b"PBDEMS2\0").unwrap();
        meta.id
    }

    async fn wait_for(storage: &Storage, id: &str, want: Status) {
        for _ in 0..500 {
            if storage.read_meta(id).unwrap().unwrap().status == want {
                return;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        panic!("match {id} did not reach {want:?}");
    }

    fn ok_runner() -> Runner {
        Arc::new(|_dem, json, report| {
            report(Progress {
                stage: Stage::Players,
                percent: 30,
            });
            std::fs::write(json, b"gz").unwrap();
            Ok(Parsed {
                map: "de_test".into(),
                schema_version: 1,
                ..Parsed::default()
            })
        })
    }

    #[tokio::test]
    async fn a_job_runs_to_done_and_records_the_result() {
        let (_g, storage, jobs) = setup(1, 4, false, ok_runner());
        let id = add_match(&storage);
        jobs.submit(id.clone()).unwrap();
        wait_for(&storage, &id, Status::Done).await;

        let meta = storage.read_meta(&id).unwrap().unwrap();
        assert_eq!(meta.map.as_deref(), Some("de_test"));
        assert_eq!(meta.schema_version, Some(1));
        assert!(meta.parsed_at.is_some());
        assert_eq!(meta.error, None);
        assert!(storage.json_path(&id).unwrap().is_file());
        assert!(
            !storage.dem_path(&id).unwrap().exists(),
            "demo removed by default"
        );
        assert_eq!(jobs.live(&id), None);
        for _ in 0..100 {
            if jobs.in_flight() == 0 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        assert_eq!(jobs.in_flight(), 0);
    }

    #[tokio::test]
    async fn keep_dem_keeps_the_demo() {
        let (_g, storage, jobs) = setup(1, 4, true, ok_runner());
        let id = add_match(&storage);
        jobs.submit(id.clone()).unwrap();
        wait_for(&storage, &id, Status::Done).await;
        assert!(storage.dem_path(&id).unwrap().is_file());
    }

    #[tokio::test]
    async fn a_failed_parse_is_recorded_and_the_queue_keeps_working() {
        let runner: Runner = Arc::new(|dem, json, report| {
            if std::fs::read(dem).unwrap().ends_with(b"bad") {
                Err("demo parse error: broken".into())
            } else {
                ok_runner()(dem, json, report)
            }
        });
        let (_g, storage, jobs) = setup(1, 4, false, runner);
        let bad = add_match(&storage);
        std::fs::write(storage.dem_path(&bad).unwrap(), b"PBDEMS2\0bad").unwrap();
        let good = add_match(&storage);
        jobs.submit(bad.clone()).unwrap();
        jobs.submit(good.clone()).unwrap();
        wait_for(&storage, &bad, Status::Failed).await;
        wait_for(&storage, &good, Status::Done).await;

        let meta = storage.read_meta(&bad).unwrap().unwrap();
        assert_eq!(meta.error.as_deref(), Some("demo parse error: broken"));
        assert!(!storage.json_path(&bad).unwrap().exists());
    }

    #[tokio::test]
    async fn a_panicking_runner_fails_the_job_but_not_the_server() {
        let runner: Runner = Arc::new(|_, _, _| panic!("boom"));
        let (_g, storage, jobs) = setup(1, 4, false, runner);
        let id = add_match(&storage);
        jobs.submit(id.clone()).unwrap();
        wait_for(&storage, &id, Status::Failed).await;
        let error = storage.read_meta(&id).unwrap().unwrap().error.unwrap();
        assert!(error.starts_with("internal error"), "{error}");
        for _ in 0..100 {
            if jobs.in_flight() == 0 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        assert_eq!(jobs.in_flight(), 0, "the slot is given back");
    }

    #[tokio::test]
    async fn at_most_max_parallel_jobs_parse_at_once() {
        let current = Arc::new(AtomicUsize::new(0));
        let peak = Arc::new(AtomicUsize::new(0));
        let (c, p) = (current.clone(), peak.clone());
        let runner: Runner = Arc::new(move |_dem, json, _report| {
            let now = c.fetch_add(1, Ordering::SeqCst) + 1;
            p.fetch_max(now, Ordering::SeqCst);
            std::thread::sleep(Duration::from_millis(80));
            c.fetch_sub(1, Ordering::SeqCst);
            std::fs::write(json, b"gz").unwrap();
            Ok(Parsed {
                map: "m".into(),
                schema_version: 1,
                ..Parsed::default()
            })
        });
        let (_g, storage, jobs) = setup(2, 8, false, runner);
        let ids: Vec<_> = (0..5).map(|_| add_match(&storage)).collect();
        for id in &ids {
            jobs.submit(id.clone()).unwrap();
        }
        for id in &ids {
            wait_for(&storage, id, Status::Done).await;
        }
        assert_eq!(peak.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn the_queue_limit_refuses_new_jobs_and_frees_up_again() {
        let release = Arc::new(AtomicBool::new(false));
        let r = release.clone();
        let runner: Runner = Arc::new(move |_dem, json, _report| {
            while !r.load(Ordering::SeqCst) {
                std::thread::sleep(Duration::from_millis(5));
            }
            std::fs::write(json, b"gz").unwrap();
            Ok(Parsed {
                map: "m".into(),
                schema_version: 1,
                ..Parsed::default()
            })
        });
        // one parse at a time, one waiting: three jobs would be too many
        let (_g, storage, jobs) = setup(1, 1, false, runner);
        let (a, b, c) = (
            add_match(&storage),
            add_match(&storage),
            add_match(&storage),
        );
        assert!(jobs.has_capacity());
        jobs.submit(a.clone()).unwrap();
        jobs.submit(b.clone()).unwrap();
        assert!(!jobs.has_capacity());
        assert!(matches!(jobs.submit(c.clone()), Err(ApiError::QueueFull)));

        wait_for(&storage, &a, Status::Parsing).await;
        assert_eq!(
            storage.read_meta(&b).unwrap().unwrap().status,
            Status::Queued
        );
        assert_eq!(jobs.live(&a).map(|l| l.stage), Some("events"));

        release.store(true, Ordering::SeqCst);
        wait_for(&storage, &a, Status::Done).await;
        wait_for(&storage, &b, Status::Done).await;
        for _ in 0..100 {
            if jobs.has_capacity() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        jobs.submit(c.clone()).unwrap();
        wait_for(&storage, &c, Status::Done).await;
    }

    #[tokio::test]
    async fn resume_ignores_the_queue_limit() {
        let (_g, storage, jobs) = setup(1, 0, false, ok_runner());
        let ids: Vec<_> = (0..3).map(|_| add_match(&storage)).collect();
        for id in &ids {
            jobs.resume(id.clone());
        }
        for id in &ids {
            wait_for(&storage, id, Status::Done).await;
        }
    }

    #[test]
    fn stage_names() {
        assert_eq!(stage_name(Stage::Events), "events");
        assert_eq!(stage_name(Stage::Done), "done");
    }
}
