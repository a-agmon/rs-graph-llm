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
graph-flow = "0.1"

# For LLM integration
graph-flow = { version = "0.1", features = ["rig"] }
```

### Basic Usage

```rust
use graph_flow::{Context, Task, TaskResult, NextAction, GraphBuilder};
use async_trait::async_trait;
use std::sync::Arc;

// Define a task
struct HelloTask;

#[async_trait]
impl Task for HelloTask {
    async fn run(&self, context: Context) -> graph_flow::Result<TaskResult> {
        let name: String = context.get_sync("name").unwrap();
        let greeting = format!("Hello, {}", name);
        
        context.set("greeting", greeting.clone()).await;
        Ok(TaskResult::new(Some(greeting), NextAction::Continue))
    }
}

#[tokio::main]
async fn main() -> graph_flow::Result<()> {
    // Build the graph
    let hello_task = Arc::new(HelloTask);
    let graph = Arc::new(GraphBuilder::new("greeting_workflow")
        .add_task(hello_task.clone())
        .build());

    // Execute
    let context = Context::new();
    context.set("name", "World".to_string()).await;
    
    let result = graph.execute(hello_task.id(), context).await?;
    println!("Result: {:?}", result.response);
    
    Ok(())
}
```

### With Session Management

```rust
use graph_flow::{InMemorySessionStorage, FlowRunner, Session};

#[tokio::main]
async fn main() -> graph_flow::Result<()> {
    let session_storage = Arc::new(InMemorySessionStorage::new());
    let flow_runner = FlowRunner::new(graph, session_storage.clone());
    
    // Create session
    let session = Session::new_from_task("session_001".to_string(), hello_task.id());
    session.context.set("name", "World".to_string()).await;
    session_storage.save(session).await?;
    
    // Execute workflow
    let result = flow_runner.run("session_001").await?;
    println!("Response: {:?}", result.response);
    
    Ok(())
}
```

## Core Concepts

### Tasks
Tasks are the building blocks of your workflow. Implement the `Task` trait:

```rust
#[async_trait]
impl Task for MyTask {
    async fn run(&self, context: Context) -> graph_flow::Result<TaskResult> {
        // Your task logic here
        Ok(TaskResult::new(Some("Done".to_string()), NextAction::End))
    }
}
```

### Context
Thread-safe state management across your workflow:

```rust
// Store data
context.set("key", value).await;

// Retrieve data
let value: String = context.get("key").await.unwrap();

// Synchronous access (when you know data is already set)
let value: String = context.get_sync("key").unwrap();
```

### Graph Building
Connect tasks to create workflows:

```rust
let graph = GraphBuilder::new("my_workflow")
    .add_task(task1.clone())
    .add_task(task2.clone())
    .add_edge(task1.id(), task2.id())  // task1 -> task2
    .add_conditional_edge(
        task1.id(),
        |ctx| ctx.get_sync::<bool>("condition").unwrap_or(false),
        task2.id(),    // if true
        task3.id(),    // if false
    )
    .build();
```

### Storage Backends

#### In-Memory (Development)
```rust
let storage = Arc::new(InMemorySessionStorage::new());
```

#### PostgreSQL (Production)
```rust
let storage = Arc::new(
    PostgresSessionStorage::connect(&database_url).await?
);
```

## Features

### Default Features
The crate works out of the box with basic workflow capabilities.

### `rig` Feature
Enables LLM integration through the Rig crate:

```rust
// Chat history management
context.add_user_message("Hi there!".to_string()).await;
context.add_assistant_message("Hello!".to_string()).await;

// Get messages for LLM
let messages = context.get_rig_messages().await;
```

## License

MIT 