use serde::{Deserialize, Serialize};

/// Supported database types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DatabaseType {
    MongoDB,
    Redis,
    PostgreSQL,
}

impl DatabaseType {
    pub fn name(&self) -> &'static str {
        match self {
            DatabaseType::MongoDB => "MongoDB",
            DatabaseType::Redis => "Redis",
            DatabaseType::PostgreSQL => "PostgreSQL",
        }
    }

    pub fn icon_path(&self) -> &'static str {
        match self {
            DatabaseType::MongoDB => "icons/mongodb.svg",
            DatabaseType::Redis => "icons/redis.svg",
            DatabaseType::PostgreSQL => "icons/postgres.svg",
        }
    }

    pub fn default_port(&self) -> u16 {
        match self {
            DatabaseType::MongoDB => 27017,
            DatabaseType::Redis => 6379,
            DatabaseType::PostgreSQL => 5432,
        }
    }

    pub fn all() -> &'static [DatabaseType] {
        &[
            DatabaseType::MongoDB,
            DatabaseType::Redis,
            DatabaseType::PostgreSQL,
        ]
    }
}

/// A database connection configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Connection {
    pub id: String,
    pub name: String,
    pub db_type: DatabaseType,
    pub host: String,
    pub port: u16,
    pub database: Option<String>,
    pub username: Option<String>,
    pub password: Option<String>,
}

impl Connection {
    pub fn new(db_type: DatabaseType) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name: format!("New {} Connection", db_type.name()),
            db_type,
            host: "localhost".to_string(),
            port: db_type.default_port(),
            database: None,
            username: None,
            password: None,
        }
    }
}
