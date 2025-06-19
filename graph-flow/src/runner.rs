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
//! ```rust
//! // At startup
//! struct AppState {
//!     flow_runner: FlowRunner,
//! }
//!
//! // In request handler
//! let result = state.flow_runner.run(&session_id).await?;
//! ```
//! **Pros**: Most efficient, zero allocation per request  
//! **Cons**: Requires the same graph for all requests
//!
//! ### Pattern 2: Per-Request FlowRunner
//! Create `FlowRunner` fresh for each request:
//! ```rust
//! // In request handler
//! let runner = FlowRunner::new(graph.clone(), storage.clone());
//! let result = runner.run(&session_id).await?;
//! ```
//! **Pros**: Flexible, can use different graphs per request  
//! **Cons**: Tiny allocation cost per request (still very cheap)
//!
//! ### Pattern 3: Manual (Original)
//! Use `Graph::execute_session` directly:
//! ```rust
//! let mut session = storage.get(&session_id).await?.unwrap();
//! let result = graph.execute_session(&mut session).await?;
//! storage.save(session).await?;
//! ```
//! **Pros**: Maximum control  
//! **Cons**: More boilerplate, easy to forget session.save()
//!
//! ## Performance Characteristics
//! - **FlowRunner creation cost**: ~2 pointer copies (negligible)
//! - **Memory overhead**: 16 bytes (2 × Arc<T>)
//! - **Runtime cost**: Identical to manual approach
//!
//! For high-throughput services, Pattern 1 is recommended. For services with different
//! graphs per request or complex routing, Pattern 2 is perfectly fine.

use std::sync::Arc;

use crate::{
    error::{GraphError, Result},
    graph::{ExecutionResult, Graph},
    storage::SessionStorage,
};

/// High-level helper that orchestrates the common _load → execute → save_ pattern.
#[derive(Clone)]
pub struct FlowRunner {
    graph: Arc<Graph>,
    storage: Arc<dyn SessionStorage>,
}

impl FlowRunner {
    /// Create a new `FlowRunner` from an `Arc<Graph>` and any `SessionStorage` implementation.
    pub fn new(graph: Arc<Graph>, storage: Arc<dyn SessionStorage>) -> Self {
        Self { graph, storage }
    }

    /// Execute **exactly one** task for the given `session_id` and persist the updated session.
    ///
    /// Returns the same [`ExecutionResult`] that `Graph::execute_session` does, so callers can
    /// still inspect the assistant's response and the status (`WaitingForInput`, `Completed`, …).
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
}
