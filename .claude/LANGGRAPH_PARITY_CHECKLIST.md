# LangGraph Parity Checklist

> What exists, what's missing, priority ranking for each gap.

Legend:
- **DONE** = Implemented and tested in Rust
- **PARTIAL** = Exists but incomplete vs Python equivalent
- **MISSING** = Not implemented
- Priority: **P0** (critical), **P1** (important), **P2** (nice-to-have), **P3** (low/skip)

---

## Core Graph Construction

| Feature | Status | Priority | Notes |
|---------|--------|----------|-------|
| `GraphBuilder` (fluent API) | DONE | — | Fully working |
| `StateGraph` (LangGraph compat) | DONE | — | `compat.rs` |
| `START` / `END` constants | DONE | — | `compat.rs` |
| `add_node()` | DONE | — | Via `add_task()` |
| `add_edge()` | DONE | — | Direct + conditional |
| Binary conditional edges | DONE | — | `add_conditional_edge(from, cond, yes, no)` |
| N-way conditional edges (`path_map`) | DONE | — | `add_conditional_edges(from, path_fn, path_map)` |
| `add_sequence()` (ordered chain) | MISSING | P2 | Sugar — easy to add |
| `set_entry_point()` | DONE | — | `set_start_task()` |
| `set_conditional_entry_point()` | MISSING | P2 | Conditional start routing |
| `set_finish_point()` | MISSING | P3 | Implicit via `NextAction::End` |
| `compile()` | DONE | — | `build()` / `StateGraph::compile()` |
| `validate()` (graph validation) | MISSING | P1 | Detect orphan nodes, cycles, unreachable |

## Graph Execution

| Feature | Status | Priority | Notes |
|---------|--------|----------|-------|
| Step-by-step execution | DONE | — | `execute_session()` |
| Run to completion | DONE | — | `FlowRunner::run()` loop |
| Async execution | DONE | — | Native async/await |
| Task timeout | DONE | — | `Graph.task_timeout` |
| Recursion limit | DONE | — | `RunConfig.recursion_limit` |
| Breakpoints (interrupt_before) | DONE | — | `BreakpointConfig` |
| Breakpoints (interrupt_after) | DONE | — | `BreakpointConfig` |
| Dynamic breakpoints | DONE | — | Via `RunConfig` |
| Batch execution | DONE | — | `FlowRunner::run_batch()` |
| `invoke()` equivalent | DONE | — | `execute_session()` |
| `stream()` equivalent | DONE | — | `StreamingRunner::stream()` |
| Stream modes (values/updates/debug) | DONE | — | `StreamMode` enum |
| Stream mode: messages | PARTIAL | P1 | Basic chat history streaming |
| Stream mode: custom | MISSING | P2 | Custom stream channels |
| Stream mode: events | MISSING | P2 | LangSmith-style events |
| `ainvoke()` / `astream()` | DONE | — | All Rust is async-native |
| Tags / metadata on runs | DONE | — | `RunConfig` |

## State Management

| Feature | Status | Priority | Notes |
|---------|--------|----------|-------|
| Key-value context | DONE | — | `Context` with DashMap |
| Typed state (`TypedContext<S>`) | DONE | — | Generic state struct |
| Chat history | DONE | — | `ChatHistory` in Context |
| `add_messages` reducer | PARTIAL | P1 | Manual add, no dedup/update by ID |
| Context serialization | DONE | — | `Context::serialize()` |
| Sync + async access | DONE | — | `get_sync()` / `set_sync()` + async |
| State snapshots | MISSING | P1 | `StateSnapshot` equivalent |
| State history / time travel | PARTIAL | P1 | `LanceSessionStorage::list_versions()` |

## Channels

| Feature | Status | Priority | Notes |
|---------|--------|----------|-------|
| `LastValue` channel | DONE | — | `ChannelReducer::LastValue` |
| `Topic` (append list) | DONE | — | `ChannelReducer::Append` |
| `BinaryOperatorAggregate` | DONE | — | `ChannelReducer::Custom(fn)` |
| `AnyValue` channel | MISSING | P3 | Rarely used |
| `EphemeralValue` channel | MISSING | P2 | Useful for one-shot data |
| `NamedBarrierValue` | MISSING | P3 | Synchronization primitive |
| `UntrackedValue` | MISSING | P3 | Rarely used |
| Channel checkpoint/restore | MISSING | P2 | From checkpoint support |
| Channel `is_available()` | MISSING | P3 | Availability tracking |
| Channel `consume()` / `finish()` | MISSING | P3 | Lifecycle methods |

## Checkpointing / Storage

| Feature | Status | Priority | Notes |
|---------|--------|----------|-------|
| `SessionStorage` trait | DONE | — | `save()` / `get()` / `delete()` |
| In-memory storage | DONE | — | `InMemorySessionStorage` |
| PostgreSQL storage | DONE | — | `PostgresSessionStorage` |
| Lance-backed storage | DONE | — | `LanceSessionStorage` |
| Version history | DONE | — | `list_versions()` / `get_at_version()` |
| Checkpoint namespacing | DONE | — | `save_namespaced()` / `get_namespaced()` |
| `put_writes()` (partial writes) | MISSING | P2 | Write individual channels |
| `copy_thread()` | MISSING | P2 | Clone session state |
| `prune()` (cleanup old) | MISSING | P2 | Storage cleanup |
| `delete_for_runs()` | MISSING | P3 | Selective deletion |
| `CheckpointMetadata` | MISSING | P2 | Rich metadata per checkpoint |
| `get_next_version()` | MISSING | P3 | Auto-version numbering |
| Serde: msgpack | MISSING | P3 | JSON is sufficient |
| Serde: encrypted | MISSING | P2 | Sensitive data at rest |
| SQLite storage | MISSING | P3 | Postgres + Lance cover needs |

## Subgraphs

| Feature | Status | Priority | Notes |
|---------|--------|----------|-------|
| `SubgraphTask` | DONE | — | Inner graph execution |
| Shared context | DONE | — | Parent/child share Context |
| Input/output mappings | DONE | — | `with_mappings()` |
| Max iteration guard | DONE | — | 1000 iteration limit |
| `get_subgraphs()` introspection | MISSING | P2 | Enumerate child graphs |

## Prebuilt Agents

| Feature | Status | Priority | Notes |
|---------|--------|----------|-------|
| `create_react_agent()` | DONE | — | With iteration guard |
| Tool routing | DONE | — | `ToolRouterTask` |
| Tool aggregation | DONE | — | `ToolAggregatorTask` |
| `ToolNode` (full) | PARTIAL | P1 | Basic routing, no interceptors |
| `tools_condition()` | DONE | — | Conditional edge on `needs_tool` |
| `InjectedState` | MISSING | P1 | Tool state injection |
| `InjectedStore` | MISSING | P2 | Tool store injection |
| `ToolRuntime` | MISSING | P2 | Runtime injection to tools |
| `ToolCallRequest` / interceptors | MISSING | P1 | Request interception pipeline |
| `ValidationNode` | MISSING | P2 | Schema validation for tool calls |
| Prompt / system message | MISSING | P1 | System prompt in ReAct agent |
| Model selection (multi-model) | MISSING | P1 | Dynamic model per-call |
| `generate_structured_response()` | MISSING | P2 | Structured output mode |
| `HumanInterrupt` config | MISSING | P1 | Structured human-in-the-loop |
| `HumanResponse` | MISSING | P1 | Response format |
| `post_model_hook` | MISSING | P2 | Post-inference processing |

## Agent Cards / YAML

| Feature | Status | Priority | Notes |
|---------|--------|----------|-------|
| Agent card YAML schema | DONE | — | `AgentCard` struct |
| `compile_agent_card()` | DONE | — | YAML → Graph |
| `TaskRegistry` | DONE | — | Real task bindings |
| Capability placeholders | DONE | — | `CapabilityTask` fallback |
| LangGraph JSON import | DONE | — | `import_langgraph_workflow()` |

## Error Handling

| Feature | Status | Priority | Notes |
|---------|--------|----------|-------|
| `GraphError` enum | DONE | — | 7 variants |
| `ToolResult` (success/error/fallback) | DONE | — | Structured results |
| Retry policy | DONE | — | Fixed/Exponential/None |
| `GraphRecursionError` | PARTIAL | P1 | Limit exists, no dedicated error |
| `NodeInterrupt` | MISSING | P1 | Per-node interrupt |
| `GraphInterrupt` | PARTIAL | P1 | Via WaitForInput |
| `InvalidUpdateError` | MISSING | P3 | Via TaskExecutionFailed |
| Error codes | MISSING | P3 | Enum-based codes |

## Store (Long-term Memory)

| Feature | Status | Priority | Notes |
|---------|--------|----------|-------|
| `BaseStore` trait | MISSING | P0 | Critical for agent memory |
| `InMemoryStore` | MISSING | P0 | Dev/test store |
| `Item` / `SearchItem` | MISSING | P0 | Store data types |
| `get()` / `put()` / `delete()` | MISSING | P0 | CRUD operations |
| `search()` with embeddings | MISSING | P0 | Vector search (lance-graph!) |
| `list_namespaces()` | MISSING | P1 | Namespace enumeration |
| `MatchCondition` | MISSING | P1 | Filter predicates |
| `IndexConfig` / `TTLConfig` | MISSING | P2 | Index + expiry config |
| `AsyncBatchedBaseStore` | MISSING | P2 | Batched async operations |
| `PostgresStore` | MISSING | P1 | Persistent store |

## Functional API

| Feature | Status | Priority | Notes |
|---------|--------|----------|-------|
| `@task` decorator | MISSING | P2 | Macro could work (`#[task]`) |
| `@entrypoint` decorator | MISSING | P2 | Macro for graph entry |
| `SyncAsyncFuture` | N/A | — | Rust is natively async |

## Runtime

| Feature | Status | Priority | Notes |
|---------|--------|----------|-------|
| `Runtime` class | MISSING | P2 | Runtime context injection |
| `get_runtime()` | MISSING | P2 | Access current runtime |
| `Runtime.override()` | MISSING | P2 | Override runtime values |

## HTTP API / Server

| Feature | Status | Priority | Notes |
|---------|--------|----------|-------|
| Thread creation (POST) | DONE | — | `/threads` |
| Thread execution (POST) | DONE | — | `/threads/{id}/runs` |
| Thread state (GET) | DONE | — | `/threads/{id}/state` |
| Thread deletion (DELETE) | DONE | — | `/threads/{id}` |
| Thread history (GET) | MISSING | P1 | `/threads/{id}/history` |
| SSE streaming endpoint | MISSING | P1 | Server-sent events |
| Cron runs | MISSING | P3 | Scheduled execution |
| Assistants CRUD | MISSING | P2 | Multi-graph management |

## Visualization / Debugging

| Feature | Status | Priority | Notes |
|---------|--------|----------|-------|
| Mermaid diagram export | MISSING | P2 | `get_graph()` → Mermaid |
| Debug stream mode | DONE | — | `StreamMode::Debug` |
| Task history tracking | DONE | — | `Session::task_history` |

---

## Priority Summary

| Priority | Count | Description |
|----------|-------|-------------|
| **P0** | 5 | Store/memory system (critical for agents) |
| **P1** | 18 | Core parity gaps (interrupts, HITL, validation, streaming) |
| **P2** | 22 | Nice-to-have features (functional API, visualization, advanced channels) |
| **P3** | 12 | Low priority (edge cases, rarely used) |

### Recommended Sprint Order

1. **Sprint 1 (P0)**: Store/Memory system — `BaseStore` trait, `InMemoryStore`, `Item`/`SearchItem`, integrate with lance-graph vector search
2. **Sprint 2 (P1-core)**: Structured interrupts (`NodeInterrupt`, `HumanInterrupt`/`HumanResponse`), graph validation, SSE streaming
3. **Sprint 3 (P1-agents)**: Enhanced ReAct agent (prompt support, model selection), `InjectedState`, `ToolCallRequest` interceptors
4. **Sprint 4 (P2)**: Functional API macros, Mermaid export, advanced channels, `Runtime`
