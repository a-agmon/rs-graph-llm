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
    /// ID of the task that generated this result
    pub task_id: String,
    /// Optional status message that describes the current state of the task
    pub status_message: Option<String>,
}

impl TaskResult {
    /// Create a new TaskResult with the given response and next action
    /// The task_id will be set automatically by the graph execution engine
    pub fn new(response: Option<String>, next_action: NextAction) -> Self {
        Self {
            response,
            next_action,
            task_id: String::new(),
            status_message: None,
        }
    }

    /// Create a new TaskResult with response, next action, and status message
    pub fn new_with_status(response: Option<String>, next_action: NextAction, status_message: Option<String>) -> Self {
        Self {
            response,
            next_action,
            task_id: String::new(),
            status_message,
        }
    }

    pub fn move_to_next() -> Self {
        Self {
            response: None,
            next_action: NextAction::Continue,
            task_id: String::new(),
            status_message: None,
        }
    }

    pub fn move_to_next_direct() -> Self {
        Self {
            response: None,
            next_action: NextAction::ContinueAndExecute,
            task_id: String::new(),
            status_message: None,
        }
    }
}

/// Defines what should happen after a task completes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NextAction {
    /// Continue to the next task in the default path (wait for user input)
    Continue,
    /// Continue to the next task and execute it immediately (old recursive behavior)
    ContinueAndExecute,
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
