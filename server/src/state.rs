//! Shared state handed to every handler. Later stages add the job queue.

use std::io;
use std::sync::Arc;

use crate::config::Config;
use crate::storage::Storage;

#[derive(Clone, Debug)]
pub struct AppState {
    pub config: Arc<Config>,
    pub storage: Arc<Storage>,
}

impl AppState {
    /// Opens the storage in `config.data_dir`, creating the directory if needed.
    pub fn new(config: Config) -> io::Result<AppState> {
        let storage = Storage::open(&config.data_dir)?;
        Ok(AppState {
            config: Arc::new(config),
            storage: Arc::new(storage),
        })
    }
}
