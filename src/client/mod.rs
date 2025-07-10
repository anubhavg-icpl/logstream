//! LogStream client implementation

pub mod logger;

#[cfg(feature = "journald")]
pub mod journald;

pub use logger::LogClient;
pub use crate::types::LogLevel;
