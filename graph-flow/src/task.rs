use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::{context::Context, error::Result};

/// Result of a task execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskResult {
    /// Response to send to the user
    pub response: Option<String>,
    /// Next action to take
    pub next_action: NextAction,
}

/// Defines what should happen after a task completes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NextAction {
    /// Continue to the next task in the default path
    Continue,
    /// Go to a specific task by ID
    GoTo(String),
    /// Go back to the previous task
    GoBack,
    /// End the graph execution
    End,
    /// Wait for user input before continuing
    WaitForInput,
}

/// Core trait that all tasks must implement
#[async_trait]
pub trait Task: Send + Sync {
    /// Unique identifier for this task
    fn id(&self) -> &str;
    
    /// Execute the task with the given context
    async fn run(&self, context: Context) -> Result<TaskResult>;
} 