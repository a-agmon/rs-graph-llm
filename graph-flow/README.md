# graph-flow

A high-performance, type-safe framework for building multi-agent workflow systems in Rust.

## Features

- **Type-Safe Workflows**: Compile-time guarantees for workflow correctness
- **Flexible Execution**: Step-by-step, batch, or mixed execution modes
- **Built-in Persistence**: PostgreSQL and in-memory storage backends
- **LLM Integration**: Optional integration with Rig for AI agent capabilities
- **Human-in-the-Loop**: Natural workflow interruption and resumption
- **Async/Await Native**: Built from the ground up for async Rust

## Quick Start

Add to your `Cargo.toml`:

```toml
[dependencies]
graph-flow = "0.2"

# For LLM integration
graph-flow = { version = "0.2", features = ["rig"] }
```

### Basic Example

```rust
use graph_flow::{Context, Task, TaskResult, NextAction, GraphBuilder, FlowRunner, InMemorySessionStorage, Session};
use async_trait::async_trait;
use std::sync::Arc;

// Define a simple greeting task
struct HelloTask;

#[async_trait]
impl Task for HelloTask {
    fn id(&self) -> &str {
        "hello_task"
    }

    async fn run(&self, context: Context) -> graph_flow::Result<TaskResult> {
        let name: String = context.get("name").await.unwrap_or("World".to_string());
        let greeting = format!("Hello, {}! How are you today?", name);
        
        // Store the greeting in context for other tasks
        context.set("greeting", greeting.clone()).await;
        
        Ok(TaskResult::new(Some(greeting), NextAction::Continue))
    }
}

#[tokio::main]
async fn main() -> graph_flow::Result<()> {
    // Build the graph
    let hello_task = Arc::new(HelloTask);
    let graph = Arc::new(
        GraphBuilder::new("greeting_workflow")
            .add_task(hello_task.clone())
            .build()
    );

    // Set up session storage and runner
    let session_storage = Arc::new(InMemorySessionStorage::new());
    let flow_runner = FlowRunner::new(graph.clone(), session_storage.clone());
    
    // Create a session with initial data
    let session = Session::new_from_task("user_123".to_string(), hello_task.id());
    session.context.set("name", "Alice".to_string()).await;
    session_storage.save(session).await?;
    
    // Execute the workflow
    let result = flow_runner.run("user_123").await?;
    println!("Response: {:?}", result.response);
    
    Ok(())
}
```

## Core API Reference

### Tasks - The Building Blocks

Tasks implement the core `Task` trait and define the units of work in your workflow:

#### Basic Task Implementation

```rust
use graph_flow::{Task, TaskResult, NextAction, Context};
use async_trait::async_trait;

struct DataProcessingTask {
    name: String,
}

#[async_trait]
impl Task for DataProcessingTask {
    fn id(&self) -> &str {
        &self.name
    }

    async fn run(&self, context: Context) -> graph_flow::Result<TaskResult> {
        // Get input data from context
        let input: String = context.get("user_input").await.unwrap_or_default();
        
        // Process the data
        let processed = format!("Processed: {}", input.to_uppercase());
        
        // Store result for next task
        context.set("processed_data", processed.clone()).await;
        
        // Return result with next action
        Ok(TaskResult::new(
            Some(format!("Data processed: {}", processed)),
            NextAction::Continue
        ))
    }
}
```

#### Task with Status Messages

```rust
struct ValidationTask;

#[async_trait]
impl Task for ValidationTask {
    fn id(&self) -> &str {
        "validator"
    }

    async fn run(&self, context: Context) -> graph_flow::Result<TaskResult> {
        let data: Option<String> = context.get("processed_data").await;
        
        match data {
            Some(data) if data.len() > 10 => {
                Ok(TaskResult::new_with_status(
                    Some("Data validation passed".to_string()),
                    NextAction::Continue,
                    Some("Data meets minimum length requirement".to_string())
                ))
            }
            Some(_) => {
                Ok(TaskResult::new_with_status(
                    Some("Data validation failed - too short".to_string()),
                    NextAction::WaitForInput,
                    Some("Waiting for user to provide more data".to_string())
                ))
            }
            None => {
                Ok(TaskResult::new_with_status(
                    Some("No data found to validate".to_string()),
                    NextAction::GoTo("data_input".to_string()),
                    Some("Redirecting to data input task".to_string())
                ))
            }
        }
    }
}
```

### NextAction - Controlling Flow

The `NextAction` enum controls how your workflow progresses:

```rust
// Continue to next task, but pause execution (step-by-step mode)
Ok(TaskResult::new(Some("Step completed".to_string()), NextAction::Continue))

// Continue and execute the next task immediately (continuous mode)
Ok(TaskResult::new(Some("Moving forward".to_string()), NextAction::ContinueAndExecute))

// Wait for user input before continuing
Ok(TaskResult::new(Some("Need more info".to_string()), NextAction::WaitForInput))

// Jump to a specific task
Ok(TaskResult::new(Some("Redirecting".to_string()), NextAction::GoTo("specific_task".to_string())))

// Go back to the previous task
Ok(TaskResult::new(Some("Going back".to_string()), NextAction::GoBack))

// End the workflow
Ok(TaskResult::new(Some("All done!".to_string()), NextAction::End))

// Convenience methods
TaskResult::move_to_next()        // NextAction::Continue
TaskResult::move_to_next_direct() // NextAction::ContinueAndExecute
```

### Context - State Management

The `Context` provides thread-safe state sharing across tasks:

#### Basic Context Operations

```rust
// Setting values
context.set("key", "value").await;
context.set("number", 42).await;
context.set("complex_data", MyStruct { field: "value" }).await;

// Getting values
let value: Option<String> = context.get("key").await;
let number: Option<i32> = context.get("number").await;
let complex: Option<MyStruct> = context.get("complex_data").await;

// Synchronous operations (useful in edge conditions)
context.set_sync("sync_key", "sync_value");
let sync_value: Option<String> = context.get_sync("sync_key");

// Removing values
let removed: Option<serde_json::Value> = context.remove("key").await;

// Clearing all data (preserves chat history)
context.clear().await;
```

#### Chat History Management

```rust
// Adding messages
context.add_user_message("Hello, assistant!".to_string()).await;
context.add_assistant_message("Hello! How can I help you?".to_string()).await;
context.add_system_message("System: Session started".to_string()).await;

// Getting chat history
let history = context.get_chat_history().await;
let all_messages = context.get_all_messages().await;
let last_5 = context.get_last_messages(5).await;

// Chat history info
let count = context.chat_history_len().await;
let is_empty = context.is_chat_history_empty().await;

// Clear chat history
context.clear_chat_history().await;

// Context with message limits
let context = Context::with_max_chat_messages(100);
```

#### LLM Integration (with `rig` feature)

```rust
#[cfg(feature = "rig")]
{
    // Get messages in rig format for LLM calls
    let rig_messages = context.get_rig_messages().await;
    let last_10_for_llm = context.get_last_rig_messages(10).await;
    
    // Use with rig's completion API
    // let response = agent.completion(&rig_messages).await?;
}
```

### Graph Building

Create complex workflows using the `GraphBuilder`:

#### Linear Workflow

```rust
let graph = GraphBuilder::new("linear_workflow")
    .add_task(task1.clone())
    .add_task(task2.clone())
    .add_task(task3.clone())
    .add_edge(task1.id(), task2.id())  // task1 -> task2
    .add_edge(task2.id(), task3.id())  // task2 -> task3
    .build();
```

#### Conditional Workflow

```rust
let graph = GraphBuilder::new("conditional_workflow")
    .add_task(input_task.clone())
    .add_task(process_a.clone())
    .add_task(process_b.clone())
    .add_task(final_task.clone())
    .add_conditional_edge(
        input_task.id(),
        |ctx| ctx.get_sync::<String>("user_type").unwrap_or_default() == "premium",
        process_a.id(),    // if premium user
        process_b.id(),    // if regular user
    )
    .add_edge(process_a.id(), final_task.id())
    .add_edge(process_b.id(), final_task.id())
    .build();
```

#### Complex Branching

```rust
let graph = GraphBuilder::new("complex_workflow")
    .add_task(start_task.clone())
    .add_task(validation_task.clone())
    .add_task(processing_task.clone())
    .add_task(error_handler.clone())
    .add_task(success_task.clone())
    .add_task(retry_task.clone())
    // Initial flow
    .add_edge(start_task.id(), validation_task.id())
    // Validation branches
    .add_conditional_edge(
        validation_task.id(),
        |ctx| ctx.get_sync::<bool>("is_valid").unwrap_or(false),
        processing_task.id(),  // valid -> process
        error_handler.id(),    // invalid -> error
    )
    // Processing branches
    .add_conditional_edge(
        processing_task.id(),
        |ctx| ctx.get_sync::<bool>("success").unwrap_or(false),
        success_task.id(),     // success -> done
        retry_task.id(),       // failure -> retry
    )
    // Retry logic
    .add_conditional_edge(
        retry_task.id(),
        |ctx| ctx.get_sync::<i32>("retry_count").unwrap_or(0) < 3,
        validation_task.id(),  // retry -> validate again
        error_handler.id(),    // max retries -> error
    )
    .set_start_task(start_task.id())
    .build();
```

### Execution Patterns

#### Step-by-Step Execution

Best for interactive applications where you want control between each step:

```rust
let flow_runner = FlowRunner::new(graph, session_storage);

loop {
    let result = flow_runner.run(&session_id).await?;
    
    match result.status {
        ExecutionStatus::Completed => {
            println!("Workflow completed: {:?}", result.response);
            break;
        }
        ExecutionStatus::WaitingForInput => {
            println!("Waiting for input: {:?}", result.response);
            // Get user input and update context
            // context.set("user_input", user_input).await;
            continue;
        }
        ExecutionStatus::Paused { next_task_id } => {
            println!("Paused at {}: {:?}", next_task_id, result.response);
            // Optionally do something before next step
            continue;
        }
        ExecutionStatus::Error(e) => {
            eprintln!("Error: {}", e);
            break;
        }
    }
}
```

#### Continuous Execution

For tasks that should run automatically until completion:

```rust
// Tasks use NextAction::ContinueAndExecute
struct AutoTask;

#[async_trait]
impl Task for AutoTask {
    fn id(&self) -> &str { "auto_task" }
    
    async fn run(&self, context: Context) -> graph_flow::Result<TaskResult> {
        // Do work...
        Ok(TaskResult::new(
            Some("Work done".to_string()),
            NextAction::ContinueAndExecute  // Continue automatically
        ))
    }
}

// Single call executes until completion or interruption
let result = flow_runner.run(&session_id).await?;
```

#### Mixed Execution

Combine both patterns in the same workflow:

```rust
struct InteractiveTask;

#[async_trait]
impl Task for InteractiveTask {
    fn id(&self) -> &str { "interactive" }
    
    async fn run(&self, context: Context) -> graph_flow::Result<TaskResult> {
        let needs_input: bool = context.get("needs_user_input").await.unwrap_or(false);
        
        if needs_input {
            Ok(TaskResult::new(
                Some("Please provide input".to_string()),
                NextAction::WaitForInput  // Stop and wait
            ))
        } else {
            Ok(TaskResult::new(
                Some("Processing automatically".to_string()),
                NextAction::ContinueAndExecute  // Continue automatically
            ))
        }
    }
}
```

### Storage Backends

#### In-Memory Storage (Development)

```rust
use graph_flow::InMemorySessionStorage;

let storage = Arc::new(InMemorySessionStorage::new());

// Create and save a session
let session = Session::new_from_task("session_1".to_string(), "start_task");
session.context.set("initial_data", "value").await;
storage.save(session).await?;

// Retrieve and use
let session = storage.get("session_1").await?.unwrap();
let data: String = session.context.get("initial_data").await.unwrap();
```

#### PostgreSQL Storage (Production)

```rust
use graph_flow::PostgresSessionStorage;

// Connect to database
let storage = Arc::new(
    PostgresSessionStorage::connect(&database_url).await?
);

// Works the same as in-memory
let session = Session::new_from_task("session_1".to_string(), "start_task");
storage.save(session).await?;
```

### Advanced Examples

#### Multi-Agent Conversation System

```rust
use graph_flow::*;

struct AgentTask {
    agent_name: String,
    system_prompt: String,
}

#[async_trait]
impl Task for AgentTask {
    fn id(&self) -> &str {
        &self.agent_name
    }

    async fn run(&self, context: Context) -> Result<TaskResult> {
        // Get conversation history
        let messages = context.get_all_messages().await;
        
        // Add system context if first message from this agent
        if messages.is_empty() {
            context.add_system_message(self.system_prompt.clone()).await;
        }
        
        // Get latest user message
        let user_input: Option<String> = context.get("user_input").await;
        
        if let Some(input) = user_input {
            context.add_user_message(input).await;
            
            // Here you would integrate with your LLM
            let response = format!("[{}] Processed: {}", self.agent_name, "response");
            context.add_assistant_message(response.clone()).await;
            
            // Store for next agent or user
            context.set("last_agent_response", response.clone()).await;
            
            Ok(TaskResult::new(Some(response), NextAction::Continue))
        } else {
            Ok(TaskResult::new(
                Some("Waiting for user input".to_string()),
                NextAction::WaitForInput
            ))
        }
    }
}

// Build multi-agent workflow
let analyst = Arc::new(AgentTask {
    agent_name: "analyst".to_string(),
    system_prompt: "You are a data analyst.".to_string(),
});

let reviewer = Arc::new(AgentTask {
    agent_name: "reviewer".to_string(),
    system_prompt: "You review and critique analysis.".to_string(),
});

let graph = GraphBuilder::new("multi_agent_chat")
    .add_task(analyst.clone())
    .add_task(reviewer.clone())
    .add_conditional_edge(
        analyst.id(),
        |ctx| ctx.get_sync::<bool>("needs_review").unwrap_or(true),
        reviewer.id(),
        analyst.id(), // Loop back for more analysis
    )
    .build();
```

#### Error Handling and Recovery

```rust
struct ResilientTask {
    max_retries: usize,
}

#[async_trait]
impl Task for ResilientTask {
    fn id(&self) -> &str {
        "resilient_task"
    }

    async fn run(&self, context: Context) -> Result<TaskResult> {
        let retry_count: usize = context.get("retry_count").await.unwrap_or(0);
        
        // Simulate work that might fail
        let success = retry_count > 2; // Succeed after 3 attempts
        
        if success {
            context.set("retry_count", 0).await; // Reset for next time
            Ok(TaskResult::new(
                Some("Task completed successfully".to_string()),
                NextAction::Continue
            ))
        } else if retry_count < self.max_retries {
            context.set("retry_count", retry_count + 1).await;
            Ok(TaskResult::new_with_status(
                Some(format!("Attempt {} failed, retrying...", retry_count + 1)),
                NextAction::GoTo("resilient_task".to_string()), // Retry self
                Some(format!("Retry {}/{}", retry_count + 1, self.max_retries))
            ))
        } else {
            Ok(TaskResult::new(
                Some("Task failed after maximum retries".to_string()),
                NextAction::GoTo("error_handler".to_string())
            ))
        }
    }
}
```

#### Dynamic Task Selection

```rust
struct RouterTask;

#[async_trait]
impl Task for RouterTask {
    fn id(&self) -> &str {
        "router"
    }

    async fn run(&self, context: Context) -> Result<TaskResult> {
        let user_type: String = context.get("user_type").await.unwrap_or_default();
        let urgency: String = context.get("urgency").await.unwrap_or_default();
        
        let next_task = match (user_type.as_str(), urgency.as_str()) {
            ("premium", "high") => "priority_handler",
            ("premium", _) => "premium_handler",
            (_, "high") => "urgent_handler",
            _ => "standard_handler",
        };
        
        Ok(TaskResult::new(
            Some(format!("Routing to {}", next_task)),
            NextAction::GoTo(next_task.to_string())
        ))
    }
}
```

## Performance and Best Practices

### Efficient Context Usage

```rust
// ✅ Good: Batch context operations
context.set("key1", value1).await;
context.set("key2", value2).await;
context.set("key3", value3).await;

// ✅ Good: Use sync methods in edge conditions  
.add_conditional_edge(
    "task1",
    |ctx| ctx.get_sync::<bool>("condition").unwrap_or(false),
    "task2",
    "task3"
)

// ✅ Good: Limit chat history size for long conversations
let context = Context::with_max_chat_messages(100);
```

### Memory Management

```rust
// ✅ Good: Reuse Arc references
let shared_task = Arc::new(MyTask::new());
let graph = GraphBuilder::new("workflow")
    .add_task(shared_task.clone())  // Clone the Arc, not the task
    .build();

// ✅ Good: Clear unused context data
context.remove("large_temporary_data").await;
```

### Error Handling

```rust
// ✅ Good: Proper error propagation
async fn run(&self, context: Context) -> Result<TaskResult> {
    let data = context.get("required_data").await
        .ok_or_else(|| GraphError::TaskExecutionFailed(
            "Missing required data".to_string()
        ))?;
    
    // Process data...
    Ok(TaskResult::new(Some("Success".to_string()), NextAction::Continue))
}
```

## Features

### Default Features
The crate works out of the box with basic workflow capabilities.

### `rig` Feature
Enables LLM integration through the Rig crate:

```toml
[dependencies]
graph-flow = { version = "0.2", features = ["rig"] }
```

```rust
#[cfg(feature = "rig")]
{
    // Get messages formatted for LLM consumption
    let messages = context.get_rig_messages().await;
    let recent = context.get_last_rig_messages(10).await;
}
```

## Testing Your Workflows

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_workflow() {
        let task = Arc::new(MyTask::new());
        let graph = Arc::new(
            GraphBuilder::new("test")
                .add_task(task.clone())
                .build()
        );
        
        let storage = Arc::new(InMemorySessionStorage::new());
        let runner = FlowRunner::new(graph, storage.clone());
        
        // Create test session
        let session = Session::new_from_task("test_session".to_string(), task.id());
        session.context.set("test_input", "test_value").await;
        storage.save(session).await.unwrap();
        
        // Execute and verify
        let result = runner.run("test_session").await.unwrap();
        assert!(result.response.is_some());
        
        // Check context was updated
        let session = storage.get("test_session").await.unwrap().unwrap();
        let output: String = session.context.get("expected_output").await.unwrap();
        assert_eq!(output, "expected_value");
    }
}
```

## Migration from 0.1.x

- `Context::get_rig_messages()` replaces manual message conversion
- `TaskResult::new_with_status()` adds debugging support
- `FlowRunner` provides simplified session management
- PostgreSQL storage is now more robust with connection pooling

## License

MIT 