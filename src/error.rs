use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("could not read tags from {path}: {message}")]
    Tags { path: PathBuf, message: String },

    #[error("not a directory: {0}")]
    NotADirectory(PathBuf),

    #[error("decode error: {0}")]
    Decode(String),

    #[error("audio output error: {0}")]
    Audio(String),

    #[error("lyrics error: {0}")]
    Lyrics(String),
}

pub type Result<T> = std::result::Result<T, Error>;

impl From<symphonia::core::errors::Error> for Error {
    fn from(err: symphonia::core::errors::Error) -> Self {
        Error::Decode(err.to_string())
    }
}
