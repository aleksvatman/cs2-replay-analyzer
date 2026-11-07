//! Shared state handed to every handler. Later stages add the job queue.

use std::io;
use std::sync::Arc;

use tokio::sync::Mutex;

use crate::config::Config;
use crate::storage::Storage;

#[derive(Clone, Debug)]
pub struct AppState {
    pub config: Arc<Config>,
    pub storage: Arc<Storage>,
    /// Held while an upload is finalized (duplicate check, rename, `meta.json`), so two
    /// identical uploads at once cannot both create a match.
    pub upload_lock: Arc<Mutex<()>>,
}

impl AppState {
    /// Opens the storage in `config.data_dir`, creating the directory if needed.
    pub fn new(config: Config) -> io::Result<AppState> {
        let storage = Storage::open(&config.data_dir)?;
        Ok(AppState {
            config: Arc::new(config),
            storage: Arc::new(storage),
            upload_lock: Arc::new(Mutex::new(())),
        })
    }
}
