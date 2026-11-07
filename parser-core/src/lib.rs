//! parser-core: CS2 .dem -> compact JSON for the 2D replay.

pub mod error;
pub mod extract;
pub mod models;
pub mod parse_demo;
pub mod stats;

pub use error::ParseError;
