# graph-flow

A high-performance, type-safe framework for building stateful, multi-agent LLM workflows in Rust — inspired by LangGraph, integrated with [Rig](https://github.com/0xPlaygrounds/rig).

## Features

- **Type-Safe Workflows**: Compile-time guarantees, validated graph construction
- **Flexible Execution**: Step-by-step, fire-and-forget, or mixed in one graph
- **Built-in Persistence**: PostgreSQL and in-memory session storage with optimistic locking
- **LLM Integration**: Optional Rig integration (`rig` feature) with built-in chat history
- **Human-in-the-Loop**: Pause on `WaitForInput`, resume when the user answers
- **Parallel Blocks**: `FanOutTask` runs multiple tasks concurrently inside a single node

## Cargo features

- `postgres` (enabled by default): provides `PostgresSessionStorage` and its SQLx dependency.
- `rig`: enables Rig message integration independently of storage.

PostgreSQL support is enabled by default; set `default-features = false` to exclude it.

## Quick Start

```toml
[dependencies]
graph-flow = { version = "0.7", features = ["rig"] }  # drop "rig" if you don't need LLM helpers
```

Migrating from an earlier version? See the [migration guides](https://github.com/a-agmon/rs-graph-llm/blob/main/graph-flow/ROADMAP.md) — 0.7 moves the `rig` feature to `rig-core` 0.42.

### Define a task

```rust
use async_trait::async_trait;
use graph_flow::{Context, NextAction, Task, TaskResult};

struct HelloTask;

#[async_trait]
impl Task for HelloTask {
    async fn run(&self, context: Context) -> graph_flow::Result<TaskResult> {
        let name: String = context.get("name").unwrap_or_default();
        let greeting = format!("Hello, {name}!");

        context.set("greeting", greeting.clone())?;
        Ok(TaskResult::new(Some(greeting), NextAction::Continue))
    }
}
```

`Context` methods are synchronous and thread-safe; `set` is fallible (JSON serialization can fail).

### Build a graph

```rust
use graph_flow::GraphBuilder;
use std::sync::Arc;

let graph = Arc::new(
    GraphBuilder::new("my_workflow")
        .add_task(hello_task.clone())
        .add_task(next_task.clone())
        .add_edge(hello_task.id(), next_task.id())
        .add_conditional_edge(
            next_task.id(),
            |ctx| ctx.get::<bool>("flag").unwrap_or(false),
            yes_task.id(),
            no_task.id(),
        )
        .build()?, // validates edges and start task; the graph is immutable afterwards
);
```

### Execute with sessions

```rust
use graph_flow::{ExecutionStatus, FlowRunner, InMemorySessionStorage, Session, SessionStorage};
use std::sync::Arc;

let storage = Arc::new(InMemorySessionStorage::new());
let runner = FlowRunner::new(graph.clone(), storage.clone());

let session = Session::new_from_task("session_001".to_string(), hello_task.id());
session.context.set("name", "Alice".to_string())?;
storage.save(session).await?;

loop {
    let result = runner.run("session_001").await?;
    match result.status {
        ExecutionStatus::Completed => break,
        ExecutionStatus::Paused { .. } => continue,   // auto-continues to the next task
        ExecutionStatus::WaitingForInput => break,    // collect input, set it in context, run again
    }
}
```

Failures are returned as `Err(GraphError)`. Sessions carry an optimistic-locking version: concurrent saves of the same session fail with `GraphError::SessionConflict` instead of losing updates.

## Execution control

Each task returns a `NextAction`:

| Variant | Behavior |
|---|---|
| `Continue` | Advance one edge, return control to the caller |
| `ContinueAndExecute` | Advance and keep executing until something pauses |
| `WaitForInput` | Park the workflow until new input arrives |
| `GoTo(task_id)` | Jump to a specific task |
| `End` | Complete the workflow |

Guard rails: `GraphBuilder::with_task_timeout(duration)` bounds each task (default 5 min); `with_max_execution_steps(n)` turns a `ContinueAndExecute` cycle into an error instead of an infinite loop.

## LLM integration (feature `rig`)

```rust
let mut chat_history = context.get_rig_messages();
let response = agent.chat(&user_input, &mut chat_history).await?;

context.add_user_message(user_input);
context.add_assistant_message(response.clone());
```

The `rig` feature pulls in `rig-core` alone, for the `rig_core::completion::Message`
conversion. Building an agent (`Chat`, `Prompt`, `.agent()`) needs the companion
`rig-agent` crate as a direct dependency of your own crate.

Chat history is a capped ring buffer (default 1000 messages) and serializes with the session.

## Storage

```rust
// Development
let storage = Arc::new(InMemorySessionStorage::new());

// Production (session ids are free-form TEXT)
let storage = Arc::new(PostgresSessionStorage::connect(&database_url).await?);

// Custom pool configuration
let storage = Arc::new(PostgresSessionStorage::with_pool(pool).await?);
```

Both implement the same `SessionStorage` trait.

## More

Full documentation, runnable examples, and complete service implementations (insurance claims workflow, RAG recommendation service) live in the [repository](https://github.com/a-agmon/rs-graph-llm).

## License

MIT
