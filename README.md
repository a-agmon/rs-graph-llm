<div align="center">
  <img src="image.png" alt="graph-flow" width="300">

  # graph-flow

  ### Stateful Graph Workflow Framework for AI Agents in Rust

  *A type-safe, LangGraph-inspired framework for building complex, interactive, resumable agent workflows*

  [![crates.io](https://img.shields.io/crates/v/graph-flow.svg)](https://crates.io/crates/graph-flow)
  [![docs.rs](https://docs.rs/graph-flow/badge.svg)](https://docs.rs/graph-flow)

  ---
</div>

**graph-flow** combines the two ideas that make LangGraph pleasant to use — a graph execution engine for stateful workflows, and tight LLM ecosystem integration — and rebuilds them natively in Rust:

- **[`graph-flow`](graph-flow/)** — the core graph execution library: task orchestration, session persistence, conditional routing, human-in-the-loop pauses
- **[Rig](https://github.com/0xPlaygrounds/rig)** — Rust-native LLM integration. The optional `rig` feature bridges `Context` chat history to `rig-core`'s `Message`; the agent runtime itself lives in the companion `rig-agent` crate

You get LangGraph-style workflow design with Rust's performance and type safety, a clean database schema for session state, and flexible execution models — step-by-step, fire-and-forget, or a mix of both in the same graph.

## Repository Layout

| Crate                                                    | What it shows                                                                                                             |
|----------------------------------------------------------|---------------------------------------------------------------------------------------------------------------------------|
| [`graph-flow/`](graph-flow/)                             | The framework library (published to crates.io)                                                                            |
| [`insurance-claims-service/`](insurance-claims-service/) | Production-style HTTP service: LLM-driven claim intake, conditional routing, human-in-the-loop approval                   |
| [`recommendation-service/`](recommendation-service/)     | RAG recommendation system with vector search                                                                              |
| [`examples/`](examples/)                                 | Small progressive examples: `simple_example`, `complex_example`, `recommendation_flow`, `fanout_basic`, `terminal_client` |

> **Start here**: read [`examples/simple_example.rs`](examples/simple_example.rs) for the core concepts, then the services for real-world patterns.

## Quick Start

```toml
[dependencies]
graph-flow = { version = "0.7", features = ["rig"] }  # drop "rig" if you don't need LLM helpers
rig-core = "0.42"                                     # provider clients + Message
rig-agent = "0.42"                                    # the agent runtime (Chat, Prompt, .agent())
```

### 1. Define tasks

Tasks are the building blocks of a workflow. Each implements the [`Task`](graph-flow/src/task.rs) trait, reads and writes shared state through `Context`, and returns a `NextAction` that steers the graph:

```rust
use async_trait::async_trait;
use graph_flow::{Context, NextAction, Task, TaskResult};

struct HelloTask;

#[async_trait]
impl Task for HelloTask {
    // id() defaults to the type name; override it for a custom identifier

    async fn run(&self, context: Context) -> graph_flow::Result<TaskResult> {
        let name: String = context.get("name").unwrap_or_default();
        let greeting = format!("Hello, {name}");

        // Store result for the next task
        context.set("greeting", greeting.clone())?;

        // Continue to the next task, but hand control back to the caller
        Ok(TaskResult::new(Some(greeting), NextAction::Continue))
    }
}
```

### 2. Build the graph

```rust
use graph_flow::GraphBuilder;
use std::sync::Arc;

let hello_task = Arc::new(HelloTask);
let excitement_task = Arc::new(ExcitementTask);

let graph = Arc::new(
    GraphBuilder::new("greeting_workflow")
        .add_task(hello_task.clone())
        .add_task(excitement_task.clone())
        .add_edge(hello_task.id(), excitement_task.id())
        .build()?, // validates edges and start task
);
```

### 3. Execute with sessions

Workflows are **stateful**: they can pause, wait for user input, and resume later — across process restarts if you use persistent storage. `FlowRunner` wraps the load → execute one step → save cycle:

```rust
use graph_flow::{ExecutionStatus, FlowRunner, InMemorySessionStorage, Session, SessionStorage};
use std::sync::Arc;

let storage = Arc::new(InMemorySessionStorage::new());
let runner = FlowRunner::new(graph.clone(), storage.clone());

// Create a session positioned at the first task
let session = Session::new_from_task("session_001".to_string(), hello_task.id());
session.context.set("name", "Batman".to_string())?;
storage.save(session).await?;

// Drive the workflow step by step
loop {
    let result = runner.run("session_001").await?;
    println!("Response: {:?}", result.response);

    match result.status {
        ExecutionStatus::Completed => break,
        ExecutionStatus::Paused { .. } => continue,   // will auto-continue to the next task
        ExecutionStatus::WaitingForInput => break,    // collect user input, set it in context, run again
    }
}
```

Prefer manual control (custom persistence, batching)? Use the lower-level API — it is exactly what `FlowRunner` does internally:

```rust
let mut session = storage.get("session_001").await?.unwrap();
let result = graph.execute_session(&mut session).await?;
storage.save(session).await?;
```

## Execution Control

You choose how the graph advances by returning the right `NextAction` from each task:

| `NextAction` | Behavior |
|---|---|
| `Continue` | Advance one edge, then return control to the caller (step-by-step) |
| `ContinueAndExecute` | Advance and keep executing tasks in the same call until something pauses |
| `WaitForInput` | Pause at the current task until new input arrives |
| `GoTo(task_id)` | Jump to a specific task |
| `End` | Complete the workflow |

**`Continue`** is the interactive mode: after every hop the engine saves state and returns, so a web service can respond to the client between steps.

**`ContinueAndExecute`** is fire-and-forget: the engine immediately runs the next task with the same context, and keeps going until a task returns `Continue`, `WaitForInput`, or `End`. Blend both freely — automated chains with interactive pauses in one workflow.

```rust
// Step-by-step: caller decides when the next task runs
Ok(TaskResult::new(Some("Done".into()), NextAction::Continue))

// Continuous: single runner.run() call executes the whole chain
Ok(TaskResult::new(None, NextAction::ContinueAndExecute))
```

### ExecutionStatus

Every execution returns an [`ExecutionResult`](graph-flow/src/graph.rs) with the task's response and a status:

- **`Paused { next_task_id, reason }`** — task finished; the workflow will proceed to `next_task_id` on the next run (returned for `Continue` and `GoTo`)
- **`WaitingForInput`** — workflow is parked on the current task until you provide input and run again
- **`Completed`** — the workflow reached `End`

Failures are reported as `Err(GraphError)` from the execution call itself.

### Guard rails

Two builder options protect long-running graphs:

```rust
let graph = GraphBuilder::new("wf")
    .add_task(task)
    .with_task_timeout(std::time::Duration::from_secs(60)) // per-task timeout (default: 5 min)
    .with_max_execution_steps(50) // cap chained ContinueAndExecute steps (default: unlimited)
    .build()?;
```

`with_max_execution_steps` turns an accidental `ContinueAndExecute` cycle into an error instead of an infinite loop.

## Advanced Features

### Conditional edges

Branch at runtime on data in the `Context` ([`complex_example.rs`](examples/complex_example.rs)):

```rust
let graph = GraphBuilder::new("sentiment_flow")
    .add_task(sentiment_task.clone())
    .add_task(positive_task.clone())
    .add_task(negative_task.clone())
    .add_conditional_edge(
        sentiment_task.id(),
        |ctx| ctx.get::<String>("sentiment").map(|s| s == "positive").unwrap_or(false),
        positive_task.id(), // yes branch
        negative_task.id(), // else branch
    )
    .build()?;
```

```mermaid
graph TD
    SA["SentimentAnalysisTask"] --> S{Sentiment?}
    S -->|"positive"| P["PositiveResponseTask"]
    S -->|"negative"| N["NegativeResponseTask"]
```

### LLM integration with Rig

With the `rig` feature, chat history stored in the `Context` converts directly to Rig messages:

```rust
// `graph-flow`'s `rig` feature needs only `rig-core` (for `Message`).
// The agent runtime — `Chat`, `Prompt`, `.agent()` — lives in `rig-agent`.
use rig_agent::prelude::*;
use rig_core::providers::openrouter;

async fn run(&self, context: Context) -> graph_flow::Result<TaskResult> {
    let user_input: String = context.get("user_input").unwrap();

    let client = openrouter::Client::new(&api_key)
        .map_err(|e| graph_flow::GraphError::TaskExecutionFailed(e.to_string()))?;
    let agent = client
        .agent("openai/gpt-4o-mini")
        .preamble("You are a helpful insurance assistant")
        .build();

    // Conversation context for the LLM. `chat` seeds the request from this
    // vector and appends the turn's committed messages back into it.
    let mut chat_history = context.get_rig_messages();
    let response = agent.chat(&user_input, &mut chat_history).await?;

    // Persist the exchange
    context.add_user_message(user_input);
    context.add_assistant_message(response.clone());

    Ok(TaskResult::new(Some(response), NextAction::Continue))
}
```

### Context and state management

[`Context`](graph-flow/src/context.rs) is thread-safe, typed, and fully serialized with the session:

All `Context` methods are synchronous (the store is an in-memory map), so the same calls work in async tasks and in edge-condition closures:

```rust
// Typed key-value state; set() is fallible (serialization can fail)
context.set("claim_amount", 1500.0)?;
let amount: f64 = context.get("claim_amount").unwrap();

// Works directly in edge conditions
let ok = context.get::<bool>("condition").unwrap_or(false);

// Built-in chat history (ring buffer, capped at 1000 messages by default)
context.add_user_message("What's my claim status?".to_string());
context.add_assistant_message("Your claim is being processed".to_string());
let recent = context.get_last_messages(5);
```

### Parallel execution (FanOut)

[`FanOutTask`](graph-flow/src/fanout.rs) runs child tasks concurrently within one graph node, waits for all of them, and aggregates their outputs into the context under prefixed keys ([`fanout_basic.rs`](examples/fanout_basic.rs)):

```rust
use graph_flow::FanOutTask;

let fanout = FanOutTask::new("fanout", vec![Arc::new(ChildA), Arc::new(ChildB)])
    .with_prefix("parallel");

// FanOut is a regular task in the graph
let graph = GraphBuilder::new("fanout_demo")
    .add_task(prepare_task.clone())
    .add_task(fanout.clone())
    .add_task(consume_task.clone())
    .add_edge(prepare_task.id(), fanout.id())
    .add_edge(fanout.id(), consume_task.id())
    .build()?;

// Downstream tasks read: parallel.child_a.response, parallel.child_b.response, ...
```

Caveats: children's `NextAction` is ignored (the FanOut node controls flow), all children share one context, and if any child fails the FanOut fails with the first error (other children still complete, so partial results may be present).

### Storage backends

Both backends implement the same `SessionStorage` trait — swap freely between development and production:

```rust
// In-memory (development)
let storage = Arc::new(InMemorySessionStorage::new());

// PostgreSQL (production)
let storage = Arc::new(PostgresSessionStorage::connect(&database_url).await?);

// Or bring your own pool configuration
let pool = sqlx::postgres::PgPoolOptions::new().max_connections(20).connect(&url).await?;
let storage = Arc::new(PostgresSessionStorage::with_pool(pool).await?);
```

## Case Study: Insurance Claims Processing

The [`insurance-claims-service`](insurance-claims-service/) is a complete agentic workflow behind an Axum HTTP API:

```mermaid
graph TD
    A["Initial Claim Query<br/>LLM conversation"] --> B["Insurance Type Classifier"]
    B --> C{Insurance Type?}
    C -->|"Car"| D["Car Insurance Details"]
    C -->|"Apartment"| E["Apartment Insurance Details"]
    D --> F["Smart Claim Validator"]
    E --> F
    F --> G{Claim Amount?}
    G -->|"< $1000"| H["Auto-Approve"]
    G -->|"≥ $1000"| I["Manual Review<br/>(waits for human input)"]
    I --> K{Approval Decision?}
    K -->|"approved"| L["Manual Approval"]
    K -->|"other"| M["Final Summary"]
    H --> M
    L --> M
```

It demonstrates the framework's key patterns working together:

- **LLM-first tasks** — every interactive step uses an agent for natural-language understanding ([`initial_claim_query.rs`](insurance-claims-service/src/tasks/initial_claim_query.rs))
- **Structured extraction** — tasks parse JSON out of LLM responses into typed context values
- **Content-based routing** — conditional edges route on the LLM's classification ([`insurance_type_classifier.rs`](insurance-claims-service/src/tasks/insurance_type_classifier.rs))
- **Human-in-the-loop** — claims ≥ $1000 return `WaitForInput` and park the session until a reviewer responds ([`smart_claim_validator.rs`](insurance-claims-service/src/tasks/smart_claim_validator.rs))
- **Full state tracking** — chat history, structured claim data, workflow position, and status messages survive across requests

### Running it

```bash
export OPENROUTER_API_KEY="your-key"
export DATABASE_URL="postgresql://user:pass@localhost/db"  # optional, defaults to in-memory

cargo run --bin insurance-claims-service

# Start a claim
curl -X POST http://localhost:3000/execute \
  -H "Content-Type: application/json" \
  -d '{"content": "I need to file a claim for my car accident"}'

# Continue the conversation (use the session_id from the response)
curl -X POST http://localhost:3000/execute \
  -H "Content-Type: application/json" \
  -d '{"session_id": "uuid-here", "content": "It happened yesterday on Main Street"}'

# Inspect session state
curl http://localhost:3000/session/uuid-here
```

The HTTP handler is one workflow step per request:

```rust
async fn execute_graph(
    State(state): State<AppState>,
    Json(request): Json<ExecuteRequest>,
) -> Result<Json<ExecuteResponse>, StatusCode> {
    // Store the user's message in the session context
    let session = state.session_storage.get(&session_id).await?;
    session
        .context
        .set("user_input", request.content)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    state.session_storage.save(session).await?;

    // Execute one step; FlowRunner loads, runs, and saves the session
    let result = state.flow_runner.run(&session_id).await?;

    Ok(Json(ExecuteResponse {
        session_id,
        response: result.response,
        status: format!("{:?}", result.status),
    }))
}
```

## Production Considerations

- **Per-session concurrency**: sessions carry an optimistic-locking version. If two runs of the same session race, the losing save fails with `GraphError::SessionConflict` instead of silently overwriting — reload and retry on conflict. Task side effects from the losing attempt are not rolled back, so keep tasks idempotent if concurrent runs are possible.
- **Timeouts and step limits**: set `with_task_timeout` to bound slow LLM calls, and `with_max_execution_steps` if your graph contains cycles reachable via `ContinueAndExecute`.
- **Durability of chained steps**: within one `ContinueAndExecute` chain, the session is saved only after the chain finishes — a crash mid-chain re-runs from the chain's first task.

## API Stability

`0.6.0` completed the API cleanup planned during the 0.5 engine review: synchronous fallible `Context` methods, validated immutable graphs (`build()` returns `Result`), optimistic session locking, and removal of the deprecated `Graph::execute`, `NextAction::GoBack`, and `ExecutionStatus::Error`. The full list and a **0.5 → 0.6 migration guide** live in [`graph-flow/ROADMAP.md`](graph-flow/ROADMAP.md).

## License

MIT License — see [LICENSE](LICENSE).
