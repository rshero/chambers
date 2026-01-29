//! MySQL driver implementation

use async_trait::async_trait;
use mysql_async::prelude::*;
use std::time::Instant;

use crate::db::connection::DatabaseType;
use crate::db::driver::{ConnectionConfig, ConnectionInfo, DatabaseConnection};
use crate::db::error::{ConnectionError, Result};

pub struct MySqlConnection {
    config: ConnectionConfig,
}

impl MySqlConnection {
    pub fn new(config: ConnectionConfig) -> Result<Self> {
        let conn_str = &config.connection_string;
        if !conn_str.starts_with("mysql://") {
            return Err(ConnectionError::InvalidConnectionString(
                "MySQL connection string must start with mysql://".into(),
            ));
        }
        Ok(Self { config })
    }
}

#[async_trait]
impl DatabaseConnection for MySqlConnection {
    async fn test_connection(&self) -> Result<ConnectionInfo> {
        let start = Instant::now();

        // Parse connection string into options
        let opts = mysql_async::Opts::from_url(&self.config.connection_string)
            .map_err(|e| ConnectionError::InvalidConnectionString(e.to_string()))?;

        let db_name = opts.db_name().map(String::from);

        // Create pool with single connection
        let pool = mysql_async::Pool::new(opts);

        // Get connection with timeout
        let mut conn = tokio::time::timeout(self.config.timeout, pool.get_conn())
            .await
            .map_err(|_| ConnectionError::Timeout(self.config.timeout))?
            .map_err(|e| ConnectionError::Failed(e.to_string()))?;

        // Get server version
        let version: Option<String> = tokio::time::timeout(
            self.config.timeout,
            conn.query_first("SELECT VERSION()"),
        )
        .await
        .map_err(|_| ConnectionError::Timeout(self.config.timeout))?
        .map_err(|e| ConnectionError::Failed(e.to_string()))?;

        let latency = start.elapsed().as_millis() as u64;

        // Clean up
        drop(conn);
        pool.disconnect().await.ok();

        Ok(ConnectionInfo {
            server_version: version.map(|v| format!("MySQL {}", v)),
            latency_ms: latency,
            database_name: db_name,
        })
    }

    fn driver(&self) -> DatabaseType {
        DatabaseType::MySQL
    }
}
