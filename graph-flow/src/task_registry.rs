//! Task registry for binding real implementations to agent card capabilities.
//!
//! Instead of using placeholder `CapabilityTask`s, the `TaskRegistry` lets you
//! register real task implementations by name, then compile agent cards that
//! reference those names.
//!
//! # Examples
//!
//! ```rust
//! use graph_flow::task_registry::TaskRegistry;
//! use graph_flow::{Task, TaskResult, NextAction, Context};
//! use async_trait::async_trait;
//! use std::sync::Arc;
//!
//! struct SearchTask;
//!
//! #[async_trait]
//! impl Task for SearchTask {
//!     fn id(&self) -> &str { "search" }
//!     async fn run(&self, _ctx: Context) -> graph_flow::Result<TaskResult> {
//!         Ok(TaskResult::new(Some("searched".to_string()), NextAction::Continue))
//!     }
//! }
//!
//! let mut registry = TaskRegistry::new();
//! registry.register("search", Arc::new(SearchTask));
//! assert!(registry.get("search").is_some());
//! ```

use std::collections::HashMap;
use std::sync::Arc;

use crate::{
    agents::agent_card::{AgentCard, CapabilityTask},
    error::{GraphError, Result},
    graph::{Graph, GraphBuilder},
    task::Task,
};

/// A registry that maps capability names to real task implementations.
///
/// Used to compile agent cards with actual task bindings instead of placeholders.
pub struct TaskRegistry {
    tasks: HashMap<String, Arc<dyn Task>>,
}

impl TaskRegistry {
    pub fn new() -> Self {
        Self {
            tasks: HashMap::new(),
        }
    }

    /// Register a task implementation under the given name.
    pub fn register(&mut self, name: impl Into<String>, task: Arc<dyn Task>) {
        self.tasks.insert(name.into(), task);
    }

    /// Get a registered task by name.
    pub fn get(&self, name: &str) -> Option<Arc<dyn Task>> {
        self.tasks.get(name).cloned()
    }

    /// Check if a task is registered.
    pub fn contains(&self, name: &str) -> bool {
        self.tasks.contains_key(name)
    }

    /// List all registered capability names.
    pub fn names(&self) -> Vec<&String> {
        self.tasks.keys().collect()
    }

    /// Compile an agent card YAML string using registered tasks.
    ///
    /// For capabilities that have a registered task, uses the real implementation.
    /// For capabilities without a registration, falls back to `CapabilityTask` placeholder.
    pub fn compile_agent_card(&self, yaml: &str) -> Result<Arc<Graph>> {
        let card: AgentCard = serde_yaml::from_str(yaml).map_err(|e| {
            GraphError::TaskExecutionFailed(format!("Failed to parse agent card YAML: {}", e))
        })?;

        self.compile_from_def(&card)
    }

    /// Compile from a parsed AgentCard definition.
    pub fn compile_from_def(&self, card: &AgentCard) -> Result<Arc<Graph>> {
        let mut builder = GraphBuilder::new(&card.agent.name);

        // Create tasks: use registry if available, otherwise placeholder
        for cap in &card.agent.capabilities {
            let task: Arc<dyn Task> = if let Some(registered) = self.tasks.get(cap) {
                registered.clone()
            } else {
                CapabilityTask::new(cap.clone())
            };
            builder = builder.add_task(task);
        }

        // Wire workflow steps (same logic as compile_agent_card_from_def)
        for step in &card.agent.workflow {
            if let Some(ref next) = step.next
                && next != "end"
            {
                builder = builder.add_edge(&step.task, next);
            }

            if let (Some(condition_key), Some(on_success), Some(on_failure)) = (
                &step.condition_key,
                &step.on_success,
                &step.on_failure,
            ) {
                let key = condition_key.clone();
                let yes_target = if on_success == "end" {
                    continue;
                } else {
                    on_success.clone()
                };
                let no_target = if on_failure == "end" {
                    continue;
                } else {
                    on_failure.clone()
                };

                builder = builder.add_conditional_edge(
                    &step.task,
                    move |ctx| ctx.get_sync::<bool>(&key).unwrap_or(false),
                    &yes_target,
                    &no_target,
                );
            } else if let (Some(on_success), Some(on_failure)) =
                (&step.on_success, &step.on_failure)
            {
                let task_name = step.task.clone();
                let yes = on_success.clone();
                let no = on_failure.clone();

                if yes != "end" && no != "end" {
                    builder = builder.add_conditional_edge(
                        &step.task,
                        move |ctx| {
                            ctx.get_sync::<bool>(&format!("{}_success", task_name))
                                .unwrap_or(false)
                        },
                        &yes,
                        &no,
                    );
                }
            }
        }

        Ok(Arc::new(builder.build()))
    }
}

impl Default for TaskRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        context::Context,
        task::{NextAction, TaskResult},
    };
    use async_trait::async_trait;

    struct RealSearchTask;

    #[async_trait]
    impl Task for RealSearchTask {
        fn id(&self) -> &str {
            "search"
        }
        async fn run(&self, context: Context) -> Result<TaskResult> {
            context.set("real_search", true).await;
            Ok(TaskResult::new(
                Some("Real search executed".to_string()),
                NextAction::Continue,
            ))
        }
    }

    struct RealSummarizeTask;

    #[async_trait]
    impl Task for RealSummarizeTask {
        fn id(&self) -> &str {
            "summarize"
        }
        async fn run(&self, context: Context) -> Result<TaskResult> {
            context.set("real_summarize", true).await;
            Ok(TaskResult::new(
                Some("Real summarize executed".to_string()),
                NextAction::End,
            ))
        }
    }

    #[test]
    fn test_registry_basic() {
        let mut registry = TaskRegistry::new();
        registry.register("search", Arc::new(RealSearchTask));

        assert!(registry.contains("search"));
        assert!(!registry.contains("unknown"));
        assert!(registry.get("search").is_some());
    }

    #[tokio::test]
    async fn test_registry_compile_with_real_tasks() {
        let mut registry = TaskRegistry::new();
        registry.register("search", Arc::new(RealSearchTask));
        registry.register("summarize", Arc::new(RealSummarizeTask));

        let yaml = r#"
agent:
  name: test_agent
  description: Test
  capabilities:
    - search
    - summarize
  workflow:
    - task: search
      next: summarize
    - task: summarize
      next: end
"#;

        let graph = registry.compile_agent_card(yaml).unwrap();

        // Execute and verify real tasks ran
        let mut session = crate::Session::new_from_task("test".to_string(), "search");

        let result = graph.execute_session(&mut session).await.unwrap();
        assert!(result.response.unwrap().contains("Real search"));

        let real: bool = session.context.get("real_search").await.unwrap_or(false);
        assert!(real);
    }

    #[tokio::test]
    async fn test_registry_fallback_to_placeholder() {
        let mut registry = TaskRegistry::new();
        // Only register "search", not "analyze"
        registry.register("search", Arc::new(RealSearchTask));

        let yaml = r#"
agent:
  name: mixed_agent
  description: Some real, some placeholder
  capabilities:
    - search
    - analyze
  workflow:
    - task: search
      next: analyze
    - task: analyze
      next: end
"#;

        let graph = registry.compile_agent_card(yaml).unwrap();
        let mut session = crate::Session::new_from_task("test".to_string(), "search");

        // First step: real task
        let result = graph.execute_session(&mut session).await.unwrap();
        assert!(result.response.unwrap().contains("Real search"));

        // Second step: placeholder (CapabilityTask)
        let result = graph.execute_session(&mut session).await.unwrap();
        assert!(result.response.unwrap().contains("Capability 'analyze'"));
    }
}
