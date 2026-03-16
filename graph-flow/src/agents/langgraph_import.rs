//! LangGraph workflow import — converts LangGraph JSON/YAML definitions to graph-flow.
//!
//! This module enables importing existing LangGraph workflow definitions
//! (JSON or YAML format) and compiling them into graph-flow `Graph`s.
//!
//! # Supported Node Types
//!
//! - `llm` — LLM inference task (stores prompt/response in context)
//! - `tool` — Tool call task (maps to McpToolTask)
//! - `retriever` — Document retrieval task
//! - `human` — Human-in-the-loop input
//! - `custom` — Custom task (placeholder)
//!
//! # Examples
//!
//! ```rust
//! use graph_flow::agents::langgraph_import::{LangGraphDef, import_langgraph_workflow};
//!
//! let def = LangGraphDef {
//!     name: "simple_chain".to_string(),
//!     nodes: vec![
//!         graph_flow::agents::langgraph_import::NodeDef {
//!             name: "prompt".to_string(),
//!             node_type: "llm".to_string(),
//!             config: serde_json::json!({"model": "gpt-4"}),
//!         },
//!         graph_flow::agents::langgraph_import::NodeDef {
//!             name: "output".to_string(),
//!             node_type: "custom".to_string(),
//!             config: serde_json::json!({}),
//!         },
//!     ],
//!     edges: vec![
//!         graph_flow::agents::langgraph_import::EdgeDef {
//!             from: "prompt".to_string(),
//!             to: "output".to_string(),
//!             condition: None,
//!             condition_key: None,
//!         },
//!     ],
//!     entry_point: Some("prompt".to_string()),
//! };
//!
//! let graph = import_langgraph_workflow(&def).unwrap();
//! assert!(graph.get_task("prompt").is_some());
//! ```

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::{
    context::Context,
    error::{GraphError, Result},
    graph::{Graph, GraphBuilder},
    task::{NextAction, Task, TaskResult},
};

/// A LangGraph workflow definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LangGraphDef {
    pub name: String,
    pub nodes: Vec<NodeDef>,
    pub edges: Vec<EdgeDef>,
    #[serde(default)]
    pub entry_point: Option<String>,
}

/// A node in a LangGraph workflow.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeDef {
    pub name: String,
    pub node_type: String,
    #[serde(default)]
    pub config: serde_json::Value,
}

/// An edge in a LangGraph workflow.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeDef {
    pub from: String,
    pub to: String,
    /// Optional condition expression (Python-style, for display only)
    #[serde(default)]
    pub condition: Option<String>,
    /// Context key to evaluate for condition (true/false)
    #[serde(default)]
    pub condition_key: Option<String>,
}

/// LLM task — reads prompt from context, writes response.
struct LlmTask {
    name: String,
    config: serde_json::Value,
}

#[async_trait]
impl Task for LlmTask {
    fn id(&self) -> &str {
        &self.name
    }

    async fn run(&self, context: Context) -> Result<TaskResult> {
        let prompt: String = context
            .get("prompt")
            .await
            .unwrap_or_else(|| "No prompt provided".to_string());

        let model = self
            .config
            .get("model")
            .and_then(|v| v.as_str())
            .unwrap_or("default");

        // Store LLM metadata in context
        context
            .set(
                format!("{}_model", self.name),
                model.to_string(),
            )
            .await;
        context
            .set(
                format!("{}_prompt", self.name),
                prompt.clone(),
            )
            .await;

        // Placeholder: in production, this would call an actual LLM
        let response = format!("[LLM:{}/{}] Response to: {}", model, self.name, prompt);
        context
            .set(format!("{}_response", self.name), response.clone())
            .await;

        Ok(TaskResult::new(Some(response), NextAction::Continue))
    }
}

/// Tool call task — placeholder for MCP tool integration.
struct ToolCallTask {
    name: String,
    config: serde_json::Value,
}

#[async_trait]
impl Task for ToolCallTask {
    fn id(&self) -> &str {
        &self.name
    }

    async fn run(&self, context: Context) -> Result<TaskResult> {
        let tool_name = self
            .config
            .get("tool_name")
            .and_then(|v| v.as_str())
            .unwrap_or(&self.name);

        let input: serde_json::Value = context
            .get("tool_input")
            .await
            .unwrap_or(serde_json::Value::Null);

        // Placeholder: store tool call info
        context
            .set(
                format!("{}_tool_name", self.name),
                tool_name.to_string(),
            )
            .await;
        context
            .set(format!("{}_input", self.name), input.clone())
            .await;

        let result = serde_json::json!({
            "tool": tool_name,
            "status": "called",
            "input": input,
        });
        context
            .set(format!("{}_result", self.name), result.clone())
            .await;

        Ok(TaskResult::new(
            Some(format!("Tool '{}' called", tool_name)),
            NextAction::Continue,
        ))
    }
}

/// Retriever task — placeholder for document retrieval.
struct RetrieverTask {
    name: String,
    #[allow(dead_code)]
    config: serde_json::Value,
}

#[async_trait]
impl Task for RetrieverTask {
    fn id(&self) -> &str {
        &self.name
    }

    async fn run(&self, context: Context) -> Result<TaskResult> {
        let query: String = context
            .get("query")
            .await
            .unwrap_or_else(|| "no query".to_string());

        context
            .set(
                format!("{}_query", self.name),
                query.clone(),
            )
            .await;

        let docs = serde_json::json!([
            {"content": format!("Document relevant to: {}", query), "score": 0.95}
        ]);
        context
            .set(format!("{}_documents", self.name), docs)
            .await;

        Ok(TaskResult::new(
            Some(format!("Retrieved documents for: {}", query)),
            NextAction::Continue,
        ))
    }
}

/// Human input task — waits for input.
struct HumanInputTask {
    name: String,
}

#[async_trait]
impl Task for HumanInputTask {
    fn id(&self) -> &str {
        &self.name
    }

    async fn run(&self, context: Context) -> Result<TaskResult> {
        let has_input: bool = context
            .get::<String>("human_input")
            .await
            .is_some();

        if has_input {
            Ok(TaskResult::new(
                Some("Human input received".to_string()),
                NextAction::Continue,
            ))
        } else {
            Ok(TaskResult::new(
                Some("Waiting for human input".to_string()),
                NextAction::WaitForInput,
            ))
        }
    }
}

/// Custom/generic task — placeholder.
struct CustomTask {
    name: String,
    node_type: String,
}

#[async_trait]
impl Task for CustomTask {
    fn id(&self) -> &str {
        &self.name
    }

    async fn run(&self, context: Context) -> Result<TaskResult> {
        context
            .set(
                format!("{}_type", self.name),
                self.node_type.clone(),
            )
            .await;
        context.set(format!("{}_executed", self.name), true).await;

        Ok(TaskResult::new(
            Some(format!("Custom task '{}' ({}) executed", self.name, self.node_type)),
            NextAction::Continue,
        ))
    }
}

/// Convert a node definition to a Task based on its type.
fn node_to_task(node: &NodeDef) -> Arc<dyn Task> {
    match node.node_type.as_str() {
        "llm" => Arc::new(LlmTask {
            name: node.name.clone(),
            config: node.config.clone(),
        }),
        "tool" => Arc::new(ToolCallTask {
            name: node.name.clone(),
            config: node.config.clone(),
        }),
        "retriever" => Arc::new(RetrieverTask {
            name: node.name.clone(),
            config: node.config.clone(),
        }),
        "human" => Arc::new(HumanInputTask {
            name: node.name.clone(),
        }),
        custom_type => Arc::new(CustomTask {
            name: node.name.clone(),
            node_type: custom_type.to_string(),
        }),
    }
}

/// Import a LangGraph workflow definition into a graph-flow Graph.
///
/// Converts nodes to Tasks and edges to graph-flow edges (including
/// conditional edges when a `condition_key` is specified).
pub fn import_langgraph_workflow(definition: &LangGraphDef) -> Result<Arc<Graph>> {
    let mut builder = GraphBuilder::new(&definition.name);

    // Add all nodes as tasks
    for node in &definition.nodes {
        let task = node_to_task(node);
        builder = builder.add_task(task);
    }

    // Set entry point if specified
    if let Some(ref entry) = definition.entry_point {
        builder = builder.set_start_task(entry);
    }

    // Wire edges
    for edge in &definition.edges {
        if let Some(ref condition_key) = edge.condition_key {
            // Conditional edge: need a "no" target
            // For now, if there's no explicit "no" target, we skip the conditional
            // and just add a regular edge
            let key = condition_key.clone();
            let to = edge.to.clone();

            // Look for another edge from the same source with a different target
            let alt_target = definition
                .edges
                .iter()
                .find(|e| e.from == edge.from && e.to != edge.to)
                .map(|e| e.to.clone());

            if let Some(no_target) = alt_target {
                builder = builder.add_conditional_edge(
                    &edge.from,
                    move |ctx| ctx.get_sync::<bool>(&key).unwrap_or(false),
                    &to,
                    &no_target,
                );
            } else {
                builder = builder.add_edge(&edge.from, &edge.to);
            }
        } else {
            builder = builder.add_edge(&edge.from, &edge.to);
        }
    }

    Ok(Arc::new(builder.build()))
}

/// Import a LangGraph workflow from a JSON string.
pub fn import_langgraph_json(json: &str) -> Result<Arc<Graph>> {
    let def: LangGraphDef = serde_json::from_str(json).map_err(|e| {
        GraphError::TaskExecutionFailed(format!("Failed to parse LangGraph JSON: {}", e))
    })?;
    import_langgraph_workflow(&def)
}

/// Import a LangGraph workflow from a YAML string.
pub fn import_langgraph_yaml(yaml: &str) -> Result<Arc<Graph>> {
    let def: LangGraphDef = serde_yaml::from_str(yaml).map_err(|e| {
        GraphError::TaskExecutionFailed(format!("Failed to parse LangGraph YAML: {}", e))
    })?;
    import_langgraph_workflow(&def)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::Session;

    #[test]
    fn test_import_simple_workflow() {
        let def = LangGraphDef {
            name: "test".to_string(),
            nodes: vec![
                NodeDef {
                    name: "start".to_string(),
                    node_type: "llm".to_string(),
                    config: serde_json::json!({"model": "gpt-4"}),
                },
                NodeDef {
                    name: "end_node".to_string(),
                    node_type: "custom".to_string(),
                    config: serde_json::json!({}),
                },
            ],
            edges: vec![EdgeDef {
                from: "start".to_string(),
                to: "end_node".to_string(),
                condition: None,
                condition_key: None,
            }],
            entry_point: Some("start".to_string()),
        };

        let graph = import_langgraph_workflow(&def).unwrap();
        assert_eq!(graph.start_task_id(), Some("start".to_string()));
        assert!(graph.get_task("start").is_some());
        assert!(graph.get_task("end_node").is_some());
    }

    #[tokio::test]
    async fn test_execute_imported_workflow() {
        let def = LangGraphDef {
            name: "exec_test".to_string(),
            nodes: vec![
                NodeDef {
                    name: "llm_node".to_string(),
                    node_type: "llm".to_string(),
                    config: serde_json::json!({"model": "test-model"}),
                },
                NodeDef {
                    name: "tool_node".to_string(),
                    node_type: "tool".to_string(),
                    config: serde_json::json!({"tool_name": "calculator"}),
                },
            ],
            edges: vec![EdgeDef {
                from: "llm_node".to_string(),
                to: "tool_node".to_string(),
                condition: None,
                condition_key: None,
            }],
            entry_point: Some("llm_node".to_string()),
        };

        let graph = import_langgraph_workflow(&def).unwrap();

        let mut session = Session::new_from_task("s1".to_string(), "llm_node");
        session.context.set("prompt", "What is 2+2?").await;

        // Execute llm_node
        let result = graph.execute_session(&mut session).await.unwrap();
        assert!(result.response.is_some());

        // Verify LLM metadata was stored
        let model: String = session
            .context
            .get("llm_node_model")
            .await
            .unwrap();
        assert_eq!(model, "test-model");

        // Execute tool_node
        let result = graph.execute_session(&mut session).await.unwrap();
        assert!(result.response.is_some());
    }

    #[test]
    fn test_import_from_json() {
        let json = r#"{
            "name": "json_test",
            "nodes": [
                {"name": "a", "node_type": "custom", "config": {}},
                {"name": "b", "node_type": "custom", "config": {}}
            ],
            "edges": [
                {"from": "a", "to": "b"}
            ],
            "entry_point": "a"
        }"#;

        let graph = import_langgraph_json(json).unwrap();
        assert!(graph.get_task("a").is_some());
        assert!(graph.get_task("b").is_some());
    }

    #[test]
    fn test_import_from_yaml() {
        let yaml = r#"
name: yaml_test
nodes:
  - name: retriever
    node_type: retriever
    config:
      index: my_index
  - name: llm
    node_type: llm
    config:
      model: claude-3
edges:
  - from: retriever
    to: llm
entry_point: retriever
"#;

        let graph = import_langgraph_yaml(yaml).unwrap();
        assert!(graph.get_task("retriever").is_some());
        assert!(graph.get_task("llm").is_some());
    }

    #[tokio::test]
    async fn test_human_in_the_loop_import() {
        let def = LangGraphDef {
            name: "human_test".to_string(),
            nodes: vec![
                NodeDef {
                    name: "ask".to_string(),
                    node_type: "human".to_string(),
                    config: serde_json::json!({}),
                },
            ],
            edges: vec![],
            entry_point: Some("ask".to_string()),
        };

        let graph = import_langgraph_workflow(&def).unwrap();

        let mut session = Session::new_from_task("s1".to_string(), "ask");

        // Without input, should wait
        let result = graph.execute_session(&mut session).await.unwrap();
        assert!(matches!(
            result.status,
            crate::graph::ExecutionStatus::WaitingForInput
        ));

        // With input, should continue
        session.context.set("human_input", "my answer").await;
        let result = graph.execute_session(&mut session).await.unwrap();
        assert!(!matches!(
            result.status,
            crate::graph::ExecutionStatus::WaitingForInput
        ));
    }
}
