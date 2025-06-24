# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

### Building and Running
```bash
# Build all workspace members
cargo build

# Build and run specific services
cargo run --bin insurance-claims-service
cargo run --bin recommendation-service

# Run examples
cargo run --bin simple_example
cargo run --bin complex_example  
cargo run --bin recommendation_flow
cargo run --bin terminal_client
```

### Testing
```bash
# Run all tests
cargo test

# Run tests for specific crate
cargo test -p graph-flow
cargo test -p insurance-claims-service
```

### Environment Setup
Required environment variables:
- `OPENROUTER_API_KEY`: For LLM integration via OpenRouter
- `DATABASE_URL`: PostgreSQL connection string (optional - uses in-memory storage if not set)

## Architecture Overview

This is a Rust implementation of a LangGraph-inspired stateful graph execution framework for LLM agents. The architecture consists of:

### Core Framework (`graph-flow/`)
- **Graph execution engine**: Handles task orchestration, state management, conditional routing
- **Session management**: Persistent workflow state with pluggable storage backends
- **Context system**: Thread-safe state sharing between tasks with built-in chat history management
- **Task abstraction**: Async trait-based task system with timeout support

### Key Components

#### Task System
- Tasks implement the `Task` trait with `run(context: Context) -> TaskResult`
- `TaskResult` controls flow with `NextAction` enum:
  - `Continue`: Move to next task, return control to caller (step-by-step execution)
  - `ContinueAndExecute`: Move to next task and execute immediately (fire-and-forget)
  - `WaitForInput`: Pause workflow waiting for user input
  - `End`: Complete workflow
  - `GoTo(task_id)`: Jump to specific task

#### Graph Structure
- Built using `GraphBuilder` with fluent API
- Supports conditional edges with runtime evaluation: `add_conditional_edge(from, condition, yes_branch, no_branch)`
- Tasks connected via directed edges with optional conditions

#### Session Management
- `Session` contains: id, graph_id, current_task_id, status_message, context
- Storage backends: `InMemorySessionStorage` (dev) and `PostgresSessionStorage` (prod)
- `FlowRunner` provides high-level session management (auto load/save)

#### Context System
- Thread-safe typed data storage: `context.set("key", value)` / `context.get("key")`
- Built-in chat history: `add_user_message()`, `add_assistant_message()`, `get_chat_history()`
- Rig integration: `get_rig_messages()` for LLM conversation context

### Example Applications

#### Insurance Claims Service (`insurance-claims-service/`)
Complete multi-step insurance processing workflow:
- LLM-driven claim intake and classification
- Conditional routing (car vs apartment insurance)
- Business rule validation ($1000 auto-approval threshold)
- Human-in-the-loop approval for high-value claims
- Comprehensive state tracking throughout process

#### Recommendation Service (`recommendation-service/`)
RAG-based recommendation system with vector search integration.

### Integration Patterns

#### LLM Integration (via Rig)
```rust
let agent = client.agent("openai/gpt-4o-mini")
    .preamble("System prompt")
    .build();
let response = agent.chat(&user_input, context.get_rig_messages().await).await?;
```

#### HTTP Service Wrapper
Services typically expose HTTP APIs using Axum:
- `/execute` endpoint for workflow execution
- `/session/{id}` for session state inspection
- State management handled by `FlowRunner`

## Development Patterns

### Task Implementation
- Tasks should be stateless - all state goes in `Context`
- Use `TaskResult::new_with_status()` for debugging/logging information
- Return appropriate `NextAction` to control workflow flow
- Handle errors via `Result<TaskResult>` return type

### Graph Construction
- Define all tasks first, then connect with edges
- Use conditional edges for runtime branching
- Validate graph connectivity (builder warns about orphaned tasks)
- Start task is automatically set to first added task

### Session Lifecycle
1. Create session with starting task: `Session::new_from_task()`
2. Set initial context data: `session.context.set()`
3. Execute step-by-step: `flow_runner.run(session_id)` 
4. Handle execution results based on `ExecutionStatus`

### Storage Strategy
- Development: Use `InMemorySessionStorage` for fast iteration
- Production: Use `PostgresSessionStorage` for persistence and scalability
- Both implement same `SessionStorage` trait for easy swapping

## Workspace Structure

- `graph-flow/`: Core framework library
- `insurance-claims-service/`: Complete insurance workflow example
- `recommendation-service/`: RAG recommendation system
- `examples/`: Simple demonstrations and learning materials

The examples/ folder contains multiple binaries showing different complexity levels - start with `simple_example.rs` for basics, then explore `recommendation_flow.rs` for a complete end-to-end RAG workflow.