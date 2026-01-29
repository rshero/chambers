use std::time::Duration;
use thiserror::Error;

/// Errors that can occur during database operations
#[derive(Error, Debug)]
pub enum ConnectionError {
    #[error("Connection failed: {0}")]
    Failed(String),
    #[error("Authentication failed")]
    #[allow(dead_code)]
    AuthFailed,
    #[error("Connection timeout after {0:?}")]
    Timeout(Duration),
    #[error("Invalid connection string: {0}")]
    InvalidConnectionString(String),
    #[error("Driver not available: {0} (not compiled)")]
    DriverNotAvailable(&'static str),
}

pub type Result<T> = std::result::Result<T, ConnectionError>;
