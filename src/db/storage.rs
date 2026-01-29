use anyhow::Result;
use rusqlite::{params, Connection as SqliteConnection};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use super::connection::{Connection, DatabaseType};

/// SQLite-based storage for database connections
pub struct ConnectionStorage {
    conn: Arc<Mutex<SqliteConnection>>,
}

impl ConnectionStorage {
    /// Create a new storage instance, initializing the database if needed
    pub fn new() -> Result<Self> {
        let db_path = Self::db_path()?;

        // Ensure parent directory exists
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let conn = SqliteConnection::open(&db_path)?;

        // Create tables if they don't exist
        conn.execute(
            "CREATE TABLE IF NOT EXISTS connections (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                db_type TEXT NOT NULL,
                host TEXT NOT NULL,
                port INTEGER NOT NULL,
                database TEXT,
                username TEXT,
                password TEXT,
                connection_string TEXT
            )",
            [],
        )?;

        // Migration: add connection_string column if it doesn't exist
        let has_conn_str: bool = conn
            .prepare("SELECT connection_string FROM connections LIMIT 1")
            .is_ok();
        if !has_conn_str {
            conn.execute(
                "ALTER TABLE connections ADD COLUMN connection_string TEXT",
                [],
            )
            .ok(); // Ignore error if column already exists
        }

        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    fn db_path() -> Result<PathBuf> {
        let data_dir = dirs::data_local_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not find local data directory"))?;
        Ok(data_dir.join("chambers").join("connections.db"))
    }

    /// Get all saved connections
    pub fn get_all(&self) -> Result<Vec<Connection>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, name, db_type, host, port, database, username, password, connection_string FROM connections",
        )?;

        let connections = stmt
            .query_map([], |row| {
                let db_type_str: String = row.get(2)?;
                let db_type = match db_type_str.as_str() {
                    "MongoDB" => DatabaseType::MongoDB,
                    "Redis" => DatabaseType::Redis,
                    "PostgreSQL" => DatabaseType::PostgreSQL,
                    "MySQL" => DatabaseType::MySQL,
                    "SQLite" => DatabaseType::SQLite,
                    _ => DatabaseType::PostgreSQL,
                };

                Ok(Connection {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    db_type,
                    host: row.get(3)?,
                    port: row.get(4)?,
                    database: row.get(5)?,
                    username: row.get(6)?,
                    password: row.get(7)?,
                    connection_string: row.get(8)?,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(connections)
    }

    /// Save a connection (insert or update)
    pub fn save(&self, connection: &Connection) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO connections (id, name, db_type, host, port, database, username, password, connection_string)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                connection.id,
                connection.name,
                connection.db_type.name(),
                connection.host,
                connection.port,
                connection.database,
                connection.username,
                connection.password,
                connection.connection_string,
            ],
        )?;
        Ok(())
    }

    /// Delete a connection by ID
    #[allow(dead_code)]
    pub fn delete(&self, id: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM connections WHERE id = ?1", params![id])?;
        Ok(())
    }
}
