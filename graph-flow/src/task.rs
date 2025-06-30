//! Task definition and execution control.
//!
//! This module contains the core [`Task`] trait and related types for defining
//! workflow steps and controlling execution flow.
//!
//! # Examples
//!
//! ## Basic Task Implementation
//!
//! ```rust
//! use graph_flow::{Task, TaskResult, NextAction, Context};
//! use async_trait::async_trait;
//!
//! struct HelloTask;
//!
//! #[async_trait]
//! impl Task for HelloTask {
//!     fn id(&self) -> &str {
//!         "hello_task"
//!     }
//!
//!     async fn run(&self, context: Context) -> graph_flow::Result<TaskResult> {
//!         let name: String = context.get("name").await.unwrap_or("World".to_string());
//!         let greeting = format!("Hello, {}!", name);
//!         
//!         // Store result for next task
//!         context.set("greeting", greeting.clone()).await;
//!         
//!         Ok(TaskResult::new(Some(greeting), NextAction::Continue))
//!     }
//! }
//! ```
//!
//! ## Task with Different Control Flow
//!
//! ```rust
//! # use graph_flow::{Task, TaskResult, NextAction, Context};
//! # use async_trait::async_trait;
//! struct ConditionalTask;
//!
//! #[async_trait]
//! impl Task for ConditionalTask {
//!     fn id(&self) -> &str {
//!         "conditional_task"
//!     }
//!
//!     async fn run(&self, context: Context) -> graph_flow::Result<TaskResult> {
//!         let user_input: Option<String> = context.get("user_input").await;
//!         
//!         match user_input {
//!             Some(input) if !input.is_empty() => {
//!                 // Process input and continue automatically
//!                 context.set("processed", input.to_uppercase()).await;
//!                 Ok(TaskResult::new(
//!                     Some("Input processed".to_string()),
//!                     NextAction::ContinueAndExecute
//!                 ))
//!             }
//!             _ => {
//!                 // Wait for user input
//!                 Ok(TaskResult::new(
//!                     Some("Please provide input".to_string()),
//!                     NextAction::WaitForInput
//!                 ))
//!             }
//!         }
//!     }
//! }
//! ```

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::{context::Context, error::Result};

/// Result of a task execution.
///
/// Contains the response to send to the user and the next action to take.
/// The `task_id` field is automatically set by the graph execution engine.
///
/// # Examples
///
/// ```rust
/// use graph_flow::{TaskResult, NextAction};
///
/// // Basic task result
/// let result = TaskResult::new(
///     Some("Task completed successfully".to_string()),
///     NextAction::Continue
/// );
///
/// // Task result with status message
/// let result = TaskResult::new_with_status(
///     Some("Data validated".to_string()),
///     NextAction::Continue,
///     Some("All validation checks passed".to_string())
/// );
///
/// // Convenience methods
/// let result = TaskResult::move_to_next();        // Continue to next task
/// let result = TaskResult::move_to_next_direct(); // Continue and execute immediately
/// ```
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
    /// Create a new TaskResult with the given response and next action.
    ///
    /// The task_id will be set automatically by the graph execution engine.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use graph_flow::{TaskResult, NextAction};
    ///
    /// let result = TaskResult::new(
    ///     Some("Hello, World!".to_string()),
    ///     NextAction::Continue
    /// );
    /// ```
    pub fn new(response: Option<String>, next_action: NextAction) -> Self {
        Self {
            response,
            next_action,
            task_id: String::new(),
            status_message: None,
        }
    }

    /// Create a new TaskResult with response, next action, and status message.
    ///
    /// The status message is used to describe the current state of the task.
    /// It's only persisted in the context but not returned to the user.
    /// Specifically aimed at debugging and logging.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use graph_flow::{TaskResult, NextAction};
    ///
    /// let result = TaskResult::new_with_status(
    ///     Some("Data processed".to_string()),
    ///     NextAction::Continue,
    ///     Some("Processing completed with 95% confidence".to_string())
    /// );
    /// ```
    pub fn new_with_status(
        response: Option<String>,
        next_action: NextAction,
        status_message: Option<String>,
    ) -> Self {
        Self {
            response,
            next_action,
            task_id: String::new(),
            status_message,
        }
    }

    /// Create a TaskResult that moves to the next task (step-by-step execution).
    ///
    /// This is a convenience method equivalent to:
    /// ```rust
    /// # use graph_flow::{TaskResult, NextAction};
    /// TaskResult::new(None, NextAction::Continue);
    /// ```
    pub fn move_to_next() -> Self {
        Self {
            response: None,
            next_action: NextAction::Continue,
            task_id: String::new(),
            status_message: None,
        }
    }

    /// Create a TaskResult that moves to the next task and executes it immediately.
    ///
    /// This is a convenience method equivalent to:
    /// ```rust
    /// # use graph_flow::{TaskResult, NextAction};
    /// TaskResult::new(None, NextAction::ContinueAndExecute);
    /// ```
    pub fn move_to_next_direct() -> Self {
        Self {
            response: None,
            next_action: NextAction::ContinueAndExecute,
            task_id: String::new(),
            status_message: None,
        }
    }
}

/// Defines what should happen after a task completes.
///
/// This enum controls the flow of execution in your workflow graph.
/// Different variants provide different execution behaviors.
///
/// # Examples
///
/// ```rust
/// use graph_flow::{NextAction, TaskResult};
///
/// // Step-by-step execution (pause after this task)
/// let result = TaskResult::new(
///     Some("Step 1 complete".to_string()),
///     NextAction::Continue
/// );
///
/// // Continuous execution (run next task immediately)
/// let result = TaskResult::new(
///     Some("Processing...".to_string()),
///     NextAction::ContinueAndExecute
/// );
///
/// // Wait for user input
/// let result = TaskResult::new(
///     Some("Please provide more information".to_string()),
///     NextAction::WaitForInput
/// );
///
/// // Jump to specific task
/// let result = TaskResult::new(
///     Some("Redirecting to error handler".to_string()),
///     NextAction::GoTo("error_handler".to_string())
/// );
///
/// // End the workflow
/// let result = TaskResult::new(
///     Some("Workflow completed!".to_string()),
///     NextAction::End
/// );
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum NextAction {
    /// Continue to the next task in the default path (step-by-step execution).
    ///
    /// The workflow will pause after this task and wait for the next
    /// execution call. This gives you control over when the next task runs.
    ///
    /// Best for: Interactive applications, web services, debugging
    Continue,

    /// Continue to the next task and execute it immediately (continuous execution).
    ///
    /// The workflow will automatically proceed to the next task without
    /// pausing. This creates a recursive execution until the workflow
    /// reaches `End`, `WaitForInput`, or an error.
    ///
    /// Best for: Batch processing, automated workflows
    ContinueAndExecute,

    /// Go to a specific task by ID.
    ///
    /// Jump directly to the specified task, skipping the normal edge-based
    /// flow. Useful for error handling, loops, or dynamic routing.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use graph_flow::{NextAction, TaskResult};
    /// // Jump to error handler
    /// let result = TaskResult::new(
    ///     Some("Error detected, routing to handler".to_string()),
    ///     NextAction::GoTo("error_handler".to_string())
    /// );
    ///
    /// // Create a retry loop
    /// let result = TaskResult::new(
    ///     Some("Retrying...".to_string()),
    ///     NextAction::GoTo("validation_task".to_string())
    /// );
    /// ```
    GoTo(String),

    /// Go back to the previous task.
    ///
    /// Note: This currently stays at the current task. Full back navigation
    /// logic may be implemented in future versions.
    GoBack,

    /// End the graph execution.
    ///
    /// Terminates the workflow completely. No further tasks will be executed.
    End,

    /// Wait for user input before continuing.
    ///
    /// Pauses the workflow and waits for external input. The workflow
    /// will stay at the current task until new data is provided and
    /// execution is resumed.
    ///
    /// Best for: Human-in-the-loop workflows, interactive applications
    WaitForInput,
}

/// Core trait that all tasks must implement.
///
/// Tasks are the building blocks of your workflow. Each task represents
/// a unit of work that can access shared context and control the flow
/// of execution.
///
/// # Examples
///
/// ## Basic Task
///
/// ```rust
/// use graph_flow::{Task, TaskResult, NextAction, Context};
/// use async_trait::async_trait;
///
/// struct GreetingTask;
///
/// #[async_trait]
/// impl Task for GreetingTask {
///     fn id(&self) -> &str {
///         "greeting"
///     }
///
///     async fn run(&self, context: Context) -> graph_flow::Result<TaskResult> {
///         let name: String = context.get("name").await.unwrap_or("World".to_string());
///         let greeting = format!("Hello, {}!", name);
///         
///         Ok(TaskResult::new(Some(greeting), NextAction::Continue))
///     }
/// }
/// ```
///
/// ## Task with Default ID
///
/// ```rust
/// # use graph_flow::{Task, TaskResult, NextAction, Context};
/// # use async_trait::async_trait;
/// struct DefaultIdTask;
///
/// #[async_trait]
/// impl Task for DefaultIdTask {
///     // id() is automatically implemented using the type name
///     
///     async fn run(&self, context: Context) -> graph_flow::Result<TaskResult> {
///         Ok(TaskResult::new(None, NextAction::End))
///     }
/// }
/// ```
///
/// ## Complex Task with Error Handling
///
/// ```rust
/// # use graph_flow::{Task, TaskResult, NextAction, Context, GraphError};
/// # use async_trait::async_trait;
/// struct ValidationTask {
///     max_retries: usize,
/// }
///
/// #[async_trait]
/// impl Task for ValidationTask {
///     fn id(&self) -> &str {
///         "validator"
///     }
///
///     async fn run(&self, context: Context) -> graph_flow::Result<TaskResult> {
///         let data: Option<String> = context.get("data").await;
///         let retry_count: usize = context.get("retry_count").await.unwrap_or(0);
///         
///         match data {
///             Some(data) if self.validate(&data) => {
///                 context.set("retry_count", 0).await; // Reset counter
///                 Ok(TaskResult::new(
///                     Some("Validation passed".to_string()),
///                     NextAction::Continue
///                 ))
///             }
///             Some(_) if retry_count < self.max_retries => {
///                 context.set("retry_count", retry_count + 1).await;
///                 Ok(TaskResult::new(
///                     Some("Validation failed, retrying...".to_string()),
///                     NextAction::GoTo("data_input".to_string())
///                 ))
///             }
///             _ => {
///                 Err(GraphError::TaskExecutionFailed(
///                     "Validation failed after max retries".to_string()
///                 ))
///             }
///         }
///     }
/// }
///
/// impl ValidationTask {
///     fn validate(&self, data: &str) -> bool {
///         !data.is_empty() && data.len() > 5
///     }
/// }
/// ```
#[async_trait]
pub trait Task: Send + Sync {
    /// Unique identifier for this task.
    ///
    /// By default, this returns the type name of the implementing struct.
    /// Override this method if you need a custom identifier.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use graph_flow::Task;
    /// # use async_trait::async_trait;
    /// # use graph_flow::{TaskResult, NextAction, Context};
    /// // Using default implementation (type name)
    /// struct MyTask;
    ///
    /// #[async_trait]
    /// impl Task for MyTask {
    ///     // id() will return "my_module::MyTask"
    ///     async fn run(&self, _context: Context) -> graph_flow::Result<TaskResult> {
    ///         Ok(TaskResult::new(None, NextAction::End))
    ///     }
    /// }
    ///
    /// // Using custom ID
    /// struct CustomTask;
    ///
    /// #[async_trait]
    /// impl Task for CustomTask {
    ///     fn id(&self) -> &str {
    ///         "custom_task_id"
    ///     }
    ///
    ///     async fn run(&self, _context: Context) -> graph_flow::Result<TaskResult> {
    ///         Ok(TaskResult::new(None, NextAction::End))
    ///     }
    /// }
    /// ```
    fn id(&self) -> &str {
        std::any::type_name::<Self>()
    }

    /// Execute the task with the given context.
    ///
    /// This is where you implement your task's logic. You have access to
    /// the shared context for reading input data and storing results.
    ///
    /// # Parameters
    ///
    /// * `context` - Shared context containing workflow state and data
    ///
    /// # Returns
    ///
    /// Returns a `Result<TaskResult>` where:
    /// - `Ok(TaskResult)` indicates successful execution
    /// - `Err(GraphError)` indicates an error that should stop the workflow
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use graph_flow::{Task, TaskResult, NextAction, Context};
    /// # use async_trait::async_trait;
    /// struct DataProcessor;
    ///
    /// #[async_trait]
    /// impl Task for DataProcessor {
    ///     fn id(&self) -> &str {
    ///         "data_processor"
    ///     }
    ///
    ///     async fn run(&self, context: Context) -> graph_flow::Result<TaskResult> {
    ///         // Read input from context
    ///         let input: String = context.get("raw_data").await
    ///             .unwrap_or_default();
    ///         
    ///         // Process the data
    ///         let processed = self.process_data(&input).await?;
    ///         
    ///         // Store result for next task
    ///         context.set("processed_data", processed.clone()).await;
    ///         
    ///         // Return result with next action
    ///         Ok(TaskResult::new(
    ///             Some(format!("Processed {} bytes", processed.len())),
    ///             NextAction::Continue
    ///         ))
    ///     }
    /// }
    ///
    /// impl DataProcessor {
    ///     async fn process_data(&self, input: &str) -> graph_flow::Result<String> {
    ///         // Your processing logic here
    ///         Ok(input.to_uppercase())
    ///     }
    /// }
    /// ```
    async fn run(&self, context: Context) -> Result<TaskResult>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;

    struct TestTaskWithDefaultId;

    #[async_trait]
    impl Task for TestTaskWithDefaultId {
        async fn run(&self, _context: Context) -> Result<TaskResult> {
            Ok(TaskResult::new(None, NextAction::End))
        }
    }

    struct TestTaskWithCustomId;

    #[async_trait]
    impl Task for TestTaskWithCustomId {
        fn id(&self) -> &str {
            "custom_task_id"
        }

        async fn run(&self, _context: Context) -> Result<TaskResult> {
            Ok(TaskResult::new(None, NextAction::End))
        }
    }

    #[test]
    fn test_default_id_implementation() {
        let task = TestTaskWithDefaultId;
        assert_eq!(task.id(), "graph_flow::task::tests::TestTaskWithDefaultId");
    }

    #[test]
    fn test_custom_id_override() {
        let task = TestTaskWithCustomId;
        assert_eq!(task.id(), "custom_task_id");
    }
}
