//! Database module - connection management and drivers

pub mod connection;
pub mod driver;
pub mod drivers;
pub mod error;
pub mod storage;

pub use connection::{Connection, DatabaseType};
pub use driver::{create_connection, ConnectionConfig};
pub use storage::ConnectionStorage;
