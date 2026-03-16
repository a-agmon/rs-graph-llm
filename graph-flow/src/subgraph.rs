//! Subgraph support for graph-flow.
//!
//! A subgraph is a [`Task`] that contains its own [`Graph`].
//! When executed, it runs the inner graph to completion and returns the result.
//!
//! This maps to LangGraph's subgraph feature, which allows composing graphs
//! hierarchically.
//!
//! # Examples
//!
//! ```rust
//! use graph_flow::{Task, TaskResult, NextAction, Context, GraphBuilder};
//! use graph_flow::subgraph::SubgraphTask;
//! use async_trait::async_trait;
//! use std::sync::Arc;
//!
//! struct InnerTask;
//!
//! #[async_trait]
//! impl Task for InnerTask {
//!     fn id(&self) -> &str { "inner" }
//!     async fn run(&self, ctx: Context) -> graph_flow::Result<TaskResult> {
//!         let input: String = ctx.get("input").await.unwrap_or_default();
//!         ctx.set("inner_result", format!("processed: {}", input)).await;
//!         Ok(TaskResult::new(Some("inner done".to_string()), NextAction::End))
//!     }
//! }
//!
//! # #[tokio::main]
//! # async fn main() -> graph_flow::Result<()> {
//! let inner_graph = Arc::new(
//!     GraphBuilder::new("inner_graph")
//!         .add_task(Arc::new(InnerTask))
//!         .build()
//! );
//!
//! let subgraph = SubgraphTask::new("my_subgraph", inner_graph);
//!
//! let ctx = Context::new();
//! ctx.set("input", "hello").await;
//! let result = subgraph.run(ctx.clone()).await?;
//!
//! let inner_result: String = ctx.get("inner_result").await.unwrap();
//! assert_eq!(inner_result, "processed: hello");
//! # Ok(())
//! # }
//! ```

use async_trait::async_trait;
use std::sync::Arc;

use crate::{
    context::Context,
    error::{GraphError, Result},
    graph::Graph,
    storage::Session,
    task::{NextAction, Task, TaskResult},
};

/// A task that executes an inner graph as a subgraph.
///
/// The subgraph shares the parent's Context, so data flows naturally
/// between the parent graph and the subgraph.
///
/// The subgraph runs to completion (all tasks execute until `End` is reached
/// or no more tasks are available).
pub struct SubgraphTask {
    name: String,
    inner_graph: Arc<Graph>,
    /// Optional: map specific context keys from parent to child
    input_mappings: Vec<(String, String)>,
    /// Optional: map specific context keys from child back to parent
    output_mappings: Vec<(String, String)>,
}

impl SubgraphTask {
    /// Create a new SubgraphTask wrapping an inner graph.
    pub fn new(name: impl Into<String>, inner_graph: Arc<Graph>) -> Arc<Self> {
        Arc::new(Self {
            name: name.into(),
            inner_graph,
            input_mappings: Vec::new(),
            output_mappings: Vec::new(),
        })
    }

    /// Create a SubgraphTask with input and output mappings.
    pub fn with_mappings(
        name: impl Into<String>,
        inner_graph: Arc<Graph>,
        input_mappings: Vec<(String, String)>,
        output_mappings: Vec<(String, String)>,
    ) -> Arc<Self> {
        Arc::new(Self {
            name: name.into(),
            inner_graph,
            input_mappings,
            output_mappings,
        })
    }
}

#[async_trait]
impl Task for SubgraphTask {
    fn id(&self) -> &str {
        &self.name
    }

    async fn run(&self, context: Context) -> Result<TaskResult> {
        // Get the start task of the inner graph
        let start_task_id = self
            .inner_graph
            .start_task_id()
            .ok_or_else(|| {
                GraphError::TaskNotFound(format!(
                    "Subgraph '{}' has no start task",
                    self.name
                ))
            })?;

        // Create a session for the subgraph execution.
        // The subgraph shares the parent context directly so data flows through.
        let mut session = Session {
            id: format!("subgraph_{}", self.name),
            graph_id: self.inner_graph.id.clone(),
            current_task_id: start_task_id,
            status_message: None,
            context: context.clone(),
            task_history: Vec::new(),
        };

        // Apply input mappings (copy values within the shared context)
        for (parent_key, child_key) in &self.input_mappings {
            if let Some(value) = context.get::<serde_json::Value>(parent_key).await {
                context.set(child_key.clone(), value).await;
            }
        }

        // Execute the subgraph to completion
        let mut last_response = None;
        let mut iterations = 0;
        const MAX_ITERATIONS: usize = 1000;

        loop {
            iterations += 1;
            if iterations > MAX_ITERATIONS {
                return Err(GraphError::TaskExecutionFailed(format!(
                    "Subgraph '{}' exceeded maximum iterations ({})",
                    self.name, MAX_ITERATIONS
                )));
            }

            let result = self.inner_graph.execute_session(&mut session).await?;

            if result.response.is_some() {
                last_response = result.response;
            }

            match result.status {
                crate::graph::ExecutionStatus::Completed => break,
                crate::graph::ExecutionStatus::WaitingForInput => {
                    // Subgraph is waiting for input — bubble up
                    return Ok(TaskResult::new(
                        last_response,
                        NextAction::WaitForInput,
                    ));
                }
                crate::graph::ExecutionStatus::Error(e) => {
                    return Err(GraphError::TaskExecutionFailed(format!(
                        "Subgraph '{}' error: {}",
                        self.name, e
                    )));
                }
                crate::graph::ExecutionStatus::Paused { .. } => {
                    // Continue executing the next step
                    continue;
                }
            }
        }

        // Apply output mappings
        for (child_key, parent_key) in &self.output_mappings {
            if let Some(value) = context.get::<serde_json::Value>(child_key).await {
                context.set(parent_key.clone(), value).await;
            }
        }

        Ok(TaskResult::new(last_response, NextAction::Continue))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::GraphBuilder;
    use async_trait::async_trait;

    struct AddOneTask;

    #[async_trait]
    impl Task for AddOneTask {
        fn id(&self) -> &str {
            "add_one"
        }
        async fn run(&self, ctx: Context) -> Result<TaskResult> {
            let val: i32 = ctx.get("value").await.unwrap_or(0);
            ctx.set("value", val + 1).await;
            Ok(TaskResult::new(
                Some(format!("value is now {}", val + 1)),
                NextAction::End,
            ))
        }
    }

    struct DoubleTask;

    #[async_trait]
    impl Task for DoubleTask {
        fn id(&self) -> &str {
            "double"
        }
        async fn run(&self, ctx: Context) -> Result<TaskResult> {
            let val: i32 = ctx.get("value").await.unwrap_or(0);
            ctx.set("value", val * 2).await;
            Ok(TaskResult::new(
                Some(format!("value is now {}", val * 2)),
                NextAction::Continue,
            ))
        }
    }

    #[tokio::test]
    async fn test_subgraph_basic() {
        let inner_graph = Arc::new(
            GraphBuilder::new("inner")
                .add_task(Arc::new(AddOneTask))
                .build(),
        );

        let subgraph = SubgraphTask::new("sub", inner_graph);

        let ctx = Context::new();
        ctx.set("value", 10).await;

        let result = subgraph.run(ctx.clone()).await.unwrap();
        assert_eq!(result.next_action, NextAction::Continue);

        let val: i32 = ctx.get("value").await.unwrap();
        assert_eq!(val, 11);
    }

    #[tokio::test]
    async fn test_subgraph_multi_step() {
        let inner_graph = Arc::new(
            GraphBuilder::new("inner")
                .add_task(Arc::new(DoubleTask))
                .add_task(Arc::new(AddOneTask))
                .add_edge("double", "add_one")
                .build(),
        );

        let subgraph = SubgraphTask::new("sub", inner_graph);

        let ctx = Context::new();
        ctx.set("value", 5).await;

        let result = subgraph.run(ctx.clone()).await.unwrap();
        assert_eq!(result.next_action, NextAction::Continue);

        // 5 * 2 = 10, then 10 + 1 = 11
        let val: i32 = ctx.get("value").await.unwrap();
        assert_eq!(val, 11);
    }

    #[tokio::test]
    async fn test_subgraph_in_parent_graph() {
        // Inner graph: add_one
        let inner_graph = Arc::new(
            GraphBuilder::new("inner")
                .add_task(Arc::new(AddOneTask))
                .build(),
        );

        let subgraph_task = SubgraphTask::new("subgraph_step", inner_graph);
        let double_task = Arc::new(DoubleTask);

        // Parent graph: double -> subgraph(add_one)
        let parent_graph = GraphBuilder::new("parent")
            .add_task(double_task.clone())
            .add_task(subgraph_task.clone())
            .add_edge("double", "subgraph_step")
            .build();

        let ctx = Context::new();
        ctx.set("value", 3).await;

        // Execute double task
        let mut session = Session {
            id: "test".to_string(),
            graph_id: "parent".to_string(),
            current_task_id: "double".to_string(),
            status_message: None,
            context: ctx.clone(),
            task_history: Vec::new(),
        };

        // Step 1: double (3 -> 6)
        let _ = parent_graph.execute_session(&mut session).await.unwrap();
        // Step 2: subgraph add_one (6 -> 7)
        let _ = parent_graph.execute_session(&mut session).await.unwrap();

        let val: i32 = ctx.get("value").await.unwrap();
        assert_eq!(val, 7);
    }
}
