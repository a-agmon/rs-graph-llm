# RS-Inter-Task: LangGraph-like Interactive Workflows in Rust

A Rust platform for building and running interactive workflow graphs with LLM integration using the Rig crate.

## Architecture

The project is organized as a workspace with two main components:

- **graph-flow**: A library for defining and executing task graphs
- **graph-service**: An Axum web service that exposes the graph execution as HTTP endpoints

## Features

- **Task-based workflows**: Define tasks as traits that can be chained together
- **LLM Integration**: Built-in support for LLM interactions using the Rig crate with OpenRouter
- **Conditional edges**: Create dynamic workflows with conditional branching
- **Session management**: Maintain state across multiple interactions
- **Chat History Management**: Built-in serializable chat history separated from context variables
- **Full Serialization**: Both context and chat history are fully serializable for persistence
- **Storage abstraction**: Pluggable storage backends (in-memory by default)
- **Type-safe**: Leverages Rust's type system for safe graph construction

## Quick Start

### Prerequisites

- Rust 1.70+
- OpenRouter API key (for the example tasks)

### Running the Service

1. Set your OpenRouter API key:
```bash
export OPENROUTER_API_KEY="your-api-key"
```

2. Run the service:
```bash
cargo run --bin graph-service
```

The service will start on `http://localhost:3000`

### API Endpoints

#### Execute Graph
```bash
POST /execute
{
  "session_id": "optional-session-id",
  "content": "your query here"
}
```

Response:
```json
{
  "session_id": "generated-or-provided-session-id",
  "response": "Task response",
  "status": "Continue|End|WaitForInput"
}
```

#### Get Session
```bash
GET /session/{session_id}
```

#### Health Check
```bash
GET /health
```

## Creating Custom Tasks

To create a custom task, implement the `Task` trait:

```rust
use async_trait::async_trait;
use graph_flow::{Context, Result, Task, TaskResult, NextAction};

pub struct MyCustomTask {
    id: String,
}

#[async_trait]
impl Task for MyCustomTask {
    fn id(&self) -> &str {
        &self.id
    }

    async fn run(&self, context: Context) -> Result<TaskResult> {
        // Your task logic here
        
        Ok(TaskResult {
            response: Some("Task completed".to_string()),
            next_action: NextAction::Continue,
        })
    }
}
```

## Building Graphs

Use the `GraphBuilder` to construct workflows:

```rust
use graph_flow::{GraphBuilder, Context};
use std::sync::Arc;

let graph = GraphBuilder::new("my-graph")
    .add_task(Arc::new(Task1::new()))
    .add_task(Arc::new(Task2::new()))
    .add_task(Arc::new(Task3::new()))
    .add_edge("task1", "task2")
    .add_conditional_edge("task2", "task3", |ctx: &Context| {
        // Condition logic
        true
    })
    .set_start_task("task1")
    .build();
```

## LLM Integration

The platform uses OpenRouter for LLM access, which provides access to various models including OpenAI, Anthropic, and others. Example:

```rust
use rig::{agent::Agent, providers::openrouter, completion::Prompt};

pub fn get_llm_agent(prompt: &str) -> anyhow::Result<Agent<openrouter::CompletionModel>> {
    let api_key = std::env::var("OPENROUTER_API_KEY")?;
    let client = openrouter::Client::new(&api_key);
    let agent = client.agent("openai/gpt-4o-mini").preamble(prompt).build();
    Ok(agent)
}

// Use the agent
let agent = get_llm_agent("You are a helpful assistant")?;
let response = agent.prompt("Hello!").await?;
```

## Storage

The platform uses trait-based storage abstractions:

- `GraphStorage`: For storing graph definitions
- `SessionStorage`: For storing session state

Default implementations use in-memory storage, but you can implement these traits for any backend (Redis, PostgreSQL, etc.).

## Chat History Management

The platform provides built-in chat history management with full serialization support. Chat history is separated from regular context variables and provides a clean API for conversation management.

### Using Chat History in Tasks

```rust
use graph_flow::{Context, SerializableMessage, MessageRole};

// In a task implementation
async fn run(&self, context: Context) -> Result<TaskResult> {
    // Add messages to chat history
    context.add_user_message("Hello!".to_string()).await;
    context.add_assistant_message("Hi there!".to_string()).await;
    context.add_system_message("System notification".to_string()).await;
    
    // Get chat history
    let history = context.get_chat_history().await;
    println!("Chat has {} messages", history.len());
    
    // Get recent messages
    let last_5 = context.get_last_messages(5).await;
    
    // Clear chat history if needed
    context.clear_chat_history().await;
    
    Ok(TaskResult::new(Some("Response".to_string()), NextAction::Continue))
}
```

### Chat History with LLM Integration

For tasks that use LLM agents with rig, use the `ContextRigExt` trait:

```rust
use crate::chat_bridge::ContextRigExt;
use rig::completion::Chat;

// In a task that uses LLM
async fn run(&self, context: Context) -> Result<TaskResult> {
    let user_input = "User's question";
    
    // Get chat history in rig format for LLM
    let chat_history = context.get_rig_messages().await;
    
    // Use with LLM agent
    let agent = get_llm_agent("Your prompt")?;
    let response = agent.chat(&user_input, chat_history).await?;
    
    // Store the conversation
    context.add_user_message(user_input.to_string()).await;
    context.add_assistant_message(response.clone()).await;
    
    Ok(TaskResult::new(Some(response), NextAction::Continue))
}
```

### Chat History Types

```rust
use graph_flow::{SerializableMessage, MessageRole, ChatHistory};

// Create messages manually
let user_msg = SerializableMessage::user("Hello".to_string());
let assistant_msg = SerializableMessage::assistant("Hi!".to_string());
let system_msg = SerializableMessage::system("System alert".to_string());

// Create chat history with message limit
let mut history = ChatHistory::with_max_messages(100);
history.add_user_message("Message content".to_string());
```

### Context Serialization

Both regular context data and chat history are fully serializable:

```rust
use graph_flow::Context;

// Context automatically serializes both data and chat history
let context = Context::new();
context.set("key", "value").await;
context.add_user_message("Hello".to_string()).await;

// Serialize the entire context (including chat history)
let serialized = serde_json::to_string(&context).unwrap();

// Deserialize back
let restored: Context = serde_json::from_str(&serialized).unwrap();
assert_eq!(restored.chat_history_len().await, 1);
```

## Development

### Running Tests
```bash
cargo test
```

### Building
```bash
cargo build --release
```

## Recent Updates

### Chat History Refactoring (Latest)

- **Separated Chat History**: Chat history is now separated from regular context variables with dedicated storage
- **Full Serialization**: Both context data and chat history are completely serializable for session persistence
- **New Chat API**: Clean, semantic methods for chat management (`add_user_message`, `get_chat_history`, etc.)
- **LLM Bridge**: Seamless integration with rig library through `ContextRigExt` trait
- **Type Safety**: Custom `SerializableMessage` types with compile-time validation
- **Backward Compatibility**: All existing context operations continue to work unchanged

### Architecture Improvements

- **Thread-Safe Design**: Concurrent access to both context data and chat history
- **Memory Management**: Optional message limits for long-running conversations
- **Performance**: Optimized storage with separate handling for data and messages
- **Maintainability**: Clear separation of concerns and well-defined interfaces

## Future Enhancements

- Enhanced message metadata and filtering
- Dedicated chat history persistence backends
- WebSocket support for streaming responses
- Graph visualization tools
- More built-in task types
- Distributed execution support
- Conversation analytics and insights

## License

MIT 