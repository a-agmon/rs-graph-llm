//! Runtime configuration for graph execution.
//!
//! Provides `RunConfig` for passing tags, metadata, timeouts, and breakpoint
//! configuration to graph execution. Maps to LangGraph's `RunnableConfig`.
//!
//! # Examples
//!
//! ```rust
//! use graph_flow::run_config::{RunConfig, BreakpointConfig};
//! use std::time::Duration;
//!
//! let config = RunConfig::new()
//!     .with_tag("production")
//!     .with_timeout(Duration::from_secs(60))
//!     .with_recursion_limit(50)
//!     .with_interrupt_before("dangerous_task");
//! ```

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::time::Duration;

/// Runtime configuration passed to graph execution.
///
/// Maps to LangGraph's `RunnableConfig`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunConfig {
    /// Tags for categorizing/filtering runs.
    #[serde(default)]
    pub tags: Vec<String>,
    /// Arbitrary metadata key-value pairs.
    #[serde(default)]
    pub metadata: HashMap<String, serde_json::Value>,
    /// Timeout for the entire execution run.
    #[serde(default, with = "optional_duration_serde")]
    pub timeout: Option<Duration>,
    /// Maximum number of recursive task executions before stopping.
    #[serde(default = "default_recursion_limit")]
    pub recursion_limit: usize,
    /// Breakpoint configuration for dynamic interrupts.
    #[serde(default)]
    pub breakpoints: BreakpointConfig,
}

fn default_recursion_limit() -> usize {
    25
}

mod optional_duration_serde {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::time::Duration;

    pub fn serialize<S>(value: &Option<Duration>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match value {
            Some(d) => d.as_millis().serialize(serializer),
            None => serializer.serialize_none(),
        }
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<Duration>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let opt: Option<u64> = Option::deserialize(deserializer)?;
        Ok(opt.map(Duration::from_millis))
    }
}

impl RunConfig {
    pub fn new() -> Self {
        Self {
            tags: Vec::new(),
            metadata: HashMap::new(),
            timeout: None,
            recursion_limit: default_recursion_limit(),
            breakpoints: BreakpointConfig::default(),
        }
    }

    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
        self.tags.push(tag.into());
        self
    }

    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.tags.extend(tags);
        self
    }

    pub fn with_metadata(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    pub fn with_recursion_limit(mut self, limit: usize) -> Self {
        self.recursion_limit = limit;
        self
    }

    pub fn with_breakpoints(mut self, breakpoints: BreakpointConfig) -> Self {
        self.breakpoints = breakpoints;
        self
    }

    pub fn with_interrupt_before(mut self, task_id: impl Into<String>) -> Self {
        self.breakpoints.interrupt_before.insert(task_id.into());
        self
    }

    pub fn with_interrupt_after(mut self, task_id: impl Into<String>) -> Self {
        self.breakpoints.interrupt_after.insert(task_id.into());
        self
    }
}

impl Default for RunConfig {
    fn default() -> Self {
        Self::new()
    }
}

/// Dynamic breakpoint configuration.
///
/// Allows specifying which tasks should trigger an interrupt
/// before or after execution, without modifying the task code.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BreakpointConfig {
    /// Task IDs to interrupt before executing.
    #[serde(default)]
    pub interrupt_before: HashSet<String>,
    /// Task IDs to interrupt after executing.
    #[serde(default)]
    pub interrupt_after: HashSet<String>,
}

impl BreakpointConfig {
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if execution should pause before the given task.
    pub fn should_interrupt_before(&self, task_id: &str) -> bool {
        self.interrupt_before.contains(task_id)
    }

    /// Check if execution should pause after the given task.
    pub fn should_interrupt_after(&self, task_id: &str) -> bool {
        self.interrupt_after.contains(task_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_run_config_builder() {
        let config = RunConfig::new()
            .with_tag("test")
            .with_tag("production")
            .with_metadata("run_id", serde_json::json!("abc-123"))
            .with_timeout(Duration::from_secs(120))
            .with_recursion_limit(50);

        assert_eq!(config.tags, vec!["test", "production"]);
        assert_eq!(config.metadata["run_id"], "abc-123");
        assert_eq!(config.timeout, Some(Duration::from_secs(120)));
        assert_eq!(config.recursion_limit, 50);
    }

    #[test]
    fn test_breakpoint_config() {
        let config = RunConfig::new()
            .with_interrupt_before("task_a")
            .with_interrupt_after("task_b");

        assert!(config.breakpoints.should_interrupt_before("task_a"));
        assert!(!config.breakpoints.should_interrupt_before("task_b"));
        assert!(config.breakpoints.should_interrupt_after("task_b"));
        assert!(!config.breakpoints.should_interrupt_after("task_a"));
    }

    #[test]
    fn test_default_recursion_limit() {
        let config = RunConfig::new();
        assert_eq!(config.recursion_limit, 25);
    }

    #[test]
    fn test_serialization() {
        let config = RunConfig::new()
            .with_tag("prod")
            .with_recursion_limit(10);

        let json = serde_json::to_string(&config).unwrap();
        let deserialized: RunConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.tags, vec!["prod"]);
        assert_eq!(deserialized.recursion_limit, 10);
    }
}
