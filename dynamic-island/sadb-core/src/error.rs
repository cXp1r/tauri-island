use thiserror::Error;

/// Core error type for sadb operations
#[derive(Error, Debug)]
pub enum Error {
    #[error("ADB command failed: {0}")]
    Adb(String),

    #[error("Protocol error: {0}")]
    Protocol(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("UTF-8 error: {0}")]
    Utf8(#[from] std::string::FromUtf8Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Server not started")]
    ServerNotStarted,

    #[error("Connection closed")]
    ConnectionClosed,

    #[error("Invalid packet format")]
    InvalidPacket,
}

pub type Result<T> = std::result::Result<T, Error>;
