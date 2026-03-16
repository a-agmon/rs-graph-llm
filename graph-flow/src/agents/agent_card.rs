//! Agent Card YAML → GraphBuilder compiler.
//!
//! Agent cards are declarative YAML definitions of agent workflows.
//! This module parses them and compiles them into graph-flow `Graph`s.
//!
//! # Agent Card Format
//!
//! ```yaml
//! agent:
//!   name: "researcher"
//!   description: "Finds and synthesizes information"
//!   capabilities:
//!     - search
//!     - summarize
//!   tools:
//!     - name: web_search
//!       mcp_server: "https://search.mcp.server/sse"
//!   workflow:
//!     - task: search
//!       next: summarize
//!     - task: summarize
//!       next: end
//! ```
//!
//! # Examples
//!
//! ```rust
//! use graph_flow::agents::agent_card::{AgentCard, compile_agent_card};
//!
//! let yaml = r#"
//! agent:
//!   name: test_agent
//!   description: A test agent
//!   capabilities:
//!     - step_one
//!     - step_two
//!   workflow:
//!     - task: step_one
//!       next: step_two
//!     - task: step_two
//!       next: end
//! "#;
//!
//! let card: AgentCard = serde_yaml::from_str(yaml).unwrap();
//! assert_eq!(card.agent.name, "test_agent");
//! ```

use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::{
    context::Context,
    error::{GraphError, Result},
    graph::{Graph, GraphBuilder},
    task::{NextAction, Task, TaskResult},
};

/// Top-level agent card definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentCard {
    pub agent: AgentDef,
}

/// Agent definition within a card.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentDef {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub capabilities: Vec<String>,
    #[serde(default)]
    pub planes: Option<PlanesDef>,
    #[serde(default)]
    pub tools: Vec<ToolDef>,
    #[serde(default)]
    pub workflow: Vec<WorkflowStep>,
}

/// Plane read/write definitions for knowledge graph integration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanesDef {
    #[serde(default)]
    pub read: Vec<String>,
    #[serde(default)]
    pub write: Vec<String>,
}

/// Tool definition (MCP server reference).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDef {
    pub name: String,
    #[serde(default)]
    pub mcp_server: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
}

/// A single step in the agent workflow.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowStep {
    pub task: String,
    /// Next task (unconditional edge)
    #[serde(default)]
    pub next: Option<String>,
    /// Conditional: if this context key is truthy, go to this task
    #[serde(default)]
    pub on_success: Option<String>,
    /// Conditional: if the context key is falsy, go to this task
    #[serde(default)]
    pub on_failure: Option<String>,
    /// Conditional edge based on a specific context key
    #[serde(default)]
    pub condition_key: Option<String>,
}

/// A generic capability task that stores its name in context when executed.
///
/// This is a placeholder task for capabilities defined in agent cards.
/// In production, each capability would be mapped to a real implementation.
pub struct CapabilityTask {
    name: String,
}

impl CapabilityTask {
    pub fn new(name: impl Into<String>) -> Arc<Self> {
        Arc::new(Self { name: name.into() })
    }
}

#[async_trait::async_trait]
impl Task for CapabilityTask {
    fn id(&self) -> &str {
        &self.name
    }

    async fn run(&self, context: Context) -> Result<TaskResult> {
        // Mark this capability as executed
        context
            .set(
                format!("capability_{}_executed", self.name),
                true,
            )
            .await;

        Ok(TaskResult::new(
            Some(format!("Capability '{}' executed", self.name)),
            NextAction::Continue,
        ))
    }
}

/// Compile an agent card YAML string into a graph-flow Graph.
///
/// This creates a `CapabilityTask` for each capability defined in the card,
/// and wires them together according to the workflow definition.
///
/// # Examples
///
/// ```rust
/// use graph_flow::agents::agent_card::compile_agent_card;
///
/// let yaml = r#"
/// agent:
///   name: simple_agent
///   description: A simple test agent
///   capabilities:
///     - analyze
///     - report
///   workflow:
///     - task: analyze
///       next: report
///     - task: report
///       next: end
/// "#;
///
/// let graph = compile_agent_card(yaml).unwrap();
/// assert!(graph.start_task_id().is_some());
/// ```
pub fn compile_agent_card(yaml: &str) -> Result<Arc<Graph>> {
    let card: AgentCard = serde_yaml::from_str(yaml).map_err(|e| {
        GraphError::TaskExecutionFailed(format!("Failed to parse agent card YAML: {}", e))
    })?;

    compile_agent_card_from_def(&card)
}

/// Compile an agent card definition into a graph-flow Graph.
pub fn compile_agent_card_from_def(card: &AgentCard) -> Result<Arc<Graph>> {
    let mut builder = GraphBuilder::new(&card.agent.name);

    // Create tasks for each capability
    for cap in &card.agent.capabilities {
        let task = CapabilityTask::new(cap.clone());
        builder = builder.add_task(task);
    }

    // Wire workflow steps
    for step in &card.agent.workflow {
        if let Some(ref next) = step.next {
            if next != "end" {
                builder = builder.add_edge(&step.task, next);
            }
            // "end" means no outgoing edge — task will naturally end
        }

        // Handle conditional edges
        if let (Some(condition_key), Some(on_success), Some(on_failure)) = (
            &step.condition_key,
            &step.on_success,
            &step.on_failure,
        ) {
            let key = condition_key.clone();
            let yes_target = if on_success == "end" {
                // Can't really point to "end" as a task, so we skip
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
            // Default condition: check "<task>_success" key
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::Session;

    #[test]
    fn test_parse_agent_card() {
        let yaml = r#"
agent:
  name: researcher
  description: Finds and synthesizes information
  capabilities:
    - search
    - analyze
    - summarize
  tools:
    - name: web_search
      mcp_server: "https://search.mcp.server/sse"
  workflow:
    - task: search
      next: analyze
    - task: analyze
      next: summarize
    - task: summarize
      next: end
"#;

        let card: AgentCard = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(card.agent.name, "researcher");
        assert_eq!(card.agent.capabilities.len(), 3);
        assert_eq!(card.agent.tools.len(), 1);
        assert_eq!(card.agent.workflow.len(), 3);
    }

    #[test]
    fn test_compile_agent_card() {
        let yaml = r#"
agent:
  name: test_agent
  description: Test
  capabilities:
    - step_a
    - step_b
  workflow:
    - task: step_a
      next: step_b
    - task: step_b
      next: end
"#;

        let graph = compile_agent_card(yaml).unwrap();
        assert_eq!(graph.start_task_id(), Some("step_a".to_string()));
        assert!(graph.get_task("step_a").is_some());
        assert!(graph.get_task("step_b").is_some());
    }

    #[tokio::test]
    async fn test_execute_compiled_agent() {
        let yaml = r#"
agent:
  name: exec_test
  description: Test execution
  capabilities:
    - alpha
    - beta
  workflow:
    - task: alpha
      next: beta
    - task: beta
      next: end
"#;

        let graph = compile_agent_card(yaml).unwrap();

        let mut session = Session::new_from_task("s1".to_string(), "alpha");

        // Execute alpha
        let result = graph.execute_session(&mut session).await.unwrap();
        assert!(result.response.is_some());

        // Execute beta
        let result = graph.execute_session(&mut session).await.unwrap();
        assert!(result.response.is_some());

        // Check capabilities were marked as executed
        let alpha_done: bool = session
            .context
            .get("capability_alpha_executed")
            .await
            .unwrap_or(false);
        assert!(alpha_done);

        let beta_done: bool = session
            .context
            .get("capability_beta_executed")
            .await
            .unwrap_or(false);
        assert!(beta_done);
    }

    #[test]
    fn test_parse_conditional_workflow() {
        let yaml = r#"
agent:
  name: conditional_agent
  description: Agent with conditional routing
  capabilities:
    - classify
    - handle_positive
    - handle_negative
  workflow:
    - task: classify
      condition_key: is_positive
      on_success: handle_positive
      on_failure: handle_negative
    - task: handle_positive
      next: end
    - task: handle_negative
      next: end
"#;

        let card: AgentCard = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(card.agent.workflow[0].condition_key, Some("is_positive".to_string()));
        assert_eq!(card.agent.workflow[0].on_success, Some("handle_positive".to_string()));
    }

    #[test]
    fn test_parse_agent_with_planes() {
        let yaml = r#"
agent:
  name: graph_agent
  description: Agent with knowledge graph planes
  capabilities:
    - query
  planes:
    read:
      - query_s
      - query_p
    write:
      - result_s
  workflow:
    - task: query
      next: end
"#;

        let card: AgentCard = serde_yaml::from_str(yaml).unwrap();
        let planes = card.agent.planes.unwrap();
        assert_eq!(planes.read.len(), 2);
        assert_eq!(planes.write.len(), 1);
    }
}
