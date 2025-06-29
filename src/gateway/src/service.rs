use crate::auth::Claims;
use crate::db::{Database, Document, VectorClock};
use crate::error::ServiceError;
use crate::grpc::memory_gateway_server::MemoryGateway;
use crate::grpc::*;
use crate::metrics;
use crate::config::Config;
use mongodb::Client;
use std::sync::Arc;
use tokio::sync::RwLock;
use tonic::{Request, Response, Status};
use tracing::{debug, error, info, instrument};

pub struct MemoryGatewayService {
    db: Arc<Database>,
    config: Config,
}

impl MemoryGatewayService {
    pub fn new(client: Client, config: Config) -> Self {
        Self {
            db: Arc::new(Database::new(client)),
            config,
        }
    }

    /// Extract and validate JWT claims from request
    fn validate_auth(&self, request: &Request<impl std::fmt::Debug>) -> Result<Claims, Status> {
        let token = request
            .metadata()
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .ok_or_else(|| Status::unauthenticated("Missing authorization header"))?;

        crate::auth::validate_token(token, &self.config.jwt_secret)
            .map_err(|e| Status::unauthenticated(format!("Invalid token: {}", e)))
    }

    /// Check if user has required scope
    fn check_scope(&self, claims: &Claims, required: &str) -> Result<(), Status> {
        if !claims.scopes.contains(&required.to_string()) {
            return Err(Status::permission_denied(format!(
                "Missing required scope: {}",
                required
            )));
        }
        Ok(())
    }
}

#[tonic::async_trait]
impl MemoryGateway for MemoryGatewayService {
    #[instrument(skip(self, request))]
    async fn write_diff(
        &self,
        request: Request<WriteDiffRequest>,
    ) -> Result<Response<WriteDiffResponse>, Status> {
        // Validate authentication
        let claims = self.validate_auth(&request)?;
        self.check_scope(&claims, "dom.write")?;

        let req = request.into_inner();
        metrics::WRITE_REQUESTS.inc();

        // Create document
        let doc = Document {
            id: None,
            repo: req.repo.clone(),
            branch: req.branch.clone(),
            path: req.path.clone(),
            blob: req.diff,
            author: claims.sub,
            version: VectorClock::new(),
            timestamp: chrono::Utc::now(),
            doc_type: "diff".to_string(),
            metadata: req.metadata,
        };

        // Attempt to write with optimistic locking
        match self.db.write_document(doc, req.parent_version).await {
            Ok((id, version)) => {
                metrics::WRITE_SUCCESS.inc();
                Ok(Response::new(WriteDiffResponse {
                    id,
                    version,
                    timestamp: Some(prost_types::Timestamp::from(chrono::Utc::now())),
                    conflict: None,
                }))
            }
            Err(ServiceError::VersionConflict { current }) => {
                metrics::WRITE_CONFLICTS.inc();
                
                // Fetch current content for conflict info
                let current_doc = self
                    .db
                    .read_document(&req.repo, &req.branch, &req.path, Some(current))
                    .await
                    .map_err(|e| Status::internal(format!("Failed to fetch conflict info: {}", e)))?;

                Ok(Response::new(WriteDiffResponse {
                    id: String::new(),
                    version: 0,
                    timestamp: Some(prost_types::Timestamp::from(chrono::Utc::now())),
                    conflict: Some(ConflictInfo {
                        has_conflict: true,
                        current_version: current,
                        current_author: current_doc.author,
                        current_content: current_doc.blob,
                    }),
                }))
            }
            Err(e) => {
                metrics::WRITE_ERRORS.inc();
                error!("Write failed: {}", e);
                Err(Status::internal("Write operation failed"))
            }
        }
    }

    #[instrument(skip(self, request))]
    async fn read_snapshot(
        &self,
        request: Request<ReadSnapshotRequest>,
    ) -> Result<Response<ReadSnapshotResponse>, Status> {
        // Validate authentication
        let claims = self.validate_auth(&request)?;
        self.check_scope(&claims, "dom.read")?;

        let req = request.into_inner();
        metrics::READ_REQUESTS.inc();

        match self
            .db
            .read_document(&req.repo, &req.branch, &req.path, req.version.into())
            .await
        {
            Ok(doc) => {
                metrics::READ_SUCCESS.inc();
                Ok(Response::new(ReadSnapshotResponse {
                    id: doc.id.unwrap_or_default(),
                    content: doc.blob,
                    version: doc.version.value(),
                    author: doc.author,
                    timestamp: Some(prost_types::Timestamp::from(doc.timestamp)),
                    metadata: doc.metadata,
                }))
            }
            Err(ServiceError::NotFound) => {
                metrics::READ_NOT_FOUND.inc();
                Err(Status::not_found("Document not found"))
            }
            Err(e) => {
                metrics::READ_ERRORS.inc();
                error!("Read failed: {}", e);
                Err(Status::internal("Read operation failed"))
            }
        }
    }

    type SubscribeChangesStream = tokio_stream::wrappers::ReceiverStream<Result<ChangeEvent, Status>>;

    #[instrument(skip(self, request))]
    async fn subscribe_changes(
        &self,
        request: Request<SubscribeChangesRequest>,
    ) -> Result<Response<Self::SubscribeChangesStream>, Status> {
        // Validate authentication
        let claims = self.validate_auth(&request)?;
        self.check_scope(&claims, "dom.read")?;

        let req = request.into_inner();
        let (tx, rx) = tokio::sync::mpsc::channel(128);

        // Spawn task to watch changes
        let db = self.db.clone();
        tokio::spawn(async move {
            if let Err(e) = db
                .watch_changes(
                    &req.repo,
                    &req.branch,
                    req.paths,
                    req.from_version as u64,
                    tx,
                )
                .await
            {
                error!("Change stream error: {}", e);
            }
        });

        Ok(Response::new(tokio_stream::wrappers::ReceiverStream::new(rx)))
    }

    #[instrument(skip(self, request))]
    async fn resolve_conflict(
        &self,
        request: Request<ResolveConflictRequest>,
    ) -> Result<Response<ResolveConflictResponse>, Status> {
        // Validate authentication
        let claims = self.validate_auth(&request)?;
        self.check_scope(&claims, "dom.write")?;

        let req = request.into_inner();
        
        // Use conflict resolver based on strategy
        let merged_content = match req.strategy.as_str() {
            "ours" => req.local_content,
            "theirs" => req.remote_content,
            "ai" => {
                // TODO: Integrate Claude for AI-powered resolution
                return Err(Status::unimplemented("AI resolution not yet implemented"));
            }
            _ => {
                return Err(Status::invalid_argument("Invalid resolution strategy"));
            }
        };

        Ok(Response::new(ResolveConflictResponse {
            merged_content,
            markers: vec![],
            ai_resolved: false,
            resolution_notes: format!("Resolved using {} strategy", req.strategy),
        }))
    }

    #[instrument(skip(self, request))]
    async fn get_history(
        &self,
        request: Request<GetHistoryRequest>,
    ) -> Result<Response<GetHistoryResponse>, Status> {
        // Validate authentication
        let claims = self.validate_auth(&request)?;
        self.check_scope(&claims, "dom.read")?;

        let req = request.into_inner();
        
        match self
            .db
            .get_history(
                &req.repo,
                &req.branch,
                &req.path,
                req.limit as usize,
                req.before_version.into(),
            )
            .await
        {
            Ok(entries) => {
                let has_more = entries.len() == req.limit as usize;
                let history_entries = entries
                    .into_iter()
                    .map(|e| HistoryEntry {
                        id: e.id,
                        version: e.version,
                        author: e.author,
                        message: e.message,
                        timestamp: Some(prost_types::Timestamp::from(e.timestamp)),
                        additions: e.additions,
                        deletions: e.deletions,
                    })
                    .collect();

                Ok(Response::new(GetHistoryResponse {
                    entries: history_entries,
                    has_more,
                }))
            }
            Err(e) => {
                error!("History fetch failed: {}", e);
                Err(Status::internal("Failed to fetch history"))
            }
        }
    }
}