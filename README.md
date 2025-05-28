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

## Message History

Tasks can access and store conversation history through the Context:

```rust
// In a task
context.set("messages", messages).await;
let messages: Vec<Message> = context.get("messages").await.unwrap_or_default();
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

## Future Enhancements

- Persistent storage implementations
- WebSocket support for streaming responses
- Graph visualization tools
- More built-in task types
- Distributed execution support

## License

MIT 