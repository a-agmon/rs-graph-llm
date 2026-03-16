//! MCP (Model Context Protocol) tool integration for graph-flow.
//!
//! This module provides a [`McpToolTask`] that calls external tools via the
//! MCP protocol. This maps to LangGraph's `ToolNode` concept.
//!
//! # Overview
//!
//! MCP servers expose tools that can be called via HTTP/SSE. The `McpToolTask`
//! wraps a single tool call as a graph-flow Task, reading input from Context
//! and storing the result back.
//!
//! # Examples
//!
//! ```rust
//! use graph_flow::mcp_tool::MockMcpToolTask;
//!
//! // Create a mock MCP tool task for testing
//! let tool = MockMcpToolTask::new("echo", |input| input);
//! ```

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

use crate::{
    context::Context,
    error::{GraphError, Result},
    task::{NextAction, Task, TaskResult},
};

/// Configuration for an MCP tool call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolConfig {
    /// URL of the MCP server
    pub server_url: String,
    /// Name of the tool to call
    pub tool_name: String,
    /// Context key to read input from (default: "tool_input")
    pub input_key: String,
    /// Context key to write result to (default: "tool_result")
    pub output_key: String,
    /// Optional static parameters to include in every call
    pub static_params: HashMap<String, serde_json::Value>,
    /// Timeout in seconds (default: 30)
    pub timeout_secs: u64,
}

impl McpToolConfig {
    pub fn new(server_url: impl Into<String>, tool_name: impl Into<String>) -> Self {
        Self {
            server_url: server_url.into(),
            tool_name: tool_name.into(),
            input_key: "tool_input".to_string(),
            output_key: "tool_result".to_string(),
            static_params: HashMap::new(),
            timeout_secs: 30,
        }
    }
}

/// A Task that calls an external tool via the MCP protocol.
///
/// The task reads input from the Context, calls the MCP server, and
/// stores the result back in the Context.
///
/// # Context Keys
///
/// - **Input**: Reads from `config.input_key` (default: `"tool_input"`)
/// - **Output**: Writes to `config.output_key` (default: `"tool_result"`)
///
/// # Error Handling
///
/// If the MCP server is unreachable or returns an error, the task
/// returns a `GraphError::TaskExecutionFailed` with details.
#[cfg(feature = "mcp")]
pub struct McpToolTask {
    name: String,
    config: McpToolConfig,
}

#[cfg(feature = "mcp")]
impl McpToolTask {
    /// Create a new MCP tool task with default configuration.
    pub fn new(
        name: impl Into<String>,
        server_url: impl Into<String>,
        tool_name: impl Into<String>,
    ) -> Arc<Self> {
        Arc::new(Self {
            name: name.into(),
            config: McpToolConfig::new(server_url, tool_name),
        })
    }

    /// Create a new MCP tool task with full configuration.
    pub fn with_config(name: impl Into<String>, config: McpToolConfig) -> Arc<Self> {
        Arc::new(Self {
            name: name.into(),
            config,
        })
    }

    /// Set custom input and output context keys.
    pub fn with_keys(
        name: impl Into<String>,
        server_url: impl Into<String>,
        tool_name: impl Into<String>,
        input_key: impl Into<String>,
        output_key: impl Into<String>,
    ) -> Arc<Self> {
        let mut config = McpToolConfig::new(server_url, tool_name);
        config.input_key = input_key.into();
        config.output_key = output_key.into();
        Arc::new(Self {
            name: name.into(),
            config,
        })
    }

    /// Build the MCP request payload.
    fn build_request(&self, input: serde_json::Value) -> serde_json::Value {
        let mut params = self.config.static_params.clone();

        // Merge input into params
        if let serde_json::Value::Object(map) = input {
            for (k, v) in map {
                params.insert(k, v);
            }
        } else {
            params.insert("input".to_string(), input);
        }

        serde_json::json!({
            "jsonrpc": "2.0",
            "method": "tools/call",
            "params": {
                "name": self.config.tool_name,
                "arguments": params,
            },
            "id": 1
        })
    }
}

#[cfg(feature = "mcp")]
#[async_trait]
impl Task for McpToolTask {
    fn id(&self) -> &str {
        &self.name
    }

    async fn run(&self, context: Context) -> Result<TaskResult> {
        // Read input from context
        let input: serde_json::Value = context
            .get(&self.config.input_key)
            .await
            .unwrap_or(serde_json::Value::Null);

        // Build request
        let request_body = self.build_request(input);

        // Call MCP server
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(self.config.timeout_secs))
            .build()
            .map_err(|e| {
                GraphError::TaskExecutionFailed(format!("Failed to create HTTP client: {}", e))
            })?;

        let response = client
            .post(&format!("{}/mcp", self.config.server_url))
            .json(&request_body)
            .send()
            .await
            .map_err(|e| {
                GraphError::TaskExecutionFailed(format!(
                    "MCP tool '{}' call failed: {}",
                    self.config.tool_name, e
                ))
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(GraphError::TaskExecutionFailed(format!(
                "MCP tool '{}' returned error {}: {}",
                self.config.tool_name, status, body
            )));
        }

        let result: serde_json::Value = response.json().await.map_err(|e| {
            GraphError::TaskExecutionFailed(format!(
                "MCP tool '{}' response parse error: {}",
                self.config.tool_name, e
            ))
        })?;

        // Extract result from JSON-RPC response
        let tool_result = result
            .get("result")
            .cloned()
            .unwrap_or(result.clone());

        // Store result in context
        context.set(&self.config.output_key, tool_result.clone()).await;

        Ok(TaskResult::new(
            Some(tool_result.to_string()),
            NextAction::Continue,
        ))
    }
}

/// A mock MCP tool task for testing without a real MCP server.
///
/// The handler function receives the input and returns a result.
pub struct MockMcpToolTask {
    name: String,
    input_key: String,
    output_key: String,
    handler: Arc<dyn Fn(serde_json::Value) -> serde_json::Value + Send + Sync>,
}

impl MockMcpToolTask {
    /// Create a mock MCP tool task with a custom handler.
    pub fn new<F>(
        name: impl Into<String>,
        handler: F,
    ) -> Arc<Self>
    where
        F: Fn(serde_json::Value) -> serde_json::Value + Send + Sync + 'static,
    {
        Arc::new(Self {
            name: name.into(),
            input_key: "tool_input".to_string(),
            output_key: "tool_result".to_string(),
            handler: Arc::new(handler),
        })
    }

    /// Create a mock MCP tool task with custom keys.
    pub fn with_keys<F>(
        name: impl Into<String>,
        input_key: impl Into<String>,
        output_key: impl Into<String>,
        handler: F,
    ) -> Arc<Self>
    where
        F: Fn(serde_json::Value) -> serde_json::Value + Send + Sync + 'static,
    {
        Arc::new(Self {
            name: name.into(),
            input_key: input_key.into(),
            output_key: output_key.into(),
            handler: Arc::new(handler),
        })
    }
}

#[async_trait]
impl Task for MockMcpToolTask {
    fn id(&self) -> &str {
        &self.name
    }

    async fn run(&self, context: Context) -> Result<TaskResult> {
        let input: serde_json::Value = context
            .get(&self.input_key)
            .await
            .unwrap_or(serde_json::Value::Null);

        let result = (self.handler)(input);
        context.set(&self.output_key, result.clone()).await;

        Ok(TaskResult::new(
            Some(result.to_string()),
            NextAction::Continue,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_mcp_tool() {
        let tool = MockMcpToolTask::new("search", |input| {
            let query = input.as_str().unwrap_or("no query");
            serde_json::json!({
                "results": [
                    {"title": format!("Result for: {}", query), "url": "https://example.com"}
                ]
            })
        });

        let ctx = Context::new();
        ctx.set("tool_input", "rust programming").await;

        let result = tool.run(ctx.clone()).await.unwrap();
        assert_eq!(result.next_action, NextAction::Continue);

        let tool_result: serde_json::Value = ctx.get("tool_result").await.unwrap();
        assert!(tool_result["results"].is_array());
    }

    #[tokio::test]
    async fn test_mock_mcp_custom_keys() {
        let tool = MockMcpToolTask::with_keys("echo", "my_input", "my_output", |input| input);

        let ctx = Context::new();
        ctx.set("my_input", serde_json::json!({"hello": "world"})).await;

        let _ = tool.run(ctx.clone()).await.unwrap();

        let output: serde_json::Value = ctx.get("my_output").await.unwrap();
        assert_eq!(output, serde_json::json!({"hello": "world"}));
    }

    #[test]
    fn test_build_request() {
        let tool = McpToolTask::new("test", "https://example.com", "search");

        let request = tool.build_request(serde_json::json!({"query": "test"}));
        assert_eq!(request["method"], "tools/call");
        assert_eq!(request["params"]["name"], "search");
        assert_eq!(request["params"]["arguments"]["query"], "test");
    }

    #[test]
    fn test_mcp_config() {
        let config = McpToolConfig::new("https://example.com", "search");
        assert_eq!(config.server_url, "https://example.com");
        assert_eq!(config.tool_name, "search");
        assert_eq!(config.input_key, "tool_input");
        assert_eq!(config.output_key, "tool_result");
        assert_eq!(config.timeout_secs, 30);
    }
}
