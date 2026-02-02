use async_trait::async_trait;
use std::time::Duration;

use super::error::{ConnectionError, Result};
use super::connection::DatabaseType;

/// Information returned from a successful connection test
#[derive(Debug, Clone)]
pub struct ConnectionInfo {
    pub server_version: Option<String>,
    pub latency_ms: u64,
}

/// Database information
#[derive(Debug, Clone)]
pub struct DatabaseInfo {
    pub name: String,
    pub size_bytes: Option<u64>,
}

/// Collection information
#[derive(Debug, Clone)]
pub struct CollectionInfo {
    pub name: String,
    pub document_count: Option<u64>,
}

/// Core trait for database connections
#[async_trait]
pub trait DatabaseConnection: Send + Sync {
    /// Test if connection can be established
    async fn test_connection(&self) -> Result<ConnectionInfo>;

    /// List all databases (for MongoDB and similar)
    /// Returns empty list for databases that don't support this operation
    async fn list_databases(&self) -> Result<Vec<DatabaseInfo>> {
        Ok(Vec::new())
    }

    /// List collections in a specific database
    /// Returns empty list for databases that don't support this operation
    #[allow(dead_code)]
    async fn list_collections(&self, database_name: &str) -> Result<Vec<CollectionInfo>> {
        let _ = database_name;
        Ok(Vec::new())
    }
}

/// Configuration for creating a database connection
#[derive(Debug, Clone)]
pub struct ConnectionConfig {
    pub driver: DatabaseType,
    pub connection_string: String,
    pub timeout: Duration,
}

impl ConnectionConfig {
    pub fn new(driver: DatabaseType, connection_string: String) -> Self {
        Self {
            driver,
            connection_string,
            timeout: Duration::from_secs(10),
        }
    }
}

/// Factory function - creates the right connection type based on driver
pub fn create_connection(config: ConnectionConfig) -> Result<Box<dyn DatabaseConnection>> {
    if !config.driver.is_available() {
        return Err(ConnectionError::DriverNotAvailable(config.driver.feature_name()));
    }
    
    match config.driver {
        #[cfg(feature = "postgres")]
        DatabaseType::PostgreSQL => {
            Ok(Box::new(super::drivers::postgres::PostgresConnection::new(config)?))
        }
        
        #[cfg(feature = "mongodb")]
        DatabaseType::MongoDB => {
            Ok(Box::new(super::drivers::mongo::MongoConnection::new(config)?))
        }
        
        #[cfg(feature = "redis")]
        DatabaseType::Redis => {
            Ok(Box::new(super::drivers::redis_driver::RedisConnection::new(config)?))
        }
        
        #[cfg(feature = "mysql")]
        DatabaseType::MySQL => {
            Ok(Box::new(super::drivers::mysql::MySqlConnection::new(config)?))
        }
        
        #[cfg(feature = "sqlite-driver")]
        DatabaseType::SQLite => {
            Ok(Box::new(super::drivers::sqlite::SqliteConnection::new(config)?))
        }
        
        // Fallback for when feature not compiled
        #[allow(unreachable_patterns)]
        _ => Err(ConnectionError::DriverNotAvailable(config.driver.feature_name())),
    }
}
