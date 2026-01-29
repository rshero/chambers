//! Redis driver implementation

use async_trait::async_trait;
use std::time::Instant;

use crate::db::connection::DatabaseType;
use crate::db::driver::{ConnectionConfig, ConnectionInfo, DatabaseConnection};
use crate::db::error::{ConnectionError, Result};

pub struct RedisConnection {
    config: ConnectionConfig,
}

impl RedisConnection {
    pub fn new(config: ConnectionConfig) -> Result<Self> {
        let conn_str = &config.connection_string;
        if !conn_str.starts_with("redis://") && !conn_str.starts_with("rediss://") {
            return Err(ConnectionError::InvalidConnectionString(
                "Redis connection string must start with redis:// or rediss://".into(),
            ));
        }
        Ok(Self { config })
    }
}

#[async_trait]
impl DatabaseConnection for RedisConnection {
    async fn test_connection(&self) -> Result<ConnectionInfo> {
        let start = Instant::now();

        // Create client
        let client = redis::Client::open(self.config.connection_string.as_str())
            .map_err(|e| ConnectionError::InvalidConnectionString(e.to_string()))?;

        // Get async connection with timeout
        let mut conn = tokio::time::timeout(
            self.config.timeout,
            client.get_multiplexed_async_connection(),
        )
        .await
        .map_err(|_| ConnectionError::Timeout(self.config.timeout))?
        .map_err(|e| ConnectionError::Failed(e.to_string()))?;

        // Ping
        let _: String = tokio::time::timeout(self.config.timeout, redis::cmd("PING").query_async(&mut conn))
            .await
            .map_err(|_| ConnectionError::Timeout(self.config.timeout))?
            .map_err(|e| ConnectionError::Failed(e.to_string()))?;

        // Get server info
        let info: String = tokio::time::timeout(self.config.timeout, redis::cmd("INFO").arg("server").query_async(&mut conn))
            .await
            .map_err(|_| ConnectionError::Timeout(self.config.timeout))?
            .map_err(|e| ConnectionError::Failed(e.to_string()))?;

        // Parse version from INFO output
        let version = info
            .lines()
            .find(|line| line.starts_with("redis_version:"))
            .map(|line| format!("Redis {}", line.trim_start_matches("redis_version:")));

        let latency = start.elapsed().as_millis() as u64;

        Ok(ConnectionInfo {
            server_version: version,
            latency_ms: latency,
            database_name: None, // Redis doesn't have named databases
        })
    }

    fn driver(&self) -> DatabaseType {
        DatabaseType::Redis
    }
}
