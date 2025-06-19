use thiserror::Error;

#[derive(Error, Debug)]
pub enum GraphError {
    #[error("Task execution failed: {0}")]
    TaskExecutionFailed(String),

    #[error("Graph not found: {0}")]
    GraphNotFound(String),

    #[error("Invalid edge: {0}")]
    InvalidEdge(String),

    #[error("Task not found: {0}")]
    TaskNotFound(String),

    #[error("Context error: {0}")]
    ContextError(String),

    #[error("Storage error: {0}")]
    StorageError(String),

    #[error("Session not found: {0}")]
    SessionNotFound(String),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

pub type Result<T> = std::result::Result<T, GraphError>;
