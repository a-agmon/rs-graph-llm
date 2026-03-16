//! LangGraph API compatibility types.
//!
//! This module provides types that map directly to LangGraph's Python API,
//! making it easier for users familiar with LangGraph to use graph-flow.
//!
//! # Mapping
//!
//! | LangGraph Python | graph-flow | This module |
//! |---|---|---|
//! | `START` | first task added | `START` constant |
//! | `END` | `NextAction::End` | `END` constant |
//! | `StateGraph` | `GraphBuilder` | `StateGraph<S>` wrapper |
//! | `Command` | `NextAction` | `Command` enum |
//! | `StreamMode` | `StreamChunk` | `StreamMode` enum |
//! | `RoutingDecision` | edge condition | `RoutingDecision` enum |
//!
//! # Examples
//!
//! ```rust
//! use graph_flow::compat::{StateGraph, START, END};
//!
//! // LangGraph-style API
//! let mut sg = StateGraph::new("my_graph");
//! // sg.add_node("start", task);
//! // sg.add_edge(START, "start");
//! // let graph = sg.compile();
//! ```

use std::sync::Arc;

use crate::{
    context::Context,
    graph::{Graph, GraphBuilder},
    task::Task,
};

/// Special node name representing the start of the graph.
/// In LangGraph Python: `from langgraph.graph import START`
pub const START: &str = "__start__";

/// Special node name representing the end of the graph.
/// In LangGraph Python: `from langgraph.graph import END`
pub const END: &str = "__end__";

/// Routing decision returned by conditional edge functions.
/// Maps to LangGraph's path function return values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RoutingDecision {
    /// Route to a specific node by name
    GoTo(String),
    /// Route to the END node
    End,
}

impl RoutingDecision {
    pub fn goto(name: impl Into<String>) -> Self {
        Self::GoTo(name.into())
    }
}

/// Stream modes supported by the execution engine.
/// Maps to LangGraph's `stream_mode` parameter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StreamMode {
    /// Stream state values after each node
    Values,
    /// Stream only the updates (deltas) from each node
    Updates,
    /// Stream individual messages
    Messages,
    /// Stream debug information
    Debug,
    /// Custom stream mode
    Custom(String),
}

/// Command type for controlling graph execution.
/// Maps to LangGraph's `Command` class.
///
/// # Examples
///
/// ```rust
/// use graph_flow::compat::Command;
///
/// let cmd = Command::goto("next_node");
/// let cmd = Command::update(serde_json::json!({"key": "value"}));
/// let cmd = Command::resume(serde_json::json!("user_input"));
/// ```
#[derive(Debug, Clone)]
pub enum Command {
    /// Update the state
    Update(serde_json::Value),
    /// Resume from an interrupt
    Resume(serde_json::Value),
    /// Go to a specific node
    GoTo(String),
}

impl Command {
    pub fn update(value: serde_json::Value) -> Self {
        Self::Update(value)
    }

    pub fn resume(value: serde_json::Value) -> Self {
        Self::Resume(value)
    }

    pub fn goto(node: impl Into<String>) -> Self {
        Self::GoTo(node.into())
    }
}

/// LangGraph-compatible StateGraph wrapper.
///
/// Provides a familiar API for users coming from Python LangGraph.
/// Internally delegates to `GraphBuilder`.
///
/// # Examples
///
/// ```rust
/// use graph_flow::compat::StateGraph;
/// use graph_flow::{Task, TaskResult, NextAction, Context};
/// use async_trait::async_trait;
/// use std::sync::Arc;
///
/// struct MyNode;
///
/// #[async_trait]
/// impl Task for MyNode {
///     fn id(&self) -> &str { "my_node" }
///     async fn run(&self, _ctx: Context) -> graph_flow::Result<TaskResult> {
///         Ok(TaskResult::new(Some("done".to_string()), NextAction::End))
///     }
/// }
///
/// let mut sg = StateGraph::new("example");
/// sg.add_node("my_node", Arc::new(MyNode));
/// let graph = sg.compile();
/// ```
pub struct StateGraph {
    name: String,
    nodes: Vec<(String, Arc<dyn Task>)>,
    edges: Vec<(String, String)>,
    conditional_edges: Vec<(
        String,
        Box<dyn Fn(&Context) -> bool + Send + Sync>,
        String,
        String,
    )>,
    entry_point: Option<String>,
}

impl StateGraph {
    /// Create a new StateGraph (equivalent to `StateGraph(State)` in Python).
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            nodes: Vec::new(),
            edges: Vec::new(),
            conditional_edges: Vec::new(),
            entry_point: None,
        }
    }

    /// Add a node to the graph.
    /// In LangGraph Python: `graph.add_node("name", function)`
    pub fn add_node(&mut self, name: impl Into<String>, task: Arc<dyn Task>) -> &mut Self {
        self.nodes.push((name.into(), task));
        self
    }

    /// Add an edge between nodes.
    /// In LangGraph Python: `graph.add_edge("from", "to")`
    ///
    /// Special values: use `START` for entry point, `END` for termination.
    pub fn add_edge(&mut self, from: impl Into<String>, to: impl Into<String>) -> &mut Self {
        let from = from.into();
        let to = to.into();

        if from == START {
            self.entry_point = Some(to);
        } else if to != END {
            self.edges.push((from, to));
        }
        // Edges to END are implicit (no outgoing edge = end)
        self
    }

    /// Add conditional edges.
    /// In LangGraph Python: `graph.add_conditional_edges("source", path_fn, path_map)`
    pub fn add_conditional_edges<F>(
        &mut self,
        source: impl Into<String>,
        condition: F,
        if_true: impl Into<String>,
        if_false: impl Into<String>,
    ) -> &mut Self
    where
        F: Fn(&Context) -> bool + Send + Sync + 'static,
    {
        let if_true = if_true.into();
        let if_false = if_false.into();
        let source = source.into();

        // Skip edges pointing to END — they become no-op
        let yes = if if_true == END { "__noop__".to_string() } else { if_true };
        let no = if if_false == END { "__noop__".to_string() } else { if_false };

        self.conditional_edges.push((source, Box::new(condition), yes, no));
        self
    }

    /// Set the entry point explicitly.
    /// In LangGraph Python: `graph.set_entry_point("node")`
    pub fn set_entry_point(&mut self, node: impl Into<String>) -> &mut Self {
        self.entry_point = Some(node.into());
        self
    }

    /// Compile the graph.
    /// In LangGraph Python: `compiled = graph.compile()`
    pub fn compile(self) -> Graph {
        let mut builder = GraphBuilder::new(&self.name);

        for (_, task) in &self.nodes {
            builder = builder.add_task(task.clone());
        }

        if let Some(entry) = &self.entry_point {
            builder = builder.set_start_task(entry);
        }

        for (from, to) in &self.edges {
            builder = builder.add_edge(from, to);
        }

        for (source, condition, yes, no) in self.conditional_edges {
            if yes != "__noop__" && no != "__noop__" {
                builder = builder.add_conditional_edge(source, condition, yes, no);
            } else if yes != "__noop__" {
                builder = builder.add_edge(&source, &yes);
            } else if no != "__noop__" {
                builder = builder.add_edge(&source, &no);
            }
        }

        builder.build()
    }
}

/// Thread state representation compatible with LangGraph API.
/// Maps to nurokhq/langgraph-api-rust `ThreadState`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ThreadState {
    /// Current state values
    pub values: serde_json::Value,
    /// Next nodes to execute
    pub next: Vec<String>,
    /// Checkpoint metadata
    pub checkpoint: CheckpointConfig,
    /// Additional metadata
    pub metadata: serde_json::Value,
    /// Creation timestamp
    pub created_at: String,
}

/// Checkpoint configuration compatible with LangGraph API.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct CheckpointConfig {
    /// Thread ID
    pub thread_id: String,
    /// Checkpoint ID
    pub checkpoint_id: Option<String>,
    /// Checkpoint namespace
    pub checkpoint_ns: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{NextAction, TaskResult};
    use async_trait::async_trait;

    struct NodeA;
    struct NodeB;

    #[async_trait]
    impl Task for NodeA {
        fn id(&self) -> &str { "a" }
        async fn run(&self, ctx: Context) -> crate::Result<TaskResult> {
            ctx.set("visited_a", true).await;
            Ok(TaskResult::new(Some("A".to_string()), NextAction::Continue))
        }
    }

    #[async_trait]
    impl Task for NodeB {
        fn id(&self) -> &str { "b" }
        async fn run(&self, ctx: Context) -> crate::Result<TaskResult> {
            ctx.set("visited_b", true).await;
            Ok(TaskResult::new(Some("B".to_string()), NextAction::End))
        }
    }

    #[tokio::test]
    async fn test_state_graph_basic() {
        let mut sg = StateGraph::new("test");
        sg.add_node("a", Arc::new(NodeA));
        sg.add_node("b", Arc::new(NodeB));
        sg.add_edge(START, "a");
        sg.add_edge("a", "b");
        sg.add_edge("b", END);

        let graph = sg.compile();
        assert_eq!(graph.start_task_id(), Some("a".to_string()));

        let ctx = Context::new();
        let mut session = crate::Session::new_from_task("s1".to_string(), "a");
        let _ = graph.execute_session(&mut session).await.unwrap();
        let _ = graph.execute_session(&mut session).await.unwrap();

        let a: bool = session.context.get("visited_a").await.unwrap_or(false);
        let b: bool = session.context.get("visited_b").await.unwrap_or(false);
        assert!(a);
        assert!(b);
    }

    #[test]
    fn test_constants() {
        assert_eq!(START, "__start__");
        assert_eq!(END, "__end__");
    }

    #[test]
    fn test_routing_decision() {
        let rd = RoutingDecision::goto("node_a");
        assert_eq!(rd, RoutingDecision::GoTo("node_a".to_string()));

        let rd = RoutingDecision::End;
        assert_eq!(rd, RoutingDecision::End);
    }

    #[test]
    fn test_command() {
        let cmd = Command::goto("next");
        assert!(matches!(cmd, Command::GoTo(ref s) if s == "next"));

        let cmd = Command::update(serde_json::json!({"key": "val"}));
        assert!(matches!(cmd, Command::Update(_)));

        let cmd = Command::resume(serde_json::json!("input"));
        assert!(matches!(cmd, Command::Resume(_)));
    }
}
