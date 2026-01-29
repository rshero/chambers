//! MongoDB driver implementation

use async_trait::async_trait;
use mongodb::{options::ClientOptions, Client};
use std::time::Instant;

use crate::db::connection::DatabaseType;
use crate::db::driver::{ConnectionConfig, ConnectionInfo, DatabaseConnection};
use crate::db::error::{ConnectionError, Result};

pub struct MongoConnection {
    config: ConnectionConfig,
}

impl MongoConnection {
    pub fn new(config: ConnectionConfig) -> Result<Self> {
        let conn_str = &config.connection_string;
        if !conn_str.starts_with("mongodb://") && !conn_str.starts_with("mongodb+srv://") {
            return Err(ConnectionError::InvalidConnectionString(
                "MongoDB connection string must start with mongodb:// or mongodb+srv://".into(),
            ));
        }
        Ok(Self { config })
    }
}

#[async_trait]
impl DatabaseConnection for MongoConnection {
    async fn test_connection(&self) -> Result<ConnectionInfo> {
        let start = Instant::now();

        // Parse connection string
        let mut client_options = tokio::time::timeout(
            self.config.timeout,
            ClientOptions::parse(&self.config.connection_string),
        )
        .await
        .map_err(|_| ConnectionError::Timeout(self.config.timeout))?
        .map_err(|e| ConnectionError::InvalidConnectionString(e.to_string()))?;

        // Set connection timeout
        client_options.connect_timeout = Some(self.config.timeout);
        client_options.server_selection_timeout = Some(self.config.timeout);

        // Create client
        let client =
            Client::with_options(client_options).map_err(|e| ConnectionError::Failed(e.to_string()))?;

        // Ping the server
        let db = client.database("admin");
        tokio::time::timeout(
            self.config.timeout,
            db.run_command(mongodb::bson::doc! { "ping": 1 }),
        )
        .await
        .map_err(|_| ConnectionError::Timeout(self.config.timeout))?
        .map_err(|e| ConnectionError::Failed(e.to_string()))?;

        // Get server info
        let server_info = tokio::time::timeout(
            self.config.timeout,
            db.run_command(mongodb::bson::doc! { "buildInfo": 1 }),
        )
        .await
        .map_err(|_| ConnectionError::Timeout(self.config.timeout))?
        .map_err(|e| ConnectionError::Failed(e.to_string()))?;

        let version = server_info
            .get_str("version")
            .ok()
            .map(|v| format!("MongoDB {}", v));

        let latency = start.elapsed().as_millis() as u64;

        // Extract database name from connection string
        let db_name = url::Url::parse(&self.config.connection_string)
            .ok()
            .and_then(|u| {
                let path = u.path();
                if path.len() > 1 {
                    Some(path[1..].to_string())
                } else {
                    None
                }
            });

        Ok(ConnectionInfo {
            server_version: version,
            latency_ms: latency,
            database_name: db_name,
        })
    }

    fn driver(&self) -> DatabaseType {
        DatabaseType::MongoDB
    }
}
