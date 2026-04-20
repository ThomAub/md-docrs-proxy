use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("invalid spec: {0}")]
    InvalidSpec(String),

    #[error("item not found: {0}")]
    NotFound(String),

    #[error("docs.rs fetch failed: {0}")]
    Fetch(String),

    #[error("unsupported rustdoc JSON format_version {got} (this build supports {expected})")]
    FormatVersionMismatch { got: u32, expected: u32 },

    #[error("json parse error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[cfg(feature = "http")]
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),
}
