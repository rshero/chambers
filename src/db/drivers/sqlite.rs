//! SQLite driver implementation

use async_trait::async_trait;
use std::path::Path;
use std::time::Instant;

use crate::db::driver::{ConnectionConfig, ConnectionInfo, DatabaseConnection};
use crate::db::error::{ConnectionError, Result};

pub struct SqliteConnection {
    config: ConnectionConfig,
}

impl SqliteConnection {
    pub fn new(config: ConnectionConfig) -> Result<Self> {
        // For SQLite, connection string is just a file path or :memory:
        let path = &config.connection_string;
        if path != ":memory:" && !path.is_empty() {
            // Check if parent directory exists for file paths
            if let Some(parent) = Path::new(path).parent() {
                if !parent.as_os_str().is_empty() && !parent.exists() {
                    return Err(ConnectionError::InvalidConnectionString(format!(
                        "Parent directory does not exist: {}",
                        parent.display()
                    )));
                }
            }
        }
        Ok(Self { config })
    }
}

#[async_trait]
impl DatabaseConnection for SqliteConnection {
    async fn test_connection(&self) -> Result<ConnectionInfo> {
        let start = Instant::now();
        let path = self.config.connection_string.clone();

        // SQLite is synchronous, so we run it in a blocking task
        let result = tokio::task::spawn_blocking(move || {
            let conn = rusqlite::Connection::open(&path)
                .map_err(|e| ConnectionError::Failed(e.to_string()))?;

            // Get SQLite version
            let version: String = conn
                .query_row("SELECT sqlite_version()", [], |row| row.get(0))
                .map_err(|e| ConnectionError::Failed(e.to_string()))?;

            Ok::<_, ConnectionError>(version)
        })
        .await
        .map_err(|e| ConnectionError::Failed(e.to_string()))??;

        let latency = start.elapsed().as_millis() as u64;

        Ok(ConnectionInfo {
            server_version: Some(format!("SQLite {}", result)),
            latency_ms: latency,
        })
    }
}
