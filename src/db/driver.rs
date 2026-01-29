use async_trait::async_trait;
use std::time::Duration;

use super::error::{ConnectionError, Result};
use super::connection::DatabaseType;

/// Information returned from a successful connection test
#[derive(Debug, Clone)]
pub struct ConnectionInfo {
    pub server_version: Option<String>,
    pub latency_ms: u64,
    #[allow(dead_code)]
    pub database_name: Option<String>,
}

/// Core trait for database connections
#[async_trait]
pub trait DatabaseConnection: Send + Sync {
    /// Test if connection can be established
    async fn test_connection(&self) -> Result<ConnectionInfo>;

    /// Get driver type
    #[allow(dead_code)]
    fn driver(&self) -> DatabaseType;
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
    
    #[allow(dead_code)]
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
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
