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
                connection_string TEXT,
                visible_databases TEXT,
                show_all_databases INTEGER
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

        // Migration: add visible_databases column if it doesn't exist
        let has_visible_dbs: bool = conn
            .prepare("SELECT visible_databases FROM connections LIMIT 1")
            .is_ok();
        if !has_visible_dbs {
            conn.execute(
                "ALTER TABLE connections ADD COLUMN visible_databases TEXT",
                [],
            )
            .ok();
        }

        // Migration: add show_all_databases column if it doesn't exist
        let has_show_all: bool = conn
            .prepare("SELECT show_all_databases FROM connections LIMIT 1")
            .is_ok();
        if !has_show_all {
            conn.execute(
                "ALTER TABLE connections ADD COLUMN show_all_databases INTEGER",
                [],
            )
            .ok();
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
            "SELECT id, name, db_type, host, port, database, username, password, connection_string, visible_databases, show_all_databases FROM connections",
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

                // Parse visible_databases from JSON
                let visible_databases_json: Option<String> = row.get(9)?;
                let visible_databases: Option<Vec<String>> =
                    visible_databases_json.and_then(|json| serde_json::from_str(&json).ok());

                // Parse show_all_databases from integer
                let show_all_int: Option<i32> = row.get(10)?;
                let show_all_databases = show_all_int.map(|v| v != 0);

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
                    visible_databases,
                    show_all_databases,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(connections)
    }

    /// Save a connection (insert or update)
    pub fn save(&self, connection: &Connection) -> Result<()> {
        let conn = self.conn.lock().unwrap();

        // Serialize visible_databases to JSON
        let visible_databases_json: Option<String> = connection
            .visible_databases
            .as_ref()
            .map(|dbs| serde_json::to_string(dbs).unwrap_or_default());

        // Convert show_all_databases to integer
        let show_all_int: Option<i32> =
            connection.show_all_databases.map(|b| if b { 1 } else { 0 });

        conn.execute(
            "INSERT OR REPLACE INTO connections (id, name, db_type, host, port, database, username, password, connection_string, visible_databases, show_all_databases)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
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
                visible_databases_json,
                show_all_int,
            ],
        )?;
        Ok(())
    }

    /// Delete a connection by ID
    pub fn delete(&self, id: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM connections WHERE id = ?1", params![id])?;
        Ok(())
    }

    /// Update visible databases and show_all setting for a connection
    pub fn update_visible_databases(
        &self,
        connection_id: &str,
        visible_databases: &[String],
        show_all: bool,
    ) -> Result<()> {
        let conn = self.conn.lock().unwrap();

        // Serialize visible_databases to JSON
        let visible_databases_json = serde_json::to_string(visible_databases).unwrap_or_default();
        let show_all_int: i32 = if show_all { 1 } else { 0 };

        conn.execute(
            "UPDATE connections SET visible_databases = ?1, show_all_databases = ?2 WHERE id = ?3",
            params![visible_databases_json, show_all_int, connection_id],
        )?;
        Ok(())
    }
}
