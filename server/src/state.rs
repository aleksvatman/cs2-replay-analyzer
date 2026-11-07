//! Shared state handed to every handler.

use std::io;
use std::sync::Arc;

use tokio::sync::{Mutex, Semaphore};

use crate::config::Config;
use crate::jobs::{Jobs, Runner};
use crate::limits::RateLimiter;
use crate::storage::{Recovery, Storage, StorageError};

#[derive(Clone, Debug)]
pub struct AppState {
    pub config: Arc<Config>,
    pub storage: Arc<Storage>,
    pub jobs: Jobs,
    /// Held while an upload is finalized (duplicate check, rename, `meta.json`, queueing),
    /// so two identical uploads at once cannot both create a match.
    pub upload_lock: Arc<Mutex<()>>,
    /// One permit per upload being received (`MAX_UPLOADS`).
    pub upload_slots: Arc<Semaphore>,
    pub limiter: Arc<RateLimiter>,
}

impl AppState {
    /// Startup scan of the data directory: removes unfinished uploads, settles interrupted
    /// matches and puts the ones that still have their demo back into the parse queue.
    /// Must be called inside the tokio runtime.
    pub fn recover_and_resume(&self) -> Result<Recovery, StorageError> {
        let recovery = self.storage.recover()?;
        for id in recovery.requeue.iter().cloned() {
            self.jobs.resume(id);
        }
        Ok(recovery)
    }

    /// Opens the storage in `config.data_dir` (creating it if needed) and sets up the parse
    /// queue with the real parser.
    pub fn new(config: Config) -> io::Result<AppState> {
        AppState::with_runner(config, Jobs::parser_runner())
    }

    /// Same, with another parse function (tests replace the heavy parser).
    pub fn with_runner(config: Config, runner: Runner) -> io::Result<AppState> {
        let storage = Arc::new(Storage::open(&config.data_dir)?);
        let jobs = Jobs::new(
            storage.clone(),
            config.max_parallel_parses,
            config.max_queue,
            config.keep_dem,
            runner,
        );
        Ok(AppState {
            upload_slots: Arc::new(Semaphore::new(config.max_uploads)),
            limiter: Arc::new(RateLimiter::new(config.rate_limit_per_minute)),
            config: Arc::new(config),
            storage,
            jobs,
            upload_lock: Arc::new(Mutex::new(())),
        })
    }
}
