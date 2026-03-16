# Recommended Crate Structure

> How the Rust crates should map to Python LangGraph's package structure.

---

## Python LangGraph Package Layout

```
langgraph (pip install langgraph)
├── langgraph.graph          # StateGraph, MessageGraph
├── langgraph.pregel         # Core execution engine (Pregel)
├── langgraph.channels       # State channels
├── langgraph.managed        # Managed values
├── langgraph.func           # Functional API (@task, @entrypoint)
├── langgraph.types          # Shared types (Command, Send, Interrupt, etc.)
├── langgraph.constants      # START, END
├── langgraph.errors         # Error types
├── langgraph.config         # Config utilities
├── langgraph.runtime        # Runtime context
├── langgraph._internal      # Private implementation details

langgraph-prebuilt (pip install langgraph-prebuilt)
├── langgraph.prebuilt.chat_agent_executor  # create_react_agent
├── langgraph.prebuilt.tool_node            # ToolNode, tools_condition
├── langgraph.prebuilt.tool_validator       # ValidationNode
├── langgraph.prebuilt.interrupt            # HumanInterrupt types

langgraph-checkpoint (pip install langgraph-checkpoint)
├── langgraph.checkpoint.base     # BaseCheckpointSaver, Checkpoint
├── langgraph.checkpoint.memory   # MemorySaver
├── langgraph.checkpoint.serde    # Serialization (jsonplus, msgpack)
├── langgraph.store.base          # BaseStore, Item
├── langgraph.store.memory        # InMemoryStore
├── langgraph.cache               # BaseCache, InMemoryCache

langgraph-checkpoint-postgres
├── langgraph.checkpoint.postgres   # PostgresSaver
├── langgraph.store.postgres        # PostgresStore

langgraph-checkpoint-sqlite
├── langgraph.checkpoint.sqlite     # SqliteSaver
├── langgraph.store.sqlite          # SqliteStore
```

---

## Current Rust Crate Layout

```
rs-graph-llm/
├── graph-flow/                    # Core framework (≈ langgraph + langgraph-prebuilt)
│   ├── src/
│   │   ├── lib.rs                 # Re-exports
│   │   ├── graph.rs               # Graph, GraphBuilder, edges
│   │   ├── task.rs                # Task trait, NextAction, TaskResult
│   │   ├── context.rs             # Context, ChatHistory
│   │   ├── error.rs               # GraphError
│   │   ├── storage.rs             # Session, SessionStorage trait, InMemory
│   │   ├── runner.rs              # FlowRunner
│   │   ├── streaming.rs           # StreamingRunner, StreamChunk, StreamMode
│   │   ├── compat.rs              # StateGraph, START/END, Command
│   │   ├── subgraph.rs            # SubgraphTask
│   │   ├── fanout.rs              # FanOutTask (≈ Send)
│   │   ├── typed_context.rs       # TypedContext<S>
│   │   ├── channels.rs            # Channels, ChannelReducer
│   │   ├── retry.rs               # RetryPolicy, BackoffStrategy
│   │   ├── run_config.rs          # RunConfig, BreakpointConfig
│   │   ├── tool_result.rs         # ToolResult
│   │   ├── react_agent.rs         # create_react_agent()
│   │   ├── task_registry.rs       # TaskRegistry
│   │   ├── thinking.rs            # Thinking graph (custom)
│   │   ├── mcp_tool.rs            # MCP tool integration (custom)
│   │   ├── lance_storage.rs       # LanceSessionStorage
│   │   ├── storage_postgres.rs    # PostgresSessionStorage
│   │   └── agents/
│   │       ├── agent_card.rs      # AgentCard YAML
│   │       └── langgraph_import.rs # JSON import
│   └── Cargo.toml
│
├── graph-flow-server/             # HTTP API (≈ langgraph-api)
│   ├── src/lib.rs
│   └── Cargo.toml
│
├── examples/                      # Example applications
├── insurance-claims-service/      # Full example app
├── recommendation-service/        # Full example app
└── medical-document-service/      # Full example app
```

---

## Recommended Rust Crate Layout (Target)

```
rs-graph-llm/
├── graph-flow/                        # Core engine (≈ langgraph core)
│   ├── src/
│   │   ├── lib.rs
│   │   │
│   │   ├── # ─── Graph Construction ───
│   │   ├── graph.rs                   # Graph, GraphBuilder, Edge, ConditionalEdgeSet
│   │   ├── compat.rs                  # StateGraph, START/END, Command, RoutingDecision
│   │   │
│   │   ├── # ─── Task System ───
│   │   ├── task.rs                    # Task trait, NextAction, TaskResult
│   │   ├── subgraph.rs               # SubgraphTask
│   │   ├── fanout.rs                  # FanOutTask (≈ Send)
│   │   │
│   │   ├── # ─── State Management ───
│   │   ├── context.rs                 # Context, ChatHistory, MessageRole
│   │   ├── typed_context.rs           # TypedContext<S>, State trait
│   │   ├── channels.rs               # Channels, ChannelReducer, ChannelConfig
│   │   │
│   │   ├── # ─── Execution Engine ───
│   │   ├── runner.rs                  # FlowRunner (run, run_batch, run_with_config)
│   │   ├── streaming.rs              # StreamingRunner, StreamChunk, StreamMode
│   │   ├── run_config.rs             # RunConfig, BreakpointConfig
│   │   ├── retry.rs                  # RetryPolicy, BackoffStrategy
│   │   │
│   │   ├── # ─── Storage / Checkpointing ───
│   │   ├── storage.rs                # Session, SessionStorage, InMemorySessionStorage
│   │   ├── storage_postgres.rs       # PostgresSessionStorage
│   │   ├── lance_storage.rs          # LanceSessionStorage (time-travel)
│   │   │
│   │   ├── # ─── Store (NEW — Long-term Memory) ───
│   │   ├── store/                    # NEW module
│   │   │   ├── mod.rs               # BaseStore trait, Item, SearchItem
│   │   │   ├── memory.rs            # InMemoryStore
│   │   │   └── lance.rs             # LanceStore (backed by lance-graph)
│   │   │
│   │   ├── # ─── Errors ───
│   │   ├── error.rs                  # GraphError, Result
│   │   │
│   │   ├── # ─── Prebuilt Components ───
│   │   ├── prebuilt/                 # NEW submodule (was flat files)
│   │   │   ├── mod.rs
│   │   │   ├── react_agent.rs        # create_react_agent() (moved from react_agent.rs)
│   │   │   ├── tool_node.rs          # ToolNode, tools_condition (NEW)
│   │   │   ├── interrupt.rs          # HumanInterrupt types (NEW)
│   │   │   └── validation.rs         # ValidationNode (NEW)
│   │   │
│   │   ├── # ─── Tool System ───
│   │   ├── tool_result.rs            # ToolResult
│   │   ├── mcp_tool.rs               # McpToolTask, MockMcpToolTask
│   │   │
│   │   ├── # ─── Agent Cards ───
│   │   ├── agents/
│   │   │   ├── agent_card.rs         # AgentCard, compile_agent_card
│   │   │   └── langgraph_import.rs   # LangGraph JSON import
│   │   ├── task_registry.rs          # TaskRegistry
│   │   │
│   │   └── # ─── Custom (Our Additions) ───
│   │       └── thinking.rs           # Thinking graph
│   │
│   └── Cargo.toml
│
├── graph-flow-server/                 # HTTP API server (≈ langgraph-api)
│   ├── src/
│   │   ├── lib.rs                    # Router, endpoints
│   │   ├── sse.rs                    # SSE streaming endpoint (NEW)
│   │   └── middleware.rs             # Auth, CORS, logging (NEW)
│   └── Cargo.toml
│
├── graph-flow-macros/                 # Proc macros (NEW — ≈ langgraph.func)
│   ├── src/lib.rs                    # #[task], #[entrypoint] macros
│   └── Cargo.toml
│
├── examples/
├── insurance-claims-service/
├── recommendation-service/
└── medical-document-service/
```

---

## Mapping: Python Package → Rust Crate

| Python Package | Rust Crate | Status |
|---------------|-----------|--------|
| `langgraph` (core) | `graph-flow` | EXISTS |
| `langgraph-prebuilt` | `graph-flow` (prebuilt/ submodule) | PARTIAL |
| `langgraph-checkpoint` (base) | `graph-flow` (storage.rs) | EXISTS |
| `langgraph-checkpoint-postgres` | `graph-flow` (storage_postgres.rs, feature-gated) | EXISTS |
| `langgraph-checkpoint-sqlite` | Not planned (Postgres + Lance sufficient) | SKIP |
| `langgraph.store` | `graph-flow` (store/ submodule) | NEW |
| `langgraph-api` / `langgraph-cli` | `graph-flow-server` | EXISTS |
| `langgraph.func` (@task/@entrypoint) | `graph-flow-macros` | NEW |

---

## Mapping: Python Module → Rust Module

| Python Module | Rust Module | File |
|--------------|------------|------|
| `langgraph.graph.state.StateGraph` | `compat::StateGraph` | `compat.rs` |
| `langgraph.graph.state.CompiledStateGraph` | `graph::Graph` | `graph.rs` |
| `langgraph.graph.message` | `context::ChatHistory` | `context.rs` |
| `langgraph.graph._branch` | `graph::ConditionalEdgeSet` | `graph.rs` |
| `langgraph.pregel.main.Pregel` | `graph::Graph` + `runner::FlowRunner` | `graph.rs` + `runner.rs` |
| `langgraph.pregel.main.NodeBuilder` | `graph::GraphBuilder` | `graph.rs` |
| `langgraph.pregel.protocol` | `task::Task` trait | `task.rs` |
| `langgraph.pregel.remote.RemoteGraph` | `graph-flow-server` (server side) | `lib.rs` |
| `langgraph.channels.*` | `channels::*` | `channels.rs` |
| `langgraph.types.Command` | `compat::Command` | `compat.rs` |
| `langgraph.types.Send` | `fanout::FanOutTask` | `fanout.rs` |
| `langgraph.types.RetryPolicy` | `retry::RetryPolicy` | `retry.rs` |
| `langgraph.types.StreamMode` | `streaming::StreamMode` | `streaming.rs` |
| `langgraph.types.Interrupt` | `task::NextAction::WaitForInput` | `task.rs` |
| `langgraph.errors.*` | `error::GraphError` | `error.rs` |
| `langgraph.checkpoint.base` | `storage::SessionStorage` | `storage.rs` |
| `langgraph.checkpoint.memory` | `storage::InMemorySessionStorage` | `storage.rs` |
| `langgraph.checkpoint.postgres` | `storage_postgres::PostgresSessionStorage` | `storage_postgres.rs` |
| `langgraph.store.base` | `store::BaseStore` (NEW) | `store/mod.rs` |
| `langgraph.store.memory` | `store::InMemoryStore` (NEW) | `store/memory.rs` |
| `langgraph.prebuilt.chat_agent_executor` | `prebuilt::react_agent` | `react_agent.rs` |
| `langgraph.prebuilt.tool_node` | `prebuilt::tool_node` (NEW) | `prebuilt/tool_node.rs` |
| `langgraph.prebuilt.interrupt` | `prebuilt::interrupt` (NEW) | `prebuilt/interrupt.rs` |
| `langgraph.func.task` | `graph-flow-macros` `#[task]` (NEW) | `macros/src/lib.rs` |
| `langgraph.func.entrypoint` | `graph-flow-macros` `#[entrypoint]` (NEW) | `macros/src/lib.rs` |
| `langgraph.runtime` | TBD | — |

---

## Feature Flags

```toml
[features]
default = ["memory"]
memory = []                    # InMemorySessionStorage, InMemoryStore
postgres = ["sqlx"]            # PostgresSessionStorage
lance = ["lance"]              # LanceSessionStorage, LanceStore
mcp = ["rmcp"]                 # McpToolTask
rig = ["rig-core"]             # Rig LLM integration
macros = ["graph-flow-macros"] # #[task], #[entrypoint] proc macros
server = ["axum", "tower"]     # HTTP server components
```

---

## Key Architectural Differences from Python

1. **No Runnable abstraction**: Python LangGraph builds on LangChain's `Runnable` protocol. Rust uses the `Task` trait directly — simpler and more performant.

2. **No channel-per-field model**: Python LangGraph creates one channel per state field. Rust uses `Context` (DashMap) for most cases, with explicit `Channels` for reducer semantics when needed.

3. **No Pregel superstep model**: Python uses a Pregel-inspired superstep execution where all triggered nodes run per step. Rust uses sequential task execution with explicit edges — simpler but less parallel.

4. **Session = Checkpoint**: Python separates Checkpoint (state) from Thread (session). Rust combines them into `Session` with context + position.

5. **Async-native**: Python has sync + async variants for everything. Rust is async-native — no sync wrappers needed (use `tokio::runtime::Runtime::block_on()` for sync callers).

6. **Storage integration**: Python has separate checkpoint + store packages. Rust integrates both into `graph-flow` with feature flags, and lance-graph provides the vector search backend directly.
