//! FlowRunner – convenience wrapper that loads a session, executes exactly **one** graph step, and
//! persists the updated session back to storage.
//!
//! ## When should you use `FlowRunner`?
//! * **Interactive workflows / web services**: you usually want to run _one_ step per HTTP
//!   request, send the assistant's reply back to the client, and have the session automatically
//!   saved for the next roundtrip. `FlowRunner` makes that a one-liner.
//! * **CLI demos & examples**: keeps example code tiny; no need to repeat the
//!   load-execute-save boilerplate.
//!
//! ## When should you use `Graph::execute_session` directly?
//! * **Batch processing** where you intentionally want to run many steps in a tight loop and save
//!   once at the end to reduce I/O.
//! * **Custom persistence logic** (e.g. optimistic locking, distributed transactions).
//! * **Advanced diagnostics** where you want to inspect the intermediate `Session` before saving.
//!
//! Both APIs are 100 % compatible – `FlowRunner` merely builds on top of the low-level function.
//!
//! ## Patterns for Stateless HTTP Services
//!
//! ### Pattern 1: Shared FlowRunner (RECOMMENDED)
//! Create `FlowRunner` once at startup, share across all requests:
//! ```rust,no_run
//! use graph_flow::FlowRunner;
//! use std::sync::Arc;
//!
//! // At startup
//! struct AppState {
//!     flow_runner: FlowRunner,
//! }
//!
//! // In request handler (async context)
//! # async fn example(state: AppState, session_id: String) -> Result<(), Box<dyn std::error::Error>> {
//! let result = state.flow_runner.run(&session_id).await?;
//! # Ok(())
//! # }
//! ```
//! **Pros**: Most efficient, zero allocation per request  
//! **Cons**: Requires the same graph for all requests
//!
//! ### Pattern 2: Per-Request FlowRunner
//! Create `FlowRunner` fresh for each request:
//! ```rust,no_run
//! use graph_flow::{FlowRunner, Graph, InMemorySessionStorage};
//! use std::sync::Arc;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! # let graph = Arc::new(Graph::new("my-graph"));
//! # let storage: Arc<dyn graph_flow::SessionStorage> = Arc::new(InMemorySessionStorage::new());
//! # let session_id = "test-session";
//! // In request handler
//! let runner = FlowRunner::new(graph.clone(), storage.clone());
//! let result = runner.run(&session_id).await?;
//! # Ok(())
//! # }
//! ```
//! **Pros**: Flexible, can use different graphs per request  
//! **Cons**: Tiny allocation cost per request (still very cheap)
//!
//! ### Pattern 3: Manual (Original)
//! Use `Graph::execute_session` directly:
//! ```rust,no_run
//! use graph_flow::{Graph, SessionStorage, InMemorySessionStorage};
//! use std::sync::Arc;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! # let graph = Arc::new(Graph::new("my-graph"));
//! # let storage: Arc<dyn SessionStorage> = Arc::new(InMemorySessionStorage::new());
//! # let session_id = "test-session";
//! let mut session = storage.get(&session_id).await?.unwrap();
//! let result = graph.execute_session(&mut session).await?;
//! storage.save(session).await?;
//! # Ok(())
//! # }
//! ```
//! **Pros**: Maximum control  
//! **Cons**: More boilerplate, easy to forget session.save()
//!
//! ## Performance Characteristics
//! - **FlowRunner creation cost**: ~2 pointer copies (negligible)
//! - **Memory overhead**: 16 bytes (2 × `Arc<T>`)
//! - **Runtime cost**: Identical to manual approach
//!
//! For high-throughput services, Pattern 1 is recommended. For services with different
//! graphs per request or complex routing, Pattern 2 is perfectly fine.
//!
//! # Examples
//!
//! ## Basic Usage
//!
//! ```rust,no_run
//! use graph_flow::{FlowRunner, Graph, InMemorySessionStorage};
//! use std::sync::Arc;
//!
//! # #[tokio::main]
//! # async fn main() -> graph_flow::Result<()> {
//! let graph = Arc::new(Graph::new("my_workflow"));
//! let storage = Arc::new(InMemorySessionStorage::new());
//! let runner = FlowRunner::new(graph, storage);
//!
//! // Execute workflow step (note: this will fail if session doesn't exist)
//! let result = runner.run("session_id").await?;
//! println!("Response: {:?}", result.response);
//! # Ok(())
//! # }
//! ```
//!
//! ## Shared Runner Pattern (Recommended for Web Services)
//!
//! ```rust
//! use graph_flow::FlowRunner;
//! use std::sync::Arc;
//!
//! // Application state
//! struct AppState {
//!     flow_runner: Arc<FlowRunner>,
//! }
//!
//! impl AppState {
//!     fn new(runner: FlowRunner) -> Self {
//!         Self {
//!             flow_runner: Arc::new(runner),
//!         }
//!     }
//! }
//!
//! // Request handler
//! async fn handle_request(
//!     state: Arc<AppState>,
//!     session_id: String,
//! ) -> Result<String, Box<dyn std::error::Error>> {
//!     let result = state.flow_runner.run(&session_id).await?;
//!     Ok(result.response.unwrap_or_default())
//! }
//! ```

use std::sync::Arc;

use crate::{
    context::Context,
    error::{GraphError, Result},
    graph::{ExecutionResult, ExecutionStatus, Graph},
    run_config::RunConfig,
    storage::{Session, SessionStorage},
};

/// High-level helper that orchestrates the common _load → execute → save_ pattern.
///
/// `FlowRunner` provides a convenient wrapper around the lower-level graph execution
/// API. It automatically handles session loading, execution, and persistence.
///
/// # When to Use FlowRunner
///
/// - **Web services**: Execute one step per HTTP request
/// - **Interactive applications**: Step-by-step workflow progression
/// - **Simple demos**: Minimal boilerplate for common use cases
///
/// # Performance
///
/// `FlowRunner` is lightweight and efficient:
/// - Creation cost: ~2 pointer copies (negligible)
/// - Memory overhead: 16 bytes (2 × `Arc<T>`)
/// - Runtime cost: Identical to manual approach
///
/// # Examples
///
/// ## Basic Usage
///
/// ```rust,no_run
/// use graph_flow::{FlowRunner, Graph, InMemorySessionStorage, Session, SessionStorage};
/// use std::sync::Arc;
///
/// # #[tokio::main]
/// # async fn main() -> graph_flow::Result<()> {
/// let graph = Arc::new(Graph::new("my_workflow"));
/// let storage = Arc::new(InMemorySessionStorage::new());
/// let runner = FlowRunner::new(graph, storage.clone());
///
/// // Create a session first
/// let session = Session::new_from_task("session_id".to_string(), "start_task");
/// storage.save(session).await?;
///
/// // Execute workflow step
/// let result = runner.run("session_id").await?;
/// println!("Response: {:?}", result.response);
/// # Ok(())
/// # }
/// ```
///
/// ## Shared Runner Pattern (Recommended for Web Services)
///
/// ```rust
/// use graph_flow::FlowRunner;
/// use std::sync::Arc;
///
/// // Application state
/// struct AppState {
///     flow_runner: Arc<FlowRunner>,
/// }
///
/// impl AppState {
///     fn new(runner: FlowRunner) -> Self {
///         Self {
///             flow_runner: Arc::new(runner),
///         }
///     }
/// }
///
/// // Request handler
/// async fn handle_request(
///     state: Arc<AppState>,
///     session_id: String,
/// ) -> Result<String, Box<dyn std::error::Error>> {
///     let result = state.flow_runner.run(&session_id).await?;
///     Ok(result.response.unwrap_or_default())
/// }
/// ```
#[derive(Clone)]
pub struct FlowRunner {
    graph: Arc<Graph>,
    storage: Arc<dyn SessionStorage>,
}

impl FlowRunner {
    /// Create a new `FlowRunner` from an `Arc<Graph>` and any `SessionStorage` implementation.
    ///
    /// # Parameters
    ///
    /// * `graph` - The workflow graph to execute
    /// * `storage` - Storage backend for session persistence
    ///
    /// # Examples
    ///
    /// ```rust
    /// use graph_flow::{FlowRunner, Graph, InMemorySessionStorage};
    /// use std::sync::Arc;
    ///
    /// let graph = Arc::new(Graph::new("my_workflow"));
    /// let storage = Arc::new(InMemorySessionStorage::new());
    /// let runner = FlowRunner::new(graph, storage);
    /// ```
    ///
    /// ## With PostgreSQL Storage
    ///
    /// ```rust,no_run
    /// use graph_flow::{FlowRunner, Graph, PostgresSessionStorage};
    /// use std::sync::Arc;
    ///
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let graph = Arc::new(Graph::new("my_workflow"));
    /// let storage = Arc::new(
    ///     PostgresSessionStorage::connect("postgresql://localhost/mydb").await?
    /// );
    /// let runner = FlowRunner::new(graph, storage);
    /// # Ok(())
    /// # }
    /// ```
    pub fn new(graph: Arc<Graph>, storage: Arc<dyn SessionStorage>) -> Self {
        Self { graph, storage }
    }

    /// Execute **exactly one** task for the given `session_id` and persist the updated session.
    ///
    /// This method:
    /// 1. Loads the session from storage
    /// 2. Executes the current task
    /// 3. Saves the updated session back to storage
    /// 4. Returns the execution result
    ///
    /// # Parameters
    ///
    /// * `session_id` - Unique identifier for the session to execute
    ///
    /// # Returns
    ///
    /// Returns the same [`ExecutionResult`] that `Graph::execute_session` does, so callers can
    /// inspect the assistant's response and the status (`WaitingForInput`, `Completed`, etc.).
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The session doesn't exist
    /// - Task execution fails
    /// - Storage operations fail
    ///
    /// # Examples
    ///
    /// ## Basic Execution
    ///
    /// ```rust,no_run
    /// # use graph_flow::{FlowRunner, Graph, InMemorySessionStorage, Session, SessionStorage};
    /// # use std::sync::Arc;
    /// # #[tokio::main]
    /// # async fn main() -> graph_flow::Result<()> {
    /// # let graph = Arc::new(Graph::new("test"));
    /// # let storage = Arc::new(InMemorySessionStorage::new());
    /// # let runner = FlowRunner::new(graph, storage.clone());
    /// # let session = Session::new_from_task("test_session".to_string(), "start_task");
    /// # storage.save(session).await?;
    /// let result = runner.run("test_session").await?;
    ///
    /// match result.status {
    ///     graph_flow::ExecutionStatus::Completed => {
    ///         println!("Workflow completed: {:?}", result.response);
    ///     }
    ///     graph_flow::ExecutionStatus::WaitingForInput => {
    ///         println!("Waiting for user input: {:?}", result.response);
    ///     }
    ///     graph_flow::ExecutionStatus::Paused { next_task_id, reason } => {
    ///         println!("Paused, next task: {}, reason: {}", next_task_id, reason);
    ///     }
    ///     graph_flow::ExecutionStatus::Error(e) => {
    ///         eprintln!("Error: {}", e);
    ///     }
    /// }
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// ## Interactive Loop
    ///
    /// ```rust,no_run
    /// # use graph_flow::{FlowRunner, ExecutionStatus, Session, SessionStorage};
    /// # use std::sync::Arc;
    /// # #[tokio::main]
    /// # async fn main() -> graph_flow::Result<()> {
    /// # let storage = Arc::new(graph_flow::InMemorySessionStorage::new());
    /// # let runner = FlowRunner::new(Arc::new(graph_flow::Graph::new("test")), storage.clone());
    /// # let session = Session::new_from_task("session_id".to_string(), "start_task");
    /// # storage.save(session).await?;
    /// loop {
    ///     let result = runner.run("session_id").await?;
    ///     
    ///     match result.status {
    ///         ExecutionStatus::Completed => break,
    ///         ExecutionStatus::WaitingForInput => {
    ///             // Get user input and update context
    ///             // Then continue loop
    ///             break; // For demo
    ///         }
    ///         ExecutionStatus::Paused { .. } => {
    ///             // Continue to next step
    ///             continue;
    ///         }
    ///         ExecutionStatus::Error(e) => {
    ///             eprintln!("Error: {}", e);
    ///             break;
    ///         }
    ///     }
    /// }
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// ## Error Handling
    ///
    /// ```rust,no_run
    /// # use graph_flow::{FlowRunner, GraphError};
    /// # use std::sync::Arc;
    /// # #[tokio::main]
    /// # async fn main() {
    /// # let runner = FlowRunner::new(Arc::new(graph_flow::Graph::new("test")), Arc::new(graph_flow::InMemorySessionStorage::new()));
    /// match runner.run("nonexistent_session").await {
    ///     Ok(result) => {
    ///         println!("Success: {:?}", result.response);
    ///     }
    ///     Err(GraphError::SessionNotFound(session_id)) => {
    ///         eprintln!("Session not found: {}", session_id);
    ///     }
    ///     Err(GraphError::TaskExecutionFailed(msg)) => {
    ///         eprintln!("Task failed: {}", msg);
    ///     }
    ///     Err(e) => {
    ///         eprintln!("Other error: {}", e);
    ///     }
    /// }
    /// # }
    /// ```
    pub async fn run(&self, session_id: &str) -> Result<ExecutionResult> {
        // 1. Load session
        let mut session = self
            .storage
            .get(session_id)
            .await?
            .ok_or_else(|| GraphError::SessionNotFound(session_id.to_string()))?;

        // 2. Execute current task (exactly one step)
        let result = self.graph.execute_session(&mut session).await?;

        // 3. Persist new state so the next call starts where we left off
        self.storage.save(session).await?;

        Ok(result)
    }

    /// Execute one step with runtime configuration.
    ///
    /// Respects `RunConfig` settings including:
    /// - Dynamic breakpoints (interrupt_before / interrupt_after)
    /// - Timeout override
    /// - Tags and metadata stored in context
    pub async fn run_with_config(
        &self,
        session_id: &str,
        config: &RunConfig,
    ) -> Result<ExecutionResult> {
        let mut session = self
            .storage
            .get(session_id)
            .await?
            .ok_or_else(|| GraphError::SessionNotFound(session_id.to_string()))?;

        // Store config metadata in context for observability
        if !config.tags.is_empty() {
            session
                .context
                .set("__run_tags", config.tags.clone())
                .await;
        }
        if !config.metadata.is_empty() {
            session
                .context
                .set("__run_metadata", config.metadata.clone())
                .await;
        }

        // Check interrupt_before
        if config
            .breakpoints
            .should_interrupt_before(&session.current_task_id)
        {
            self.storage.save(session.clone()).await?;
            return Ok(ExecutionResult {
                response: None,
                status: ExecutionStatus::WaitingForInput,
            });
        }

        // Execute with optional timeout override
        let result = if let Some(timeout_dur) = config.timeout {
            match tokio::time::timeout(timeout_dur, self.graph.execute_session(&mut session)).await
            {
                Ok(r) => r?,
                Err(_) => {
                    self.storage.save(session).await?;
                    return Ok(ExecutionResult {
                        response: None,
                        status: ExecutionStatus::Error(format!(
                            "Execution timed out after {:?}",
                            timeout_dur
                        )),
                    });
                }
            }
        } else {
            self.graph.execute_session(&mut session).await?
        };

        // Check interrupt_after (use the task that just ran from history)
        let last_task = session.task_history.last().cloned().unwrap_or_default();
        if config.breakpoints.should_interrupt_after(&last_task) {
            self.storage.save(session).await?;
            return Ok(ExecutionResult {
                response: result.response,
                status: ExecutionStatus::WaitingForInput,
            });
        }

        self.storage.save(session).await?;
        Ok(result)
    }

    /// Execute the graph for multiple inputs in parallel (batch invoke).
    ///
    /// Each input `Context` gets its own session. Results are returned
    /// in the same order as the inputs.
    ///
    /// # Parameters
    ///
    /// * `start_task` - The starting task ID for all sessions
    /// * `inputs` - Vector of contexts, one per input
    ///
    /// # Returns
    ///
    /// Vector of `ExecutionResult`s in the same order as inputs.
    /// Each result represents running the graph to completion for that input.
    pub async fn run_batch(
        &self,
        start_task: &str,
        inputs: Vec<Context>,
    ) -> Vec<Result<ExecutionResult>> {
        let mut handles = Vec::with_capacity(inputs.len());

        for (i, ctx) in inputs.into_iter().enumerate() {
            let session_id = format!("__batch_{}_{}", start_task, i);
            let session = Session::new_from_task(session_id.clone(), start_task);
            // Copy context data into the session
            // We use a new session with the provided context
            let mut session_with_ctx = session;
            session_with_ctx.context = ctx;

            let storage = self.storage.clone();
            let graph = self.graph.clone();

            handles.push(tokio::spawn(async move {
                storage.save(session_with_ctx).await?;

                let mut final_result = None;
                for _ in 0..100 {
                    // safety limit
                    let mut session = storage
                        .get(&session_id)
                        .await?
                        .ok_or_else(|| GraphError::SessionNotFound(session_id.clone()))?;

                    let result = graph.execute_session(&mut session).await?;
                    storage.save(session).await?;

                    let done = matches!(
                        result.status,
                        ExecutionStatus::Completed
                            | ExecutionStatus::WaitingForInput
                            | ExecutionStatus::Error(_)
                    );
                    final_result = Some(result);
                    if done {
                        break;
                    }
                }

                Ok(final_result.unwrap_or(ExecutionResult {
                    response: None,
                    status: ExecutionStatus::Error("Batch execution exceeded step limit".into()),
                }))
            }));
        }

        let mut results = Vec::with_capacity(handles.len());
        for handle in handles {
            match handle.await {
                Ok(r) => results.push(r),
                Err(e) => results.push(Err(GraphError::TaskExecutionFailed(format!(
                    "Batch task panicked: {}",
                    e
                )))),
            }
        }
        results
    }
}
