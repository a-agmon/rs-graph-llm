# Our Additions: Features in Rust graph-flow That LangGraph Doesn't Have

> Things we built that go beyond Python LangGraph's feature set.

---

## 1. Lance-backed Session Storage with Time Travel

**File:** `graph-flow/src/lance_storage.rs`

Python LangGraph has checkpoint versioning via `get_state_history()`, but it relies on generic checkpoint savers (Postgres, SQLite). Our `LanceSessionStorage` uses the Lance columnar format for:

- **Efficient versioned storage** — each save creates a new version
- **Time travel** — `get_at_version(session_id, version)` to retrieve any historical state
- **Checkpoint namespacing** — `save_namespaced()` / `get_namespaced()` for multi-tenant isolation
- **Columnar scan performance** — Lance's zero-copy reads for session enumeration
- **Native vector search integration** — can leverage lance-graph's vector indices for semantic session retrieval

```rust
let storage = LanceSessionStorage::new("/data/sessions");
storage.save(session).await?;

// Time travel
let versions = storage.list_versions("thread_1").await?;
let v2_session = storage.get_at_version("thread_1", 2).await?;

// Namespaced
storage.save_namespaced(session, "tenant_a").await?;
let s = storage.get_namespaced("thread_1", "tenant_a").await?;
```

---

## 2. Agent Card YAML System

**Files:** `graph-flow/src/agents/agent_card.rs`, `graph-flow/src/task_registry.rs`

Python LangGraph has no declarative agent definition format. We provide:

- **YAML-based agent cards** — define agents, capabilities, tools, and workflows declaratively
- **TaskRegistry** — bind real task implementations to capability names
- **Capability placeholders** — `CapabilityTask` stubs for capabilities without implementations
- **Workflow compilation** — YAML → executable `Graph` with real tasks

```yaml
agent:
  name: research_agent
  description: Searches and summarizes
  capabilities:
    - search
    - summarize
  tools:
    - name: web_search
      mcp_server: "http://localhost:8080"
  workflow:
    - task: search
      next: summarize
    - task: summarize
      next: end
```

```rust
let mut registry = TaskRegistry::new();
registry.register("search", Arc::new(SearchTask));
registry.register("summarize", Arc::new(SummarizeTask));
let graph = registry.compile_agent_card(yaml)?;
```

---

## 3. LangGraph JSON Import

**File:** `graph-flow/src/agents/langgraph_import.rs`

Direct import of LangGraph workflow definitions from JSON:

```rust
let def = LangGraphDef {
    name: "my_flow".to_string(),
    nodes: vec![...],
    edges: vec![...],
    entry_point: Some("start".to_string()),
};
let graph = import_langgraph_workflow(&def)?;
```

This enables Python → Rust migration by exporting LangGraph definitions as JSON and importing into Rust.

---

## 4. MCP Tool Integration

**File:** `graph-flow/src/mcp_tool.rs`

Native Model Context Protocol (MCP) integration as tasks:

- **`McpToolTask`** — connects to MCP servers and invokes tools
- **`MockMcpToolTask`** — mock for testing without real MCP servers
- **Configurable** — `McpToolConfig` with server URL, tool name, input/output keys, static params, timeout

Python LangGraph relies on LangChain's tool abstraction. Our MCP integration is direct and protocol-native.

```rust
let tool = McpToolTask::new("search", "http://mcp-server:8080", "web_search");
// or with full config
let config = McpToolConfig::new("http://mcp-server:8080", "web_search");
let tool = McpToolTask::with_config("search", config);
```

---

## 5. Thinking Graph

**File:** `graph-flow/src/thinking.rs`

A prebuilt graph for structured "thinking" workflows — not present in Python LangGraph:

```rust
let thinking_graph = build_thinking_graph();
```

Provides a chain-of-thought reasoning structure as a ready-to-use graph.

---

## 6. GoBack Navigation

**Files:** `graph-flow/src/task.rs`, `graph-flow/src/storage.rs`

Python LangGraph has no concept of "going back" in a workflow. Our system provides:

- **`NextAction::GoBack`** — a task can navigate back to its previous task
- **`Session::task_history`** — stack of visited tasks
- **`Session::advance_to()`** — push current to history, move forward
- **`Session::go_back()`** — pop history, return to previous

This enables interactive workflows where users can undo/revisit steps.

```rust
// In a task's run() method:
Ok(TaskResult::new(Some("Going back".to_string()), NextAction::GoBack))

// Session tracks history automatically
session.advance_to("next_task".to_string());
let prev = session.go_back(); // Returns previous task ID
```

---

## 7. `ContinueAndExecute` Flow Control

**File:** `graph-flow/src/task.rs`

Python LangGraph always returns control to the caller between steps (or runs to completion). We have a third option:

- **`NextAction::Continue`** — move to next task, return control to caller (step-by-step)
- **`NextAction::ContinueAndExecute`** — move to next task AND execute it immediately (fire-and-forget chaining)
- **`NextAction::GoTo(task_id)`** — jump to specific task

`ContinueAndExecute` enables task chains that execute without returning intermediate results.

---

## 8. Rig LLM Integration

**File:** `graph-flow/src/context.rs` (feature-gated)

Direct integration with the [Rig](https://github.com/0xPlaygrounds/rig) Rust LLM framework:

- **`Context::get_rig_messages()`** — convert chat history to Rig `Message` format
- **`Context::get_last_rig_messages(n)`** — get last N messages in Rig format

This provides zero-friction LLM calling from within tasks.

```rust
#[cfg(feature = "rig")]
async fn run(&self, ctx: Context) -> Result<TaskResult> {
    let messages = ctx.get_rig_messages().await;
    let response = agent.chat("prompt", messages).await?;
    ctx.add_assistant_message(response).await;
    Ok(TaskResult::new(Some(response), NextAction::Continue))
}
```

---

## 9. FanOut Task (Parallel Execution)

**File:** `graph-flow/src/fanout.rs`

While Python LangGraph has `Send` for dispatching to nodes, our `FanOutTask` provides true parallel task execution:

- Spawns all child tasks concurrently via `tokio::spawn`
- Aggregates results into context with configurable prefixes
- Works as a regular `Task` in the graph

```rust
let fanout = FanOutTask::new("parallel_search", vec![
    Arc::new(WebSearchTask),
    Arc::new(DbSearchTask),
    Arc::new(CacheSearchTask),
]);
```

---

## 10. TypedContext with State Trait

**File:** `graph-flow/src/typed_context.rs`

While Python uses `TypedDict` for state schemas, our `TypedContext<S>` provides:

- **Compile-time type safety** — generic over a `State` trait bound
- **RwLock-protected state** — thread-safe read/write access
- **`update_state()`** — mutable access via closure
- **`snapshot_state()`** — clone current state
- **Dual access** — typed state AND raw Context key-value store

```rust
#[derive(Clone, Serialize, Deserialize)]
struct MyState {
    count: i32,
    name: String,
}
impl State for MyState {}

let typed_ctx = TypedContext::new(MyState { count: 0, name: "init".into() });
typed_ctx.update_state(|s| s.count += 1);
let snapshot = typed_ctx.snapshot_state();
```

---

## 11. ToolResult with Fallback

**File:** `graph-flow/src/tool_result.rs`

Python LangGraph has `ToolMessage` for tool responses. Our `ToolResult` adds:

- **`ToolResult::Fallback { value, reason }`** — graceful degradation with explanation
- **`is_retryable()`** — explicit retry signaling
- **`into_result()`** — convert to standard `Result<Value, String>`

```rust
let result = ToolResult::fallback(
    serde_json::json!({"cached": true}),
    "API unavailable, using cached data"
);
```

---

## 12. Graph-Flow Server (Axum HTTP API)

**File:** `graph-flow-server/src/lib.rs`

While Python LangGraph has `langgraph-api` (a separate deployment platform), our server is:

- **Embeddable** — `create_router()` returns an Axum `Router` you can compose
- **Lightweight** — no separate deployment, just add to your binary
- **LangGraph API compatible** — same REST semantics (threads, runs, state)

```rust
let app = create_router(graph, storage);
let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await?;
axum::serve(listener, app).await?;
```

---

## 13. lance-graph Integration (Separate Repo)

**Repo:** `AdaWorldAPI/lance-graph`

Not part of Python LangGraph at all. Provides:

- **Graph database** with SPO (Subject-Predicate-Object) triple store
- **BLASGraph** — matrix-based graph operations using BLAS
- **Cypher-like query language** with DataFusion backend
- **Vector search** via Lance native indices
- **HDR fingerprinting** for graph similarity
- **Unity Catalog** integration for enterprise data governance

This powers the storage layer and enables semantic search capabilities that Python LangGraph achieves via third-party integrations.

---

## Summary

| Addition | Python LangGraph Has? | Our Advantage |
|----------|----------------------|---------------|
| Lance time-travel storage | No (generic checkpoints) | Columnar + versioned + vector-native |
| Agent Card YAML | No | Declarative agent definitions |
| LangGraph JSON import | No | Python → Rust migration path |
| MCP tool integration | No (uses LangChain tools) | Protocol-native tool calling |
| GoBack navigation | No | Interactive undo/revisit |
| ContinueAndExecute | No (all-or-nothing) | Fine-grained flow control |
| Rig LLM integration | No (uses LangChain) | Rust-native LLM framework |
| FanOut parallel tasks | Partial (Send) | True parallel execution |
| TypedContext<S> | No (TypedDict is runtime) | Compile-time type safety |
| ToolResult::Fallback | No | Graceful degradation |
| Embeddable HTTP server | No (separate platform) | Single-binary deployment |
| lance-graph storage | No | Graph DB + vector search |
| Thinking graph | No | Prebuilt reasoning chain |
