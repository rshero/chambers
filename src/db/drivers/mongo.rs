//! MongoDB driver implementation

use async_trait::async_trait;
use futures::TryStreamExt;
use mongodb::{bson::Document, options::ClientOptions, Client};
use std::time::Instant;

use crate::db::driver::{CollectionInfo, ConnectionConfig, ConnectionInfo, DatabaseConnection, DatabaseInfo};
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

        Ok(ConnectionInfo {
            server_version: version,
            latency_ms: latency,
        })
    }

    async fn list_databases(&self) -> Result<Vec<DatabaseInfo>> {
        // Parse connection string
        let mut client_options = tokio::time::timeout(
            self.config.timeout,
            ClientOptions::parse(&self.config.connection_string),
        )
        .await
        .map_err(|_| ConnectionError::Timeout(self.config.timeout))?
        .map_err(|e| ConnectionError::InvalidConnectionString(e.to_string()))?;

        client_options.connect_timeout = Some(self.config.timeout);
        client_options.server_selection_timeout = Some(self.config.timeout);

        let client =
            Client::with_options(client_options).map_err(|e| ConnectionError::Failed(e.to_string()))?;

        let databases = tokio::time::timeout(
            self.config.timeout,
            client.list_databases(),
        )
        .await
        .map_err(|_| ConnectionError::Timeout(self.config.timeout))?
        .map_err(|e| ConnectionError::Failed(e.to_string()))?;

        let result = databases
            .into_iter()
            .map(|db| DatabaseInfo {
                name: db.name,
                size_bytes: Some(db.size_on_disk),
            })
            .collect();

        Ok(result)
    }

    async fn list_collections(&self, database_name: &str) -> Result<Vec<CollectionInfo>> {
        // Parse connection string
        let mut client_options = tokio::time::timeout(
            self.config.timeout,
            ClientOptions::parse(&self.config.connection_string),
        )
        .await
        .map_err(|_| ConnectionError::Timeout(self.config.timeout))?
        .map_err(|e| ConnectionError::InvalidConnectionString(e.to_string()))?;

        client_options.connect_timeout = Some(self.config.timeout);
        client_options.server_selection_timeout = Some(self.config.timeout);

        let client =
            Client::with_options(client_options).map_err(|e| ConnectionError::Failed(e.to_string()))?;

        let db = client.database(database_name);

        let collections = tokio::time::timeout(
            self.config.timeout,
            db.list_collection_names(),
        )
        .await
        .map_err(|_| ConnectionError::Timeout(self.config.timeout))?
        .map_err(|e| ConnectionError::Failed(e.to_string()))?;

        let result = collections
            .into_iter()
            .map(|name| CollectionInfo {
                name,
                document_count: None, // Would require additional queries per collection
            })
            .collect();

        Ok(result)
    }

    async fn query_documents(
        &self,
        database_name: &str,
        collection_name: &str,
        limit: u32,
        skip: u32,
    ) -> Result<Vec<serde_json::Value>> {
        // Parse connection string
        let mut client_options = tokio::time::timeout(
            self.config.timeout,
            ClientOptions::parse(&self.config.connection_string),
        )
        .await
        .map_err(|_| ConnectionError::Timeout(self.config.timeout))?
        .map_err(|e| ConnectionError::InvalidConnectionString(e.to_string()))?;

        client_options.connect_timeout = Some(self.config.timeout);
        client_options.server_selection_timeout = Some(self.config.timeout);

        let client =
            Client::with_options(client_options).map_err(|e| ConnectionError::Failed(e.to_string()))?;

        let db = client.database(database_name);
        let collection = db.collection::<Document>(collection_name);

        // Build find options
        let find_options = mongodb::options::FindOptions::builder()
            .limit(Some(limit as i64))
            .skip(Some(skip as u64))
            .build();

        // Execute query
        let mut cursor = tokio::time::timeout(
            self.config.timeout,
            collection.find(Document::new()).with_options(find_options),
        )
        .await
        .map_err(|_| ConnectionError::Timeout(self.config.timeout))?
        .map_err(|e| ConnectionError::Failed(e.to_string()))?;

        // Collect results
        let mut documents = Vec::new();
        while let Some(doc) = cursor
            .try_next()
            .await
            .map_err(|e: mongodb::error::Error| ConnectionError::Failed(e.to_string()))?
        {
            // Convert BSON Document to JSON Value
            let json = mongodb::bson::to_bson(&doc)
                .map_err(|e| ConnectionError::Failed(e.to_string()))?;
            let value = serde_json::to_value(&json)
                .map_err(|e| ConnectionError::Failed(e.to_string()))?;
            documents.push(value);
        }

        Ok(documents)
    }

    async fn count_documents(
        &self,
        database_name: &str,
        collection_name: &str,
    ) -> Result<usize> {
        // Parse connection string
        let mut client_options = tokio::time::timeout(
            self.config.timeout,
            ClientOptions::parse(&self.config.connection_string),
        )
        .await
        .map_err(|_| ConnectionError::Timeout(self.config.timeout))?
        .map_err(|e| ConnectionError::InvalidConnectionString(e.to_string()))?;

        client_options.connect_timeout = Some(self.config.timeout);
        client_options.server_selection_timeout = Some(self.config.timeout);

        let client =
            Client::with_options(client_options).map_err(|e| ConnectionError::Failed(e.to_string()))?;

        let db = client.database(database_name);
        let collection = db.collection::<Document>(collection_name);

        let count = tokio::time::timeout(
            self.config.timeout,
            collection.count_documents(Document::new()),
        )
        .await
        .map_err(|_| ConnectionError::Timeout(self.config.timeout))?
        .map_err(|e| ConnectionError::Failed(e.to_string()))?;

        Ok(count as usize)
    }
}
