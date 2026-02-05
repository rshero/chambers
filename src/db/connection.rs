use serde::{Deserialize, Serialize};

/// Supported database types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DatabaseType {
    MongoDB,
    Redis,
    PostgreSQL,
    MySQL,
    SQLite,
}

impl DatabaseType {
    pub fn name(&self) -> &'static str {
        match self {
            DatabaseType::MongoDB => "MongoDB",
            DatabaseType::Redis => "Redis",
            DatabaseType::PostgreSQL => "PostgreSQL",
            DatabaseType::MySQL => "MySQL",
            DatabaseType::SQLite => "SQLite",
        }
    }

    pub fn icon_path(&self) -> &'static str {
        match self {
            DatabaseType::MongoDB => "icons/mongodb.svg",
            DatabaseType::Redis => "icons/redis.svg",
            DatabaseType::PostgreSQL => "icons/postgres.svg",
            DatabaseType::MySQL => "icons/mysql.svg",
            DatabaseType::SQLite => "icons/sqlite.svg",
        }
    }

    pub fn default_port(&self) -> u16 {
        match self {
            DatabaseType::MongoDB => 27017,
            DatabaseType::Redis => 6379,
            DatabaseType::PostgreSQL => 5432,
            DatabaseType::MySQL => 3306,
            DatabaseType::SQLite => 0, // No port for SQLite
        }
    }

    /// Connection string scheme/prefix
    pub fn scheme(&self) -> &'static str {
        match self {
            DatabaseType::MongoDB => "mongodb://",
            DatabaseType::Redis => "redis://",
            DatabaseType::PostgreSQL => "postgresql://",
            DatabaseType::MySQL => "mysql://",
            DatabaseType::SQLite => "", // File path
        }
    }

    /// Feature name for error messages
    pub fn feature_name(&self) -> &'static str {
        match self {
            DatabaseType::PostgreSQL => "postgres",
            DatabaseType::MongoDB => "mongodb",
            DatabaseType::Redis => "redis",
            DatabaseType::MySQL => "mysql",
            DatabaseType::SQLite => "sqlite-driver",
        }
    }

    /// Check if this driver was compiled in
    pub fn is_available(&self) -> bool {
        match self {
            DatabaseType::PostgreSQL => cfg!(feature = "postgres"),
            DatabaseType::MongoDB => cfg!(feature = "mongodb"),
            DatabaseType::Redis => cfg!(feature = "redis"),
            DatabaseType::MySQL => cfg!(feature = "mysql"),
            DatabaseType::SQLite => cfg!(feature = "sqlite-driver"),
        }
    }

    /// All database types (for UI listing)
    pub fn all() -> &'static [DatabaseType] {
        &[
            DatabaseType::PostgreSQL,
            DatabaseType::MongoDB,
            DatabaseType::Redis,
            DatabaseType::MySQL,
            DatabaseType::SQLite,
        ]
    }

    /// Only available (compiled) database types
    #[allow(dead_code)]
    pub fn available() -> Vec<DatabaseType> {
        Self::all()
            .iter()
            .filter(|dt| dt.is_available())
            .copied()
            .collect()
    }
}

/// A database connection configuration (stored in app database)
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
    /// Optional connection string that overrides individual fields
    pub connection_string: Option<String>,
    /// Visible databases in the picker (for MongoDB connections)
    /// Stored as JSON array, remembered across app restarts
    pub visible_databases: Option<Vec<String>>,
    /// Whether "Show All" is enabled in the database picker
    pub show_all_databases: Option<bool>,
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
            connection_string: None,
            visible_databases: None,
            show_all_databases: None,
        }
    }

    /// Build connection string from fields, or return custom one if set
    pub fn get_connection_string(&self) -> String {
        // If custom connection string is set, use it
        if let Some(ref conn_str) = self.connection_string {
            if !conn_str.is_empty() {
                return conn_str.clone();
            }
        }

        // Build from individual fields
        match self.db_type {
            DatabaseType::SQLite => {
                // For SQLite, database field is the file path
                self.database
                    .clone()
                    .unwrap_or_else(|| ":memory:".to_string())
            }
            _ => {
                let mut url = String::from(self.db_type.scheme());

                // Add credentials if present
                if let Some(ref user) = self.username {
                    url.push_str(user);
                    if let Some(ref pass) = self.password {
                        url.push(':');
                        url.push_str(pass);
                    }
                    url.push('@');
                }

                // Add host:port
                url.push_str(&self.host);
                if self.port > 0 {
                    url.push(':');
                    url.push_str(&self.port.to_string());
                }

                // Add database name
                if let Some(ref db) = self.database {
                    url.push('/');
                    url.push_str(db);
                }

                url
            }
        }
    }
}
