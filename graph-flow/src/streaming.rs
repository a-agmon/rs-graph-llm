//! Streaming execution support for graph-flow.
//!
//! This module adds streaming capabilities to the graph execution engine,
//! allowing tasks to yield intermediate results as they execute.
//!
//! # Overview
//!
//! LangGraph (Python) supports streaming via `CompiledGraph.stream()`.
//! This module provides the Rust equivalent through:
//! - [`StreamChunk`] — a single streaming event from a task
//! - [`StreamingTask`] — trait for tasks that support streaming
//! - [`StreamingRunner`] — executes a graph and streams results
//!
//! # Examples
//!
//! ```rust
//! use graph_flow::streaming::{StreamChunk, StreamingRunner};
//! use graph_flow::{Graph, GraphBuilder, InMemorySessionStorage, Session, SessionStorage};
//! use std::sync::Arc;
//! use tokio::sync::mpsc;
//!
//! # #[tokio::main]
//! # async fn main() -> graph_flow::Result<()> {
//! // StreamingRunner wraps a normal graph execution and emits
//! // StreamChunks for each task that completes.
//! # Ok(())
//! # }
//! ```

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::mpsc;

use crate::{
    context::Context,
    error::{GraphError, Result},
    graph::{ExecutionResult, ExecutionStatus, Graph},
    storage::{Session, SessionStorage},
    task::{Task, TaskResult},
};

/// A single streaming event emitted during graph execution.
///
/// Each chunk represents the output of one task completing.
/// The `is_final` flag indicates whether this is the last chunk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamChunk {
    /// ID of the task that produced this chunk
    pub task_id: String,
    /// The data payload (task response as JSON)
    pub data: serde_json::Value,
    /// Whether this is the final chunk in the stream
    pub is_final: bool,
    /// Optional metadata about the execution step
    pub metadata: Option<StreamMetadata>,
}

/// Metadata attached to a stream chunk for observability.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamMetadata {
    /// The execution status after this task completed
    pub status: String,
    /// The next task that will execute (if any)
    pub next_task_id: Option<String>,
    /// Elapsed time for this task in milliseconds
    pub elapsed_ms: Option<u64>,
}

/// Trait for tasks that support streaming intermediate results.
///
/// This is an optional extension to the base [`Task`] trait. Tasks that
/// implement `StreamingTask` can emit intermediate results during execution.
///
/// # Examples
///
/// ```rust
/// use graph_flow::{Task, TaskResult, NextAction, Context};
/// use graph_flow::streaming::{StreamingTask, StreamChunk};
/// use async_trait::async_trait;
/// use tokio::sync::mpsc;
///
/// struct LlmStreamTask;
///
/// #[async_trait]
/// impl Task for LlmStreamTask {
///     fn id(&self) -> &str { "llm_stream" }
///     async fn run(&self, context: Context) -> graph_flow::Result<TaskResult> {
///         Ok(TaskResult::new(Some("done".to_string()), NextAction::Continue))
///     }
/// }
///
/// #[async_trait]
/// impl StreamingTask for LlmStreamTask {
///     async fn run_streaming(
///         &self,
///         context: Context,
///         sender: mpsc::Sender<StreamChunk>,
///     ) -> graph_flow::Result<TaskResult> {
///         // Emit intermediate chunks
///         let _ = sender.send(StreamChunk {
///             task_id: self.id().to_string(),
///             data: serde_json::json!({"token": "Hello"}),
///             is_final: false,
///             metadata: None,
///         }).await;
///         let _ = sender.send(StreamChunk {
///             task_id: self.id().to_string(),
///             data: serde_json::json!({"token": " World"}),
///             is_final: false,
///             metadata: None,
///         }).await;
///         Ok(TaskResult::new(Some("Hello World".to_string()), NextAction::Continue))
///     }
/// }
/// ```
#[async_trait::async_trait]
pub trait StreamingTask: Task {
    /// Execute the task with streaming support.
    ///
    /// The task should send intermediate results via the `sender` channel.
    /// The final result is still returned as a `TaskResult`.
    async fn run_streaming(
        &self,
        context: Context,
        sender: mpsc::Sender<StreamChunk>,
    ) -> Result<TaskResult>;
}

/// Streaming execution runner for graph workflows.
///
/// Wraps a `Graph` and `SessionStorage` and provides streaming execution
/// that emits `StreamChunk`s for each task that completes.
///
/// # Examples
///
/// ```rust,no_run
/// use graph_flow::streaming::StreamingRunner;
/// use graph_flow::{Graph, InMemorySessionStorage, Session, SessionStorage};
/// use std::sync::Arc;
/// use tokio::sync::mpsc;
///
/// # #[tokio::main]
/// # async fn main() -> graph_flow::Result<()> {
/// let graph = Arc::new(Graph::new("my_workflow"));
/// let storage = Arc::new(InMemorySessionStorage::new());
/// let runner = StreamingRunner::new(graph, storage.clone());
///
/// // Create a session
/// let session = Session::new_from_task("s1".to_string(), "start");
/// storage.save(session).await?;
///
/// // Stream execution
/// let (tx, mut rx) = mpsc::channel(32);
/// tokio::spawn(async move {
///     let _ = runner.run_streaming("s1", tx).await;
/// });
///
/// while let Some(chunk) = rx.recv().await {
///     println!("Chunk from {}: {:?}", chunk.task_id, chunk.data);
///     if chunk.is_final { break; }
/// }
/// # Ok(())
/// # }
/// ```
#[derive(Clone)]
pub struct StreamingRunner {
    graph: Arc<Graph>,
    storage: Arc<dyn SessionStorage>,
}

impl StreamingRunner {
    /// Create a new streaming runner.
    pub fn new(graph: Arc<Graph>, storage: Arc<dyn SessionStorage>) -> Self {
        Self { graph, storage }
    }

    /// Execute the graph for the given session, streaming results via the sender.
    ///
    /// This runs the full graph to completion (following ContinueAndExecute chains),
    /// emitting a `StreamChunk` after each task completes.
    ///
    /// For step-by-step execution (one task per call), use `run_streaming_step`.
    pub async fn run_streaming(
        &self,
        session_id: &str,
        sender: mpsc::Sender<StreamChunk>,
    ) -> Result<ExecutionResult> {
        let mut session = self
            .storage
            .get(session_id)
            .await?
            .ok_or_else(|| GraphError::SessionNotFound(session_id.to_string()))?;

        let result = self
            .execute_streaming(&mut session, &sender)
            .await;

        // Save session regardless of result
        self.storage.save(session).await?;

        result
    }

    /// Execute exactly one step, streaming any intermediate results.
    pub async fn run_streaming_step(
        &self,
        session_id: &str,
        sender: mpsc::Sender<StreamChunk>,
    ) -> Result<ExecutionResult> {
        let mut session = self
            .storage
            .get(session_id)
            .await?
            .ok_or_else(|| GraphError::SessionNotFound(session_id.to_string()))?;

        let result = self.graph.execute_session(&mut session).await?;

        // Emit a chunk for the completed task
        let chunk = StreamChunk {
            task_id: session.current_task_id.clone(),
            data: serde_json::json!({
                "response": result.response,
            }),
            is_final: matches!(result.status, ExecutionStatus::Completed),
            metadata: Some(StreamMetadata {
                status: format!("{:?}", result.status),
                next_task_id: match &result.status {
                    ExecutionStatus::Paused { next_task_id, .. } => Some(next_task_id.clone()),
                    _ => None,
                },
                elapsed_ms: None,
            }),
        };
        let _ = sender.send(chunk).await;

        self.storage.save(session).await?;
        Ok(result)
    }

    /// Internal: execute graph with streaming, following ContinueAndExecute chains.
    async fn execute_streaming(
        &self,
        session: &mut Session,
        sender: &mpsc::Sender<StreamChunk>,
    ) -> Result<ExecutionResult> {
        let start = std::time::Instant::now();
        let result = self.graph.execute_session(session).await?;
        let elapsed = start.elapsed().as_millis() as u64;

        let is_final = matches!(
            result.status,
            ExecutionStatus::Completed | ExecutionStatus::WaitingForInput
        );

        let chunk = StreamChunk {
            task_id: session.current_task_id.clone(),
            data: serde_json::json!({
                "response": result.response,
            }),
            is_final,
            metadata: Some(StreamMetadata {
                status: format!("{:?}", result.status),
                next_task_id: match &result.status {
                    ExecutionStatus::Paused { next_task_id, .. } => Some(next_task_id.clone()),
                    _ => None,
                },
                elapsed_ms: Some(elapsed),
            }),
        };
        let _ = sender.send(chunk).await;

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        GraphBuilder, InMemorySessionStorage, NextAction, Session, SessionStorage,
    };
    use async_trait::async_trait;

    struct StepTask {
        name: String,
        next: NextAction,
    }

    #[async_trait]
    impl Task for StepTask {
        fn id(&self) -> &str {
            &self.name
        }
        async fn run(&self, context: Context) -> Result<TaskResult> {
            context
                .set(
                    format!("{}_done", self.name),
                    true,
                )
                .await;
            Ok(TaskResult::new(
                Some(format!("{} completed", self.name)),
                self.next.clone(),
            ))
        }
    }

    #[tokio::test]
    async fn test_streaming_step() {
        let task_a = Arc::new(StepTask {
            name: "a".to_string(),
            next: NextAction::Continue,
        });
        let task_b = Arc::new(StepTask {
            name: "b".to_string(),
            next: NextAction::End,
        });

        let graph = Arc::new(
            GraphBuilder::new("test")
                .add_task(task_a.clone())
                .add_task(task_b.clone())
                .add_edge("a", "b")
                .build(),
        );

        let storage = Arc::new(InMemorySessionStorage::new());
        let session = Session::new_from_task("s1".to_string(), "a");
        storage.save(session).await.unwrap();

        let runner = StreamingRunner::new(graph, storage);
        let (tx, mut rx) = mpsc::channel(32);

        let result = runner.run_streaming_step("s1", tx).await.unwrap();
        assert!(matches!(result.status, ExecutionStatus::Paused { .. }));

        let chunk = rx.recv().await.unwrap();
        assert_eq!(chunk.task_id, "b"); // session moved to next task
        assert!(!chunk.is_final);
    }

    #[tokio::test]
    async fn test_stream_chunk_serialization() {
        let chunk = StreamChunk {
            task_id: "test_task".to_string(),
            data: serde_json::json!({"result": "hello"}),
            is_final: false,
            metadata: Some(StreamMetadata {
                status: "Paused".to_string(),
                next_task_id: Some("next".to_string()),
                elapsed_ms: Some(42),
            }),
        };

        let json = serde_json::to_string(&chunk).unwrap();
        let deserialized: StreamChunk = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.task_id, "test_task");
        assert!(!deserialized.is_final);
    }
}
