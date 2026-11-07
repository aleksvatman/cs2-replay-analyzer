//! Shared state handed to every handler.

use std::io;
use std::sync::Arc;

use tokio::sync::Mutex;

use crate::config::Config;
use crate::jobs::{Jobs, Runner};
use crate::storage::Storage;

#[derive(Clone, Debug)]
pub struct AppState {
    pub config: Arc<Config>,
    pub storage: Arc<Storage>,
    pub jobs: Jobs,
    /// Held while an upload is finalized (duplicate check, rename, `meta.json`, queueing),
    /// so two identical uploads at once cannot both create a match.
    pub upload_lock: Arc<Mutex<()>>,
}

impl AppState {
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
            config: Arc::new(config),
            storage,
            jobs,
            upload_lock: Arc::new(Mutex::new(())),
        })
    }
}
