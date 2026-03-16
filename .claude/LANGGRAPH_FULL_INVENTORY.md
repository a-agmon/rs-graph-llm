# LangGraph Full Inventory: Python → Rust Mapping

> Every public class, method, constant, and type in Python LangGraph mapped to its Rust equivalent (or marked MISSING).

---

## 1. `langgraph.constants`

| Python | Rust Equivalent | Location |
|--------|----------------|----------|
| `START = "__start__"` | `pub const START: &str = "__start__"` | `graph-flow/src/compat.rs` |
| `END = "__end__"` | `pub const END: &str = "__end__"` | `graph-flow/src/compat.rs` |
| `TAG_NOSTREAM` | MISSING | — |
| `TAG_HIDDEN` | MISSING | — |

---

## 2. `langgraph.types`

| Python | Rust Equivalent | Location |
|--------|----------------|----------|
| `StreamMode` (Literal: values/updates/debug/messages/custom/events) | `pub enum StreamMode { Values, Updates, Debug }` | `graph-flow/src/streaming.rs` |
| `StreamWriter` (Callable) | `mpsc::Sender<StreamChunk>` | `graph-flow/src/streaming.rs` |
| `RetryPolicy` (NamedTuple) | `pub struct RetryPolicy` | `graph-flow/src/retry.rs` |
| `CachePolicy` | MISSING | — |
| `Interrupt` | MISSING (partial via `NextAction::WaitForInput`) | — |
| `StateSnapshot` (NamedTuple) | MISSING | — |
| `Send` (fanout dispatch) | `pub struct FanOutTask` | `graph-flow/src/fanout.rs` |
| `Command` (Generic) | `pub enum Command` | `graph-flow/src/compat.rs` |
| `Command.goto()` | `Command::goto()` | `graph-flow/src/compat.rs` |
| `Command.resume()` | `Command::resume()` | `graph-flow/src/compat.rs` |
| `Command.update()` | `Command::update()` | `graph-flow/src/compat.rs` |
| `interrupt()` function | MISSING | — |
| `Overwrite` | MISSING | — |
| `PregelTask` | MISSING (internal) | — |
| `PregelExecutableTask` | MISSING (internal) | — |
| `StateUpdate` | MISSING | — |
| `CacheKey` | MISSING | — |
| `CheckpointTask` | MISSING | — |
| `TaskPayload` | MISSING | — |
| `TaskResultPayload` | MISSING | — |
| `CheckpointPayload` | MISSING | — |
| `ValuesStreamPart` | `StreamChunk` (partial) | `graph-flow/src/streaming.rs` |
| `UpdatesStreamPart` | `StreamChunk` (partial) | `graph-flow/src/streaming.rs` |
| `MessagesStreamPart` | `StreamChunk` (partial) | `graph-flow/src/streaming.rs` |
| `CustomStreamPart` | MISSING | — |
| `DebugStreamPart` | `StreamChunk` (partial) | `graph-flow/src/streaming.rs` |
| `GraphOutput` | `ExecutionResult` | `graph-flow/src/graph.rs` |
| `RoutingDecision` (not in Python) | `pub enum RoutingDecision` | `graph-flow/src/compat.rs` |

---

## 3. `langgraph.errors`

| Python | Rust Equivalent | Location |
|--------|----------------|----------|
| `GraphRecursionError` | `RunConfig.recursion_limit` (guarded) | `graph-flow/src/run_config.rs` |
| `InvalidUpdateError` | `GraphError::TaskExecutionFailed` | `graph-flow/src/error.rs` |
| `GraphBubbleUp` | MISSING | — |
| `GraphInterrupt` | `NextAction::WaitForInput` (partial) | `graph-flow/src/task.rs` |
| `NodeInterrupt` | MISSING | — |
| `ParentCommand` | MISSING | — |
| `EmptyInputError` | MISSING | — |
| `TaskNotFound` (exception) | `GraphError::TaskNotFound` | `graph-flow/src/error.rs` |
| `EmptyChannelError` | MISSING | — |
| `ErrorCode` (enum) | MISSING | — |

---

## 4. `langgraph.config`

| Python | Rust Equivalent | Location |
|--------|----------------|----------|
| `get_config()` | MISSING (no LangChain RunnableConfig) | — |
| `get_store()` | MISSING | — |
| `get_stream_writer()` | MISSING | — |

---

## 5. `langgraph.graph.state` — `StateGraph`

| Python Method | Rust Equivalent | Location |
|---------------|----------------|----------|
| `StateGraph.__init__(state_schema)` | `StateGraph::new(name)` | `graph-flow/src/compat.rs` |
| `StateGraph.add_node(name, fn)` | `StateGraph::add_node(name, task)` | `graph-flow/src/compat.rs` |
| `StateGraph.add_edge(from, to)` | `StateGraph::add_edge(from, to)` | `graph-flow/src/compat.rs` |
| `StateGraph.add_conditional_edges(source, path, path_map)` | `StateGraph::add_conditional_edges(source, cond, yes, no)` | `graph-flow/src/compat.rs` |
| `StateGraph.add_sequence(nodes)` | MISSING | — |
| `StateGraph.set_entry_point(key)` | `StateGraph::set_entry_point(node)` | `graph-flow/src/compat.rs` |
| `StateGraph.set_conditional_entry_point(path, path_map)` | MISSING | — |
| `StateGraph.set_finish_point(key)` | MISSING (implicit via END) | — |
| `StateGraph.compile(checkpointer, interrupt_before, interrupt_after)` | `StateGraph::compile()` (no checkpointer arg) | `graph-flow/src/compat.rs` |
| `StateGraph.validate()` | MISSING | — |
| `CompiledStateGraph` (subclass of Pregel) | `Graph` | `graph-flow/src/graph.rs` |
| `CompiledStateGraph.get_input_jsonschema()` | MISSING | — |
| `CompiledStateGraph.get_output_jsonschema()` | MISSING | — |
| `CompiledStateGraph.attach_node()` | MISSING (internal) | — |
| `CompiledStateGraph.attach_edge()` | MISSING (internal) | — |
| `CompiledStateGraph.attach_branch()` | MISSING (internal) | — |

---

## 6. `langgraph.graph.message`

| Python | Rust Equivalent | Location |
|--------|----------------|----------|
| `add_messages()` reducer | `Context::add_user_message()` / `add_assistant_message()` | `graph-flow/src/context.rs` |
| `MessageGraph` | MISSING (use StateGraph with ChatHistory) | — |
| `MessagesState` (TypedDict) | `ChatHistory` | `graph-flow/src/context.rs` |
| `push_message()` | `Context::add_user_message()` | `graph-flow/src/context.rs` |

---

## 7. `langgraph.graph._branch`

| Python | Rust Equivalent | Location |
|--------|----------------|----------|
| `BranchSpec` | `ConditionalEdgeSet` / binary conditional edges | `graph-flow/src/graph.rs` |
| `BranchSpec.run()` | `Graph::find_next_task()` | `graph-flow/src/graph.rs` |

---

## 8. `langgraph.pregel.main` — `Pregel` (Core Engine)

| Python Method | Rust Equivalent | Location |
|---------------|----------------|----------|
| `Pregel.__init__(nodes, channels, ...)` | `GraphBuilder::new().build()` | `graph-flow/src/graph.rs` |
| `Pregel.invoke(input, config)` | `Graph::execute_session()` | `graph-flow/src/graph.rs` |
| `Pregel.stream(input, config, stream_mode)` | `StreamingRunner::stream()` | `graph-flow/src/streaming.rs` |
| `Pregel.astream(input, config, stream_mode)` | `StreamingRunner::stream()` (async native) | `graph-flow/src/streaming.rs` |
| `Pregel.ainvoke(input, config)` | `Graph::execute_session()` (async native) | `graph-flow/src/graph.rs` |
| `Pregel.get_state(config)` | `SessionStorage::get()` | `graph-flow/src/storage.rs` |
| `Pregel.aget_state(config)` | `SessionStorage::get()` (async native) | `graph-flow/src/storage.rs` |
| `Pregel.get_state_history(config)` | `LanceSessionStorage::list_versions()` | `graph-flow/src/lance_storage.rs` |
| `Pregel.update_state(config, values)` | `Context::set()` + `SessionStorage::save()` | `graph-flow/src/context.rs` |
| `Pregel.bulk_update_state(configs, values)` | MISSING | — |
| `Pregel.get_graph()` | MISSING (no Mermaid export) | — |
| `Pregel.get_subgraphs()` | MISSING | — |
| `Pregel.clear_cache(nodes)` | MISSING | — |
| `Pregel.with_config(config)` | `FlowRunner::run_with_config()` | `graph-flow/src/runner.rs` |
| `Pregel.copy()` | MISSING | — |
| `Pregel.config_schema()` | MISSING | — |
| `Pregel.validate()` | MISSING | — |
| `NodeBuilder` | `GraphBuilder` | `graph-flow/src/graph.rs` |
| `NodeBuilder.subscribe_to()` | MISSING (uses edge model) | — |
| `NodeBuilder.write_to()` | MISSING (uses edge model) | — |
| `NodeBuilder.build()` | `GraphBuilder::build()` | `graph-flow/src/graph.rs` |

---

## 9. `langgraph.pregel.protocol` — `PregelProtocol`

| Python | Rust Equivalent | Location |
|--------|----------------|----------|
| `PregelProtocol` (abstract base) | `trait Task` + `Graph` | `graph-flow/src/task.rs` + `graph.rs` |
| `StreamProtocol` | `StreamingRunner` | `graph-flow/src/streaming.rs` |

---

## 10. `langgraph.pregel.remote` — `RemoteGraph`

| Python | Rust Equivalent | Location |
|--------|----------------|----------|
| `RemoteGraph` | `graph-flow-server` (HTTP client not impl) | `graph-flow-server/src/lib.rs` |
| `RemoteException` | MISSING | — |

---

## 11. `langgraph.pregel.types`

| Python | Rust Equivalent | Location |
|--------|----------------|----------|
| `PregelTask` | MISSING (internal) | — |
| `PregelExecutableTask` | MISSING (internal) | — |

---

## 12. `langgraph.channels` — Channel System

| Python Channel | Rust Equivalent | Location |
|---------------|----------------|----------|
| `BaseChannel` (ABC) | `pub enum ChannelReducer` | `graph-flow/src/channels.rs` |
| `BaseChannel.update()` | `Channels::apply()` | `graph-flow/src/channels.rs` |
| `BaseChannel.get()` | `Channels::get()` | `graph-flow/src/channels.rs` |
| `BaseChannel.checkpoint()` | `Channels::snapshot()` | `graph-flow/src/channels.rs` |
| `BaseChannel.from_checkpoint()` | MISSING | — |
| `BaseChannel.is_available()` | MISSING | — |
| `BaseChannel.consume()` | MISSING | — |
| `BaseChannel.finish()` | MISSING | — |
| `LastValue` | `ChannelReducer::LastValue` | `graph-flow/src/channels.rs` |
| `LastValueAfterFinish` | MISSING | — |
| `AnyValue` | MISSING | — |
| `BinaryOperatorAggregate` | `ChannelReducer::Custom(ReducerFn)` | `graph-flow/src/channels.rs` |
| `Topic` (accumulate list) | `ChannelReducer::Append` | `graph-flow/src/channels.rs` |
| `EphemeralValue` | MISSING | — |
| `NamedBarrierValue` | MISSING | — |
| `NamedBarrierValueAfterFinish` | MISSING | — |
| `UntrackedValue` | MISSING | — |

---

## 13. `langgraph.managed` — Managed Values

| Python | Rust Equivalent | Location |
|--------|----------------|----------|
| `ManagedValue` (ABC) | MISSING | — |
| `IsLastStepManager` | MISSING | — |
| `RemainingStepsManager` | MISSING | — |
| `is_managed_value()` | MISSING | — |

---

## 14. `langgraph.func` — Functional API

| Python | Rust Equivalent | Location |
|--------|----------------|----------|
| `@task` decorator | MISSING (use `impl Task` trait) | — |
| `@entrypoint` decorator | MISSING (use `GraphBuilder`) | — |
| `_TaskFunction` | MISSING | — |
| `SyncAsyncFuture` | MISSING (Rust async native) | — |

---

## 15. `langgraph.runtime`

| Python | Rust Equivalent | Location |
|--------|----------------|----------|
| `Runtime` class | MISSING | — |
| `get_runtime()` | MISSING | — |
| `Runtime.merge()` | MISSING | — |
| `Runtime.override()` | MISSING | — |

---

## 16. `langgraph.prebuilt` (separate package)

### 16a. `chat_agent_executor`

| Python | Rust Equivalent | Location |
|--------|----------------|----------|
| `create_react_agent(model, tools, ...)` | `create_react_agent(llm_task, tools, max_iter)` | `graph-flow/src/react_agent.rs` |
| `AgentState` (TypedDict) | Context keys: `needs_tool`, `selected_tool`, etc. | `graph-flow/src/react_agent.rs` |
| `AgentStatePydantic` | MISSING | — |
| `call_model()` / `acall_model()` | Internal `IterationGuardTask` | `graph-flow/src/react_agent.rs` |
| `should_continue()` | Conditional edge on `needs_tool` key | `graph-flow/src/react_agent.rs` |
| `generate_structured_response()` | MISSING | — |
| `route_tool_responses()` | MISSING | — |
| Prompt/system message support | MISSING | — |
| Model selection via Runtime | MISSING | — |
| `post_model_hook` | MISSING | — |

### 16b. `tool_node`

| Python | Rust Equivalent | Location |
|--------|----------------|----------|
| `ToolNode` | `ToolRouterTask` + `ToolAggregatorTask` | `graph-flow/src/react_agent.rs` |
| `tools_condition()` | Conditional edge in `create_react_agent` | `graph-flow/src/react_agent.rs` |
| `ToolCallRequest` | MISSING | — |
| `InjectedState` | MISSING | — |
| `InjectedStore` | MISSING | — |
| `ToolRuntime` | MISSING | — |
| `msg_content_output()` | MISSING | — |
| `ToolInvocationError` | `ToolResult::Error` | `graph-flow/src/tool_result.rs` |
| `ValidationNode` | MISSING | — |

### 16c. `interrupt`

| Python | Rust Equivalent | Location |
|--------|----------------|----------|
| `HumanInterruptConfig` | MISSING | — |
| `HumanInterrupt` | `NextAction::WaitForInput` (basic) | `graph-flow/src/task.rs` |
| `HumanResponse` | MISSING | — |
| `ActionRequest` | MISSING | — |

---

## 17. `langgraph.checkpoint` (separate package)

| Python | Rust Equivalent | Location |
|--------|----------------|----------|
| `BaseCheckpointSaver` | `trait SessionStorage` | `graph-flow/src/storage.rs` |
| `BaseCheckpointSaver.get()` | `SessionStorage::get()` | `graph-flow/src/storage.rs` |
| `BaseCheckpointSaver.put()` | `SessionStorage::save()` | `graph-flow/src/storage.rs` |
| `BaseCheckpointSaver.put_writes()` | MISSING | — |
| `BaseCheckpointSaver.list()` | `LanceSessionStorage::list_versions()` | `graph-flow/src/lance_storage.rs` |
| `BaseCheckpointSaver.delete_thread()` | `SessionStorage::delete()` | `graph-flow/src/storage.rs` |
| `BaseCheckpointSaver.copy_thread()` | MISSING | — |
| `BaseCheckpointSaver.prune()` | MISSING | — |
| `BaseCheckpointSaver.get_next_version()` | MISSING | — |
| `Checkpoint` (TypedDict) | `Session` | `graph-flow/src/storage.rs` |
| `CheckpointMetadata` | MISSING | — |
| `CheckpointTuple` | `VersionedSession` | `graph-flow/src/lance_storage.rs` |
| `copy_checkpoint()` | MISSING | — |
| `empty_checkpoint()` | `Session::new_from_task()` | `graph-flow/src/storage.rs` |
| `MemorySaver` | `InMemorySessionStorage` | `graph-flow/src/storage.rs` |
| `PostgresSaver` | `PostgresSessionStorage` | `graph-flow/src/storage_postgres.rs` |
| `SqliteSaver` | MISSING | — |
| Serde (jsonplus/msgpack) | `serde_json` (JSON only) | — |
| `EncryptedSerde` | MISSING | — |

---

## 18. `langgraph.store` (separate package)

| Python | Rust Equivalent | Location |
|--------|----------------|----------|
| `BaseStore` (ABC) | MISSING | — |
| `BaseStore.get()` | MISSING | — |
| `BaseStore.search()` | MISSING | — |
| `BaseStore.put()` | MISSING | — |
| `BaseStore.delete()` | MISSING | — |
| `BaseStore.list_namespaces()` | MISSING | — |
| `Item` | MISSING | — |
| `SearchItem` | MISSING | — |
| `GetOp` / `SearchOp` / `PutOp` | MISSING | — |
| `ListNamespacesOp` | MISSING | — |
| `MatchCondition` | MISSING | — |
| `AsyncBatchedBaseStore` | MISSING | — |
| `InMemoryStore` | MISSING | — |
| `PostgresStore` | MISSING | — |
| `IndexConfig` / `TTLConfig` | MISSING | — |
| Embeddings support | MISSING (lance-graph has vector search) | — |

---

## 19. `langgraph._internal` (private but important)

| Python | Rust Equivalent | Location |
|--------|----------------|----------|
| `_retry.default_retry_on()` | `RetryPolicy` | `graph-flow/src/retry.rs` |
| `_config.merge_configs()` | MISSING | — |
| `_config.patch_config()` | MISSING | — |
| `_config.ensure_config()` | MISSING | — |
| `_serde.*` (serialization allowlist) | MISSING (not needed in Rust) | — |
| `_cache.*` | MISSING | — |
| `_replay.*` | MISSING | — |

---

## 20. `langgraph.pregel` (execution internals)

| Python | Rust Equivalent | Location |
|--------|----------------|----------|
| `_algo.py` (superstep algorithm) | `Graph::execute_session()` loop | `graph-flow/src/graph.rs` |
| `_loop.py` (execution loop) | `FlowRunner::run()` | `graph-flow/src/runner.rs` |
| `_io.py` (input/output mapping) | `SubgraphTask` mappings | `graph-flow/src/subgraph.rs` |
| `_checkpoint.py` | `SessionStorage::save()/get()` | `graph-flow/src/storage.rs` |
| `_retry.py` (retry logic) | `RetryPolicy` | `graph-flow/src/retry.rs` |
| `_read.py` (channel reads) | `Context::get()` | `graph-flow/src/context.rs` |
| `_write.py` (channel writes) | `Context::set()` | `graph-flow/src/context.rs` |
| `_validate.py` | MISSING | — |
| `_draw.py` (Mermaid diagrams) | MISSING | — |
| `_messages.py` (message handling) | `ChatHistory` | `graph-flow/src/context.rs` |
| `_executor.py` (task execution) | `Graph::execute()` | `graph-flow/src/graph.rs` |
| `_runner.py` (step runner) | `FlowRunner` | `graph-flow/src/runner.rs` |
| `debug.py` (debug utilities) | `StreamMode::Debug` | `graph-flow/src/streaming.rs` |

---

## Summary Statistics

| Category | Python Count | Rust Implemented | Rust Missing | Coverage |
|----------|-------------|-----------------|-------------|----------|
| Constants | 4 | 2 | 2 | 50% |
| Core Types | 20 | 8 | 12 | 40% |
| Errors | 10 | 3 | 7 | 30% |
| StateGraph API | 12 | 7 | 5 | 58% |
| Pregel/Engine | 20 | 10 | 10 | 50% |
| Channels | 10 | 4 | 6 | 40% |
| Managed Values | 4 | 0 | 4 | 0% |
| Functional API | 3 | 0 | 3 | 0% |
| Runtime | 4 | 0 | 4 | 0% |
| Prebuilt/ReAct | 15 | 4 | 11 | 27% |
| Checkpoint | 15 | 8 | 7 | 53% |
| Store | 15 | 0 | 15 | 0% |
| **TOTAL** | **132** | **46** | **86** | **35%** |
