//! Shared state handed to every handler. Later stages add the storage and the job queue.

use std::sync::Arc;

use crate::config::Config;

#[derive(Clone, Debug)]
pub struct AppState {
    pub config: Arc<Config>,
}

impl AppState {
    pub fn new(config: Config) -> AppState {
        AppState {
            config: Arc::new(config),
        }
    }
}
