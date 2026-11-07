//! parser-core: CS2 .dem -> compact JSON for the 2D replay.
//!
//! ```no_run
//! let json = parser_core::parse_file_to_json(std::path::Path::new("match.dem"), None)?;
//! # Ok::<(), parser_core::ParseError>(())
//! ```
//!
//! The API is synchronous and CPU-heavy: the caller picks the thread. There is no networking
//! and no platform code, so the same crate builds for the server and for iOS.

pub mod error;
pub mod extract;
pub mod models;
pub mod parse_demo;
pub mod stats;

pub use error::ParseError;
pub use models::Match;
pub use parse_demo::{Progress, Stage};

use std::path::Path;

/// Optional progress callback (see `Progress`; reported at step boundaries, not continuously).
pub type ProgressFn<'a> = &'a mut dyn FnMut(Progress);

/// Source 2 demo files start with this magic.
const DEMO_MAGIC: &[u8] = b"PBDEMS2\0";

/// Parses a demo held in memory.
///
/// Invalid input returns `Err`. The demoparser2 core can panic on malformed data (it indexes
/// slices unchecked), so panics are caught and turned into `ParseError::Demo`; this relies on
/// unwinding, so builds that embed the crate (e.g. `ios-bridge`) must not use `panic = "abort"`.
pub fn parse_bytes(data: &[u8], progress: Option<ProgressFn>) -> Result<Match, ParseError> {
    if !data.starts_with(DEMO_MAGIC) {
        return Err(ParseError::Demo("not a CS2 demo (bad header magic)".into()));
    }
    let run = || match progress {
        Some(cb) => parse_demo::parse(data, cb),
        None => parse_demo::parse(data, &mut |_| {}),
    };
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(run)).unwrap_or_else(|payload| {
        let msg = payload
            .downcast_ref::<&str>()
            .map(|s| s.to_string())
            .or_else(|| payload.downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "unknown panic".into());
        Err(ParseError::Demo(format!("demo parser panicked: {msg}")))
    })
}

/// Parses a demo file. The file is memory-mapped, so the OS can evict its pages under memory
/// pressure instead of the process holding the whole demo in RAM. The file must not be
/// modified while parsing.
pub fn parse_file(path: &Path, progress: Option<ProgressFn>) -> Result<Match, ParseError> {
    let file = std::fs::File::open(path)?;
    // Safety: read-only mapping; the documented contract is that the file is not modified.
    let map = unsafe { memmap2::MmapOptions::new().map(&file)? };
    parse_bytes(&map, progress)
}

/// Compact JSON (no whitespace), the format described in `docs/format.md`.
pub fn to_json(m: &Match) -> Result<String, ParseError> {
    serde_json::to_string(m).map_err(|e| ParseError::Demo(format!("json: {e}")))
}

/// Writes compact JSON to `out` without building the string in memory first.
pub fn write_json<W: std::io::Write>(m: &Match, out: W) -> Result<(), ParseError> {
    serde_json::to_writer(out, m).map_err(|e| ParseError::Demo(format!("json: {e}")))
}

/// `parse_file` followed by `to_json`.
pub fn parse_file_to_json(path: &Path, progress: Option<ProgressFn>) -> Result<String, ParseError> {
    to_json(&parse_file(path, progress)?)
}
