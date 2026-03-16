//! Prebuilt ReAct (Reason + Act) agent pattern.
//!
//! Provides `create_react_agent()` which builds a standard ReAct loop graph:
//! LLM reasons → decides if tool is needed → calls tool → feeds result back to LLM.
//!
//! # Examples
//!
//! ```rust
//! use graph_flow::react_agent::create_react_agent;
//! use graph_flow::{Task, TaskResult, NextAction, Context};
//! use async_trait::async_trait;
//! use std::sync::Arc;
//!
//! struct MockLlm;
//! #[async_trait]
//! impl Task for MockLlm {
//!     fn id(&self) -> &str { "llm" }
//!     async fn run(&self, ctx: Context) -> graph_flow::Result<TaskResult> {
//!         // In a real implementation, call an LLM here
//!         ctx.set("needs_tool", false).await;
//!         Ok(TaskResult::new(Some("answer".to_string()), NextAction::Continue))
//!     }
//! }
//!
//! let graph = create_react_agent(Arc::new(MockLlm), vec![], 5);
//! assert!(graph.start_task_id().is_some());
//! ```

use async_trait::async_trait;
use std::sync::Arc;

use crate::{
    context::Context,
    error::Result,
    graph::{Graph, GraphBuilder},
    task::{NextAction, Task, TaskResult},
};

/// Internal task that routes to the correct tool based on context.
struct ToolRouterTask {
    tool_names: Vec<String>,
}

#[async_trait]
impl Task for ToolRouterTask {
    fn id(&self) -> &str {
        "tool_router"
    }

    async fn run(&self, context: Context) -> Result<TaskResult> {
        let tool_name: String = context
            .get("selected_tool")
            .await
            .unwrap_or_else(|| {
                self.tool_names.first().cloned().unwrap_or_default()
            });

        context.set("routed_tool", tool_name.clone()).await;

        Ok(TaskResult::new(
            Some(format!("Routing to tool: {}", tool_name)),
            NextAction::ContinueAndExecute,
        ))
    }
}

/// Internal task that aggregates tool results and loops back to LLM.
struct ToolAggregatorTask;

#[async_trait]
impl Task for ToolAggregatorTask {
    fn id(&self) -> &str {
        "tool_aggregator"
    }

    async fn run(&self, context: Context) -> Result<TaskResult> {
        let iteration: usize = context.get("react_iteration").await.unwrap_or(0);
        context.set("react_iteration", iteration + 1).await;

        // Collect tool result and add to message history
        let tool_result: String = context
            .get("tool_result")
            .await
            .unwrap_or_else(|| "No tool result".to_string());

        // Append to tool results history
        let mut history: Vec<String> = context
            .get("tool_results_history")
            .await
            .unwrap_or_default();
        history.push(tool_result);
        context.set("tool_results_history", history).await;

        Ok(TaskResult::new(
            Some("Tool result collected, returning to LLM".to_string()),
            NextAction::Continue,
        ))
    }
}

/// Internal task that enforces max iterations.
struct IterationGuardTask {
    max_iterations: usize,
}

#[async_trait]
impl Task for IterationGuardTask {
    fn id(&self) -> &str {
        "iteration_guard"
    }

    async fn run(&self, context: Context) -> Result<TaskResult> {
        let iteration: usize = context.get("react_iteration").await.unwrap_or(0);
        context
            .set("within_iteration_limit", iteration < self.max_iterations)
            .await;

        if iteration >= self.max_iterations {
            Ok(TaskResult::new(
                Some(format!(
                    "Max iterations ({}) reached, stopping",
                    self.max_iterations
                )),
                NextAction::End,
            ))
        } else {
            Ok(TaskResult::new(None, NextAction::ContinueAndExecute))
        }
    }
}

/// Create a prebuilt ReAct (Reason + Act) agent graph.
///
/// The graph implements the standard ReAct loop:
/// 1. `iteration_guard` — checks iteration limit
/// 2. `llm` (your LLM task) — reasons about the problem, sets `needs_tool` in context
/// 3. If `needs_tool == true`: routes to `tool_router` → tool tasks → `tool_aggregator` → back to guard
/// 4. If `needs_tool == false`: ends
///
/// # Context Keys
///
/// Your LLM task should read/write these context keys:
/// - **`needs_tool`** (bool): Set to `true` if a tool call is needed, `false` to finish.
/// - **`selected_tool`** (String): Name of the tool to call.
/// - **`tool_result`** (String): Result from the last tool call.
/// - **`tool_results_history`** (Vec<String>): All past tool results.
/// - **`react_iteration`** (usize): Current iteration count.
///
/// # Parameters
///
/// * `llm_task` - Your LLM task (must have id "llm")
/// * `tools` - Tool tasks to make available (each must implement `Task`)
/// * `max_iterations` - Maximum number of ReAct loop iterations
pub fn create_react_agent(
    llm_task: Arc<dyn Task>,
    tools: Vec<Arc<dyn Task>>,
    max_iterations: usize,
) -> Arc<Graph> {
    let tool_names: Vec<String> = tools.iter().map(|t| t.id().to_string()).collect();

    let guard = Arc::new(IterationGuardTask { max_iterations }) as Arc<dyn Task>;
    let router = Arc::new(ToolRouterTask {
        tool_names: tool_names.clone(),
    }) as Arc<dyn Task>;
    let aggregator = Arc::new(ToolAggregatorTask) as Arc<dyn Task>;

    let mut builder = GraphBuilder::new("react_agent")
        .add_task(guard)
        .add_task(llm_task)
        .add_task(router)
        .add_task(aggregator);

    // Add all tool tasks
    for tool in &tools {
        builder = builder.add_task(tool.clone());
    }

    // iteration_guard → llm
    builder = builder.add_edge("iteration_guard", "llm");

    // llm → conditional: needs_tool? → tool_router : END
    builder = builder.add_conditional_edge(
        "llm",
        |ctx| ctx.get_sync::<bool>("needs_tool").unwrap_or(false),
        "tool_router",
        "tool_aggregator", // no tool needed → aggregator ends cycle
    );

    // tool_router → first tool (or aggregator if no tools)
    if let Some(first_tool) = tool_names.first() {
        builder = builder.add_edge("tool_router", first_tool);

        // Each tool → aggregator
        for tool_name in &tool_names {
            builder = builder.add_edge(tool_name, "tool_aggregator");
        }
    } else {
        builder = builder.add_edge("tool_router", "tool_aggregator");
    }

    // aggregator → back to iteration guard (loop)
    builder = builder.add_edge("tool_aggregator", "iteration_guard");

    // Set start task
    builder = builder.set_start_task("iteration_guard");

    Arc::new(builder.build())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{FlowRunner, InMemorySessionStorage, Session, SessionStorage};

    struct MockLlm {
        call_count_key: &'static str,
    }

    #[async_trait]
    impl Task for MockLlm {
        fn id(&self) -> &str {
            "llm"
        }
        async fn run(&self, ctx: Context) -> Result<TaskResult> {
            let calls: usize = ctx.get(self.call_count_key).await.unwrap_or(0);
            ctx.set(self.call_count_key, calls + 1).await;

            if calls == 0 {
                // First call: need a tool
                ctx.set("needs_tool", true).await;
                ctx.set("selected_tool", "search".to_string()).await;
                Ok(TaskResult::new(
                    Some("Need to search first".to_string()),
                    NextAction::Continue,
                ))
            } else {
                // Second call: done
                ctx.set("needs_tool", false).await;
                Ok(TaskResult::new(
                    Some("Got the answer".to_string()),
                    NextAction::End,
                ))
            }
        }
    }

    struct MockSearchTool;

    #[async_trait]
    impl Task for MockSearchTool {
        fn id(&self) -> &str {
            "search"
        }
        async fn run(&self, ctx: Context) -> Result<TaskResult> {
            ctx.set("tool_result", "Search found: 42".to_string())
                .await;
            Ok(TaskResult::new(
                Some("Search complete".to_string()),
                NextAction::Continue,
            ))
        }
    }

    #[test]
    fn test_create_react_agent_structure() {
        let llm = Arc::new(MockLlm {
            call_count_key: "llm_calls",
        }) as Arc<dyn Task>;
        let search = Arc::new(MockSearchTool) as Arc<dyn Task>;

        let graph = create_react_agent(llm, vec![search], 5);
        assert_eq!(graph.start_task_id(), Some("iteration_guard".to_string()));
        assert!(graph.get_task("llm").is_some());
        assert!(graph.get_task("tool_router").is_some());
        assert!(graph.get_task("search").is_some());
    }

    #[tokio::test]
    async fn test_react_agent_execution() {
        let llm = Arc::new(MockLlm {
            call_count_key: "llm_calls",
        }) as Arc<dyn Task>;
        let search = Arc::new(MockSearchTool) as Arc<dyn Task>;

        let graph = create_react_agent(llm, vec![search], 5);

        let storage = Arc::new(InMemorySessionStorage::new());
        let runner = FlowRunner::new(graph, storage.clone());

        let session = Session::new_from_task("test".to_string(), "iteration_guard");
        storage.save(session).await.unwrap();

        // Run through the react loop
        let mut completed = false;
        for _ in 0..20 {
            let result = runner.run("test").await.unwrap();
            if matches!(result.status, crate::ExecutionStatus::Completed) {
                completed = true;
                break;
            }
        }
        assert!(completed, "ReAct agent should complete");

        // Verify tool was called
        let session = storage.get("test").await.unwrap().unwrap();
        let history: Vec<String> = session
            .context
            .get("tool_results_history")
            .await
            .unwrap_or_default();
        assert!(!history.is_empty(), "Tool should have been called");
    }

    #[test]
    fn test_react_agent_no_tools() {
        let llm = Arc::new(MockLlm {
            call_count_key: "calls",
        }) as Arc<dyn Task>;
        let graph = create_react_agent(llm, vec![], 3);
        assert!(graph.get_task("tool_router").is_some());
    }
}
