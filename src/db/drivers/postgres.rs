//! PostgreSQL driver implementation

use async_trait::async_trait;
use std::time::Instant;
use tokio_postgres::NoTls;

use crate::db::driver::{ConnectionConfig, ConnectionInfo, DatabaseConnection};
use crate::db::error::{ConnectionError, Result};

pub struct PostgresConnection {
    config: ConnectionConfig,
}

impl PostgresConnection {
    pub fn new(config: ConnectionConfig) -> Result<Self> {
        // Basic validation
        let conn_str = &config.connection_string;
        if !conn_str.starts_with("postgres://") && !conn_str.starts_with("postgresql://") {
            return Err(ConnectionError::InvalidConnectionString(
                "PostgreSQL connection string must start with postgres:// or postgresql://".into(),
            ));
        }
        Ok(Self { config })
    }
}

#[async_trait]
impl DatabaseConnection for PostgresConnection {
    async fn test_connection(&self) -> Result<ConnectionInfo> {
        let start = Instant::now();

        // Connect with timeout
        let connect_future = tokio_postgres::connect(&self.config.connection_string, NoTls);
        
        let (client, connection) = tokio::time::timeout(self.config.timeout, connect_future)
            .await
            .map_err(|_| ConnectionError::Timeout(self.config.timeout))?
            .map_err(|e| ConnectionError::Failed(e.to_string()))?;

        // Spawn connection handler (required by tokio-postgres)
        tokio::spawn(async move {
            if let Err(e) = connection.await {
                eprintln!("PostgreSQL connection error: {}", e);
            }
        });

        // Get server version
        let row = client
            .query_one("SELECT version()", &[])
            .await
            .map_err(|e| ConnectionError::Failed(e.to_string()))?;

        let version: String = row.get(0);
        let latency = start.elapsed().as_millis() as u64;

        Ok(ConnectionInfo {
            server_version: Some(version),
            latency_ms: latency,
        })
    }
}
