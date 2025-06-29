use crate::error::ServiceError;
use crate::grpc::ChangeEvent;
use bson::{doc, Document as BsonDocument};
use chrono::{DateTime, Utc};
use mongodb::{
    change_stream::event::ChangeStreamEvent,
    options::{ChangeStreamOptions, ClientOptions, FindOneOptions, UpdateOptions},
    Client, Collection,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::sync::mpsc::Sender;
use tonic::Status;
use tracing::{debug, error, info, instrument};

/// Vector clock for optimistic locking
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct VectorClock {
    value: i64,
}

impl VectorClock {
    pub fn new() -> Self {
        Self { value: 1 }
    }

    pub fn increment(&mut self) {
        self.value += 1;
    }

    pub fn value(&self) -> i64 {
        self.value
    }
}

/// Document structure in MongoDB
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    #[serde(rename = "_id", skip_serializing_if = "Option::is_none")]
    pub id: Option<bson::oid::ObjectId>,
    pub repo: String,
    pub branch: String,
    pub path: String,
    pub blob: Vec<u8>,
    pub author: String,
    #[serde(rename = "_v")]
    pub version: VectorClock,
    pub timestamp: DateTime<Utc>,
    #[serde(rename = "type")]
    pub doc_type: String,
    pub metadata: HashMap<String, String>,
}

/// History entry for a document
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub id: String,
    pub version: i64,
    pub author: String,
    pub message: String,
    pub timestamp: DateTime<Utc>,
    pub additions: i32,
    pub deletions: i32,
}

/// Database connection and operations
pub struct Database {
    client: Client,
    database_name: String,
}

impl Database {
    pub fn new(client: Client) -> Self {
        Self {
            client,
            database_name: "virtual_dom".to_string(),
        }
    }

    fn collection(&self) -> Collection<Document> {
        self.client
            .database(&self.database_name)
            .collection("documents")
    }

    fn history_collection(&self) -> Collection<HistoryEntry> {
        self.client
            .database(&self.database_name)
            .collection("history")
    }

    #[instrument(skip(self, doc))]
    pub async fn write_document(
        &self,
        mut doc: Document,
        parent_version: i64,
    ) -> Result<(String, i64), ServiceError> {
        let collection = self.collection();

        // Check for existing document and version
        let filter = doc! {
            "repo": &doc.repo,
            "branch": &doc.branch,
            "path": &doc.path,
        };

        let existing = collection.find_one(filter.clone()).await?;

        match existing {
            Some(existing_doc) if existing_doc.version.value() != parent_version => {
                // Version conflict
                Err(ServiceError::VersionConflict {
                    current: existing_doc.version.value(),
                })
            }
            Some(mut existing_doc) => {
                // Update existing document
                existing_doc.version.increment();
                doc.version = existing_doc.version;
                doc.id = existing_doc.id;

                let update = doc! {
                    "$set": bson::to_document(&doc)?,
                };

                collection
                    .update_one(filter, update)
                    .with_options(UpdateOptions::builder().upsert(true).build())
                    .await?;

                // Record in history
                self.record_history(&doc).await?;

                Ok((
                    doc.id.unwrap().to_string(),
                    doc.version.value(),
                ))
            }
            None => {
                // New document
                let result = collection.insert_one(&doc).await?;
                let id = result
                    .inserted_id
                    .as_object_id()
                    .ok_or_else(|| ServiceError::Internal("Failed to get inserted ID".into()))?;

                // Record in history
                doc.id = Some(id);
                self.record_history(&doc).await?;

                Ok((id.to_string(), doc.version.value()))
            }
        }
    }

    #[instrument(skip(self))]
    pub async fn read_document(
        &self,
        repo: &str,
        branch: &str,
        path: &str,
        version: Option<i64>,
    ) -> Result<Document, ServiceError> {
        let collection = self.collection();

        let filter = doc! {
            "repo": repo,
            "branch": branch,
            "path": path,
        };

        if let Some(v) = version {
            // TODO: Implement version-specific retrieval from history
            // For now, just return latest if version matches
        }

        collection
            .find_one(filter)
            .await?
            .ok_or(ServiceError::NotFound)
    }

    #[instrument(skip(self, tx))]
    pub async fn watch_changes(
        &self,
        repo: &str,
        branch: &str,
        paths: Vec<String>,
        from_version: u64,
        tx: Sender<Result<ChangeEvent, Status>>,
    ) -> Result<(), ServiceError> {
        let collection = self.collection();

        let mut pipeline = vec![];

        // Filter by repo and branch
        let mut match_doc = doc! {
            "fullDocument.repo": repo,
            "fullDocument.branch": branch,
        };

        // Filter by paths if specified
        if !paths.is_empty() {
            match_doc.insert("fullDocument.path", doc! { "$in": paths });
        }

        // Filter by version if specified
        if from_version > 0 {
            match_doc.insert("fullDocument._v.value", doc! { "$gt": from_version as i64 });
        }

        pipeline.push(doc! { "$match": match_doc });

        let options = ChangeStreamOptions::builder()
            .full_document(mongodb::options::FullDocumentType::UpdateLookup)
            .build();

        let mut change_stream = collection
            .watch()
            .with_options(options)
            .pipeline(pipeline)
            .await?;

        while let Some(event) = change_stream.next().await {
            match event {
                Ok(ChangeStreamEvent { full_document, operation_type, .. }) => {
                    if let Some(doc) = full_document {
                        let event_type = match operation_type {
                            mongodb::change_stream::event::OperationType::Insert => 1, // CREATE
                            mongodb::change_stream::event::OperationType::Update => 2, // UPDATE
                            mongodb::change_stream::event::OperationType::Delete => 3, // DELETE
                            _ => 0, // UNSPECIFIED
                        };

                        let change_event = ChangeEvent {
                            r#type: event_type,
                            repo: doc.repo,
                            branch: doc.branch,
                            path: doc.path,
                            diff: doc.blob,
                            author: doc.author,
                            version: doc.version.value(),
                            timestamp: Some(prost_types::Timestamp::from(doc.timestamp)),
                            metadata: doc.metadata,
                        };

                        if tx.send(Ok(change_event)).await.is_err() {
                            break; // Client disconnected
                        }
                    }
                }
                Err(e) => {
                    error!("Change stream error: {}", e);
                    let _ = tx.send(Err(Status::internal("Change stream error"))).await;
                    break;
                }
            }
        }

        Ok(())
    }

    #[instrument(skip(self))]
    pub async fn get_history(
        &self,
        repo: &str,
        branch: &str,
        path: &str,
        limit: usize,
        before_version: Option<i64>,
    ) -> Result<Vec<HistoryEntry>, ServiceError> {
        let collection = self.history_collection();

        let mut filter = doc! {
            "repo": repo,
            "branch": branch,
            "path": path,
        };

        if let Some(v) = before_version {
            filter.insert("version", doc! { "$lt": v });
        }

        let options = mongodb::options::FindOptions::builder()
            .sort(doc! { "version": -1 })
            .limit(limit as i64)
            .build();

        let mut cursor = collection.find(filter).with_options(options).await?;
        let mut entries = Vec::new();

        while let Some(entry) = cursor.try_next().await? {
            entries.push(entry);
        }

        Ok(entries)
    }

    async fn record_history(&self, doc: &Document) -> Result<(), ServiceError> {
        let collection = self.history_collection();

        let history_entry = HistoryEntry {
            id: doc.id.unwrap().to_string(),
            version: doc.version.value(),
            author: doc.author.clone(),
            message: doc.metadata.get("message").cloned().unwrap_or_default(),
            timestamp: doc.timestamp,
            additions: 0, // TODO: Calculate from diff
            deletions: 0, // TODO: Calculate from diff
        };

        collection.insert_one(history_entry).await?;
        Ok(())
    }
}

/// Connect to MongoDB
pub async fn connect(uri: &str) -> Result<Client, ServiceError> {
    let client_options = ClientOptions::parse(uri).await?;
    let client = Client::with_options(client_options)?;

    // Ping to verify connection
    client
        .database("admin")
        .run_command(doc! { "ping": 1 })
        .await?;

    info!("Successfully connected to MongoDB");
    Ok(client)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vector_clock() {
        let mut clock = VectorClock::new();
        assert_eq!(clock.value(), 1);
        
        clock.increment();
        assert_eq!(clock.value(), 2);
    }
}