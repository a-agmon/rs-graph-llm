# LangGraph Feature Parity Audit

## Overview

This document maps every Python LangGraph concept to its graph-flow (Rust) equivalent,
identifies feature gaps, and tracks implementation status.

## Feature Mapping

| Python LangGraph | graph-flow (Rust) | Status | Notes |
|---|---|---|---|
| `StateGraph` | `GraphBuilder` | COMPLETE | Fluent builder API |
| `StateGraph.add_node()` | `.add_task()` | COMPLETE | Tasks = Nodes |
| `StateGraph.add_edge()` | `.add_edge()` | COMPLETE | Direct edges |
| `StateGraph.add_conditional_edges()` | `.add_conditional_edge()` | COMPLETE | Binary yes/no conditions |
| `StateGraph.compile()` | `.build()` | COMPLETE | Returns `Graph` |
| `CompiledGraph.invoke()` | `FlowRunner::run()` | COMPLETE | Session-based execution |
| `CompiledGraph.stream()` | `StreamingRunner::run_streaming()` | NEW | Yields `StreamChunk` per task |
| `MemorySaver` | `InMemorySessionStorage` | COMPLETE | DashMap-backed |
| `SqliteSaver` | N/A | SKIPPED | Postgres preferred for production |
| `PostgresSaver` | `PostgresSessionStorage` | COMPLETE | sqlx-based |
| `State` (TypedDict) | `TypedContext<S>` | NEW | Generic typed state wrapper |
| `MessageState` | `Context` chat history | COMPLETE | Built-in chat history management |
| `ToolNode` | `McpToolTask` | NEW | MCP protocol integration |
| `human_in_the_loop` | `WaitForInput` / `NextAction` | COMPLETE | Native support |
| `Subgraphs` | `SubgraphTask` | NEW | Task wrapping inner Graph |
| `Send` (parallel branches) | `FanOutTask` | COMPLETE | JoinSet-based parallelism |
| `Breakpoints` | `NextAction::Continue` | COMPLETE | Step-by-step execution |
| `Time Travel` | `LanceSessionStorage` | NEW | Lance dataset versioning |
| `Channels` (topic, last_value) | Context key-value | PARTIAL | No channel abstraction yet |
| `Pregel` engine | `Graph::execute_session` | COMPLETE | Recursive execution engine |
| `Retry` policy | N/A | DEFERRED | Can be implemented per-task |
| `RunnableConfig` | N/A | DEFERRED | Config passed via Context |

## Feature Gap Priority

### P0 — Critical for parity
1. **Streaming** (`StreamingTask`, `StreamChunk`, `StreamingRunner`)
2. **Subgraphs** (`SubgraphTask`) — needed for agent composition
3. **MCP Tool Integration** (`McpToolTask`) — needed for tool-using agents

### P1 — Important for production
4. **Typed State** (`TypedContext<S>`) — type safety for complex workflows
5. **Lance Storage** (`LanceSessionStorage`) — time travel + vector search
6. **Agent Card YAML** — declarative agent definitions

### P2 — Nice to have
7. **LangGraph Import** — migration path from Python
8. **Multi-condition edges** — more than binary routing
9. **Channel abstractions** — topic/last_value semantics

## Architecture Differences

### Python LangGraph
- Channel-based state management (topic channels, last_value channels)
- Pregel-inspired superstep execution
- State is a TypedDict with reducer functions
- Compiled graph is a Runnable (LangChain interface)

### Rust graph-flow
- Context-based state management (thread-safe HashMap + ChatHistory)
- Recursive task execution with NextAction control flow
- State is dynamic (serde_json::Value) with optional typed wrapper
- Graph executes via sessions with pluggable storage

### Key Design Decisions
1. **No channels**: graph-flow uses a flat key-value Context instead of typed channels.
   This is simpler but less structured. The `TypedContext<S>` addition provides
   optional type safety without the channel complexity.
2. **No Pregel supersteps**: graph-flow executes tasks sequentially (or parallel via FanOut).
   This is more predictable and easier to debug.
3. **Session-first**: Every execution is session-based, making persistence natural.
   LangGraph treats persistence as optional (checkpointer).
