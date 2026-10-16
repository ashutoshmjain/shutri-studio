//! Shutri Studio core library.
//!
//! Canonical artifact is the narrative (conversation + word timing).
//! Everything else — KaTeX article, audio, clips, channels — derives from it.

pub mod bundle;
pub mod channels;
pub mod episodes;
pub mod git;
pub mod jobs;
pub mod media;
pub mod narrative;
pub mod nostr;
pub mod plan;
pub mod project;
pub mod transcribe;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("{0}")]
    Msg(String),
}

pub type Result<T> = std::result::Result<T, CoreError>;

pub fn msg<T>(s: impl Into<String>) -> Result<T> {
    Err(CoreError::Msg(s.into()))
}
