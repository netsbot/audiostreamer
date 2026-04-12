use thiserror::Error;
use tokio::sync::AcquireError;

pub type Result<T> = std::result::Result<T, StreamerError>;

#[derive(Debug, Error)]
pub enum StreamerError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("config parse error: {0}")]
    Toml(#[from] toml::de::Error),

    #[error("network error: {0}")]
    Reqwest(#[from] reqwest::Error),

    #[error("regex error: {0}")]
    Regex(#[from] regex::Error),

    #[error("invalid header value: {0}")]
    InvalidHeaderValue(#[from] reqwest::header::InvalidHeaderValue),

    #[error("invalid config: {0}")]
    InvalidConfig(String),

    #[error("unsupported operation: {0}")]
    Unsupported(String),

    #[error("{0}")]
    Message(String),

    #[error("ffmpeg error: {0}")]
    Ffmpeg(#[from] ffmpeg_next::Error),

    #[error("semaphore acquisition error: {0}")]
    Acquire(#[from] AcquireError),
}