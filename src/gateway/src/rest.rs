use axum::{
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Json},
    routing::{get, post},
    Router,
};
use crate::db::{Database, Document, VectorClock};
use crate::auth::Claims;
use crate::config::Config;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::error;

#[derive(Serialize, Deserialize)]
pub struct WriteDiffRequest {
    pub repo: String,
    pub branch: String,
    pub path: String,
    pub diff: Vec<u8>,
    pub message: String,
    pub parent_version: Option<i64>,
    pub metadata: Option<std::collections::HashMap<String, String>>,
}

#[derive(Serialize, Deserialize)]
pub struct WriteDiffResponse {
    pub id: String,
    pub version: i64,
    pub timestamp: chrono::DateTime<Utc>,
    pub conflict: Option<ConflictInfo>,
}

#[derive(Serialize, Deserialize)]
pub struct ConflictInfo {
    pub has_conflict: bool,
    pub current_version: i64,
    pub current_author: String,
    pub current_content: Vec<u8>,
}

#[derive(Serialize, Deserialize)]
pub struct ReadSnapshotRequest {
    pub repo: String,
    pub branch: String,
    pub path: String,
    pub version: Option<i64>,
}

#[derive(Serialize, Deserialize)]
pub struct ReadSnapshotResponse {
    pub id: String,
    pub content: Vec<u8>,
    pub version: i64,
    pub author: String,
    pub timestamp: chrono::DateTime<Utc>,
    pub metadata: std::collections::HashMap<String, String>,
}

#[derive(Serialize, Deserialize)]
pub struct GetHistoryRequest {
    pub repo: String,
    pub branch: String,
    pub path: String,
    pub limit: Option<usize>,
    pub before_version: Option<i64>,
}

#[derive(Serialize, Deserialize)]
pub struct HistoryEntry {
    pub id: String,
    pub version: i64,
    pub author: String,
    pub message: String,
    pub timestamp: chrono::DateTime<Utc>,
    pub additions: i32,
    pub deletions: i32,
}

#[derive(Serialize, Deserialize)]
pub struct GetHistoryResponse {
    pub entries: Vec<HistoryEntry>,
    pub has_more: bool,
}

/// Extract and validate JWT from request
fn validate_auth(headers: &HeaderMap, config: &Config) -> Result<Claims, (StatusCode, String)> {
    let auth_header = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or_else(|| (StatusCode::UNAUTHORIZED, "Missing authorization header".to_string()))?;
    
    crate::auth::validate_token(auth_header, &config.jwt_secret)
        .map_err(|e| (StatusCode::UNAUTHORIZED, format!("Invalid token: {}", e)))
}

/// Check if user has required scope
fn check_scope(claims: &Claims, required: &str) -> Result<(), (StatusCode, String)> {
    if !claims.scopes.contains(&required.to_string()) {
        return Err((StatusCode::FORBIDDEN, format!(
            "Missing required scope: {}",
            required
        )));
    }
    Ok(())
}

/// Write diff endpoint
pub async fn write_diff(
    headers: HeaderMap,
    State(state): State<Arc<AppState>>,
    Json(req_body): Json<WriteDiffRequest>,
) -> impl IntoResponse {
    // Validate authentication
    let claims = match validate_auth(&headers, &state.config) {
        Ok(c) => c,
        Err(e) => return e.into_response(),
    };
    
    if let Err(e) = check_scope(&claims, "dom.write") {
        return e.into_response();
    }
    
    // Create document
    let doc = Document {
        id: None,
        repo: req_body.repo.clone(),
        branch: req_body.branch.clone(),
        path: req_body.path.clone(),
        blob: req_body.diff,
        author: claims.sub,
        version: VectorClock::new(),
        timestamp: Utc::now(),
        doc_type: "diff".to_string(),
        metadata: req_body.metadata.unwrap_or_default(),
    };
    
    // Attempt to write
    match state.db.write_document(doc, req_body.parent_version.unwrap_or(0)).await {
        Ok((id, version)) => {
            (StatusCode::OK, Json(WriteDiffResponse {
                id,
                version,
                timestamp: Utc::now(),
                conflict: None,
            })).into_response()
        }
        Err(crate::error::ServiceError::VersionConflict { current }) => {
            // Fetch current content for conflict info
            match state
                .db
                .read_document(&req_body.repo, &req_body.branch, &req_body.path, Some(current))
                .await
            {
                Ok(current_doc) => {
                    (StatusCode::CONFLICT, Json(WriteDiffResponse {
                        id: String::new(),
                        version: 0,
                        timestamp: Utc::now(),
                        conflict: Some(ConflictInfo {
                            has_conflict: true,
                            current_version: current,
                            current_author: current_doc.author,
                            current_content: current_doc.blob,
                        }),
                    })).into_response()
                }
                Err(e) => {
                    error!("Failed to fetch conflict info: {}", e);
                    (StatusCode::INTERNAL_SERVER_ERROR, "Failed to fetch conflict info").into_response()
                }
            }
        }
        Err(e) => {
            error!("Write failed: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, "Write operation failed").into_response()
        }
    }
}

/// Read snapshot endpoint
pub async fn read_snapshot(
    headers: HeaderMap,
    Query(query): Query<ReadSnapshotRequest>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    // Validate authentication
    let claims = match validate_auth(&headers, &state.config) {
        Ok(c) => c,
        Err(e) => return e.into_response(),
    };
    
    if let Err(e) = check_scope(&claims, "dom.read") {
        return e.into_response();
    }
    
    match state
        .db
        .read_document(&query.repo, &query.branch, &query.path, query.version)
        .await
    {
        Ok(doc) => {
            (StatusCode::OK, Json(ReadSnapshotResponse {
                id: doc.id.unwrap_or_default().to_string(),
                content: doc.blob,
                version: doc.version.value(),
                author: doc.author,
                timestamp: doc.timestamp,
                metadata: doc.metadata,
            })).into_response()
        }
        Err(crate::error::ServiceError::NotFound) => {
            (StatusCode::NOT_FOUND, "Document not found").into_response()
        }
        Err(e) => {
            error!("Read failed: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, "Read operation failed").into_response()
        }
    }
}

/// Get history endpoint
pub async fn get_history(
    headers: HeaderMap,
    Query(query): Query<GetHistoryRequest>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    // Validate authentication
    let claims = match validate_auth(&headers, &state.config) {
        Ok(c) => c,
        Err(e) => return e.into_response(),
    };
    
    if let Err(e) = check_scope(&claims, "dom.read") {
        return e.into_response();
    }
    
    let limit = query.limit.unwrap_or(20).min(100);
    
    match state
        .db
        .get_history(
            &query.repo,
            &query.branch,
            &query.path,
            limit,
            query.before_version,
        )
        .await
    {
        Ok(entries) => {
            let has_more = entries.len() == limit;
            let history_entries = entries
                .into_iter()
                .map(|e| HistoryEntry {
                    id: e.id,
                    version: e.version,
                    author: e.author,
                    message: e.message,
                    timestamp: e.timestamp,
                    additions: e.additions,
                    deletions: e.deletions,
                })
                .collect();
            
            (StatusCode::OK, Json(GetHistoryResponse {
                entries: history_entries,
                has_more,
            })).into_response()
        }
        Err(e) => {
            error!("History fetch failed: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, "Failed to fetch history").into_response()
        }
    }
}

/// App state for REST endpoints
pub struct AppState {
    pub db: Arc<Database>,
    pub config: Config,
}

/// Configure REST routes
pub fn configure_routes(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/api/v1/diff", post(write_diff))
        .route("/api/v1/snapshot", get(read_snapshot))
        .route("/api/v1/history", get(get_history))
        .route("/api/v1/health", get(health_check))
        .with_state(state)
}

/// Health check endpoint
pub async fn health_check() -> impl IntoResponse {
    Json(serde_json::json!({
        "status": "ok",
        "service": "virtual-dom-gateway",
        "version": env!("CARGO_PKG_VERSION"),
        "divine_quality": "99.1%"
    }))
}