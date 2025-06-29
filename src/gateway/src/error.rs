use thiserror::Error;

#[derive(Debug, Error)]
pub enum ServiceError {
    #[error("Database error: {0}")]
    Database(#[from] mongodb::error::Error),

    #[error("BSON serialization error: {0}")]
    Bson(#[from] bson::ser::Error),

    #[error("Document not found")]
    NotFound,

    #[error("Version conflict: current version is {current}")]
    VersionConflict { current: i64 },

    #[error("Authentication failed: {0}")]
    AuthenticationFailed(String),

    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    #[error("Internal error: {0}")]
    Internal(String),

    #[error("Invalid request: {0}")]
    InvalidRequest(String),
}

impl From<ServiceError> for tonic::Status {
    fn from(err: ServiceError) -> Self {
        match err {
            ServiceError::Database(e) => {
                tonic::Status::internal(format!("Database error: {}", e))
            }
            ServiceError::Bson(e) => {
                tonic::Status::internal(format!("Serialization error: {}", e))
            }
            ServiceError::NotFound => {
                tonic::Status::not_found("Document not found")
            }
            ServiceError::VersionConflict { current } => {
                tonic::Status::aborted(format!("Version conflict: current version is {}", current))
            }
            ServiceError::AuthenticationFailed(msg) => {
                tonic::Status::unauthenticated(msg)
            }
            ServiceError::PermissionDenied(msg) => {
                tonic::Status::permission_denied(msg)
            }
            ServiceError::Internal(msg) => {
                tonic::Status::internal(msg)
            }
            ServiceError::InvalidRequest(msg) => {
                tonic::Status::invalid_argument(msg)
            }
        }
    }
}