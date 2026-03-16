# LangGraph Transcoding Map: Python → Rust

> Direct type-for-type, function-for-function mapping between Python LangGraph and Rust graph-flow.

---

## Type Mappings

| Python Type | Rust Type | Notes |
|------------|----------|-------|
| `str` | `String` / `&str` | |
| `dict[str, Any]` | `serde_json::Map<String, Value>` | Or typed struct |
| `Any` | `serde_json::Value` | Generic storage |
| `list[T]` | `Vec<T>` | |
| `set[T]` | `HashSet<T>` | |
| `tuple[T, ...]` | `(T, ...)` | |
| `Optional[T]` | `Option<T>` | |
| `Callable[[A], R]` | `Arc<dyn Fn(A) -> R + Send + Sync>` | Thread-safe closure |
| `Callable[[A], Awaitable[R]]` | `Arc<dyn Fn(A) -> Pin<Box<dyn Future<Output=R>>> + Send + Sync>` | Async closure |
| `TypedDict` | `#[derive(Serialize, Deserialize)] struct` | |
| `NamedTuple` | `struct` | |
| `Enum` | `enum` | |
| `Literal["a", "b"]` | `enum { A, B }` | |
| `Generic[T]` | `<T>` | |
| `ABC` (abstract base) | `trait` | |
| `Protocol` | `trait` | |
| `TypeVar` | Generic parameter `T` | |
| `TypeAlias` | `type Alias = ...` | |
| `dataclass` | `struct` + derives | |
| `BaseModel` (Pydantic) | `#[derive(Serialize, Deserialize)] struct` | |
| `RunnableConfig` | `RunConfig` | |
| `BaseMessage` | `SerializableMessage` | |
| `ToolCall` | `serde_json::Value` (or typed struct) | |
| `ToolMessage` | `ToolResult` | |

---

## Core Class Mappings

### StateGraph

```python
# Python
from langgraph.graph import StateGraph, START, END

class State(TypedDict):
    messages: Annotated[list[BaseMessage], add_messages]
    count: int

graph = StateGraph(State)
graph.add_node("agent", agent_fn)
graph.add_node("tools", tool_fn)
graph.add_edge(START, "agent")
graph.add_conditional_edges("agent", should_continue, {"continue": "tools", "end": END})
graph.add_edge("tools", "agent")
compiled = graph.compile(checkpointer=MemorySaver())
```

```rust
// Rust
use graph_flow::compat::{StateGraph, START, END};
use graph_flow::{Task, TaskResult, NextAction, Context};
use std::sync::Arc;
use std::collections::HashMap;

let mut sg = StateGraph::new("my_graph");
sg.add_node("agent", Arc::new(AgentTask));
sg.add_node("tools", Arc::new(ToolsTask));
sg.add_edge(START, "agent");
sg.add_conditional_edges(
    "agent",
    |ctx: &Context| ctx.get_sync::<String>("route").unwrap_or_default(),
    HashMap::from([
        ("continue".to_string(), "tools".to_string()),
        ("end".to_string(), END.to_string()),
    ]),
);
sg.add_edge("tools", "agent");
let graph = sg.compile();
```

### GraphBuilder (Rust-native API)

```rust
use graph_flow::{GraphBuilder, Graph};

let graph = GraphBuilder::new("my_graph")
    .add_task(Arc::new(AgentTask))
    .add_task(Arc::new(ToolsTask))
    .add_edge("agent", "tools")
    .add_conditional_edge("agent", |ctx| ctx.get_sync::<bool>("needs_tool").unwrap_or(false), "tools", "done")
    .set_start_task("agent")
    .build();
```

### Task (Node)

```python
# Python
def my_node(state: State) -> dict:
    return {"messages": [response], "count": state["count"] + 1}

# Or with config
def my_node(state: State, config: RunnableConfig) -> dict:
    return {"messages": [response]}
```

```rust
// Rust
use graph_flow::{Task, TaskResult, NextAction, Context};
use async_trait::async_trait;

struct MyNode;

#[async_trait]
impl Task for MyNode {
    fn id(&self) -> &str { "my_node" }

    async fn run(&self, ctx: Context) -> graph_flow::Result<TaskResult> {
        let count: i32 = ctx.get("count").await.unwrap_or(0);
        ctx.set("count", count + 1).await;
        ctx.add_assistant_message("response".to_string()).await;
        Ok(TaskResult::new(Some("done".to_string()), NextAction::Continue))
    }
}
```

### Execution

```python
# Python
result = compiled.invoke({"messages": [("user", "hello")]})
# or
async for chunk in compiled.astream({"messages": [("user", "hello")]}, stream_mode="updates"):
    print(chunk)
```

```rust
// Rust — Step by step
let mut session = Session::new_from_task("thread_1".to_string(), "agent");
session.context.set("input", "hello").await;
let result = graph.execute_session(&mut session).await?;

// Rust — Run to completion
let runner = FlowRunner::new(Arc::new(graph), storage.clone());
let result = runner.run("thread_1").await?;

// Rust — Streaming
let streaming = StreamingRunner::new(Arc::new(graph), storage.clone());
let mut stream = streaming.stream("thread_1").await?;
while let Some(chunk) = stream.next().await {
    println!("{:?}", chunk?);
}
```

### Checkpointing

```python
# Python
from langgraph.checkpoint.memory import MemorySaver
checkpointer = MemorySaver()
compiled = graph.compile(checkpointer=checkpointer)

# Get state
state = compiled.get_state({"configurable": {"thread_id": "1"}})

# Time travel
for state in compiled.get_state_history({"configurable": {"thread_id": "1"}}):
    print(state)
```

```rust
// Rust
use graph_flow::{InMemorySessionStorage, LanceSessionStorage, SessionStorage};

// In-memory
let storage = Arc::new(InMemorySessionStorage::new());

// Lance-backed (time-travel)
let storage = Arc::new(LanceSessionStorage::new("/data/sessions"));

// Get state
let session = storage.get("thread_1").await?.unwrap();

// Time travel
let versions = storage.list_versions("thread_1").await?;
let old_session = storage.get_at_version("thread_1", versions[0]).await?;
```

### Command

```python
# Python
from langgraph.types import Command, interrupt

def my_node(state):
    answer = interrupt("What should I do?")
    return Command(goto="next", update={"answer": answer})
```

```rust
// Rust
use graph_flow::compat::Command;
use graph_flow::{NextAction, TaskResult};

// GoTo
Ok(TaskResult::new(Some("done".to_string()), NextAction::GoTo("next".to_string())))

// WaitForInput (interrupt equivalent)
Ok(TaskResult::new(Some("What should I do?".to_string()), NextAction::WaitForInput))

// Command enum (for programmatic use)
let cmd = Command::goto("next");
let cmd = Command::update(serde_json::json!({"answer": "yes"}));
let cmd = Command::resume(serde_json::json!("user input"));
```

### Channels / Reducers

```python
# Python
from typing import Annotated
from langgraph.graph import add_messages

class State(TypedDict):
    messages: Annotated[list[BaseMessage], add_messages]  # Append reducer
    count: Annotated[int, lambda a, b: a + b]             # Custom reducer
    name: str                                               # LastValue (default)
```

```rust
// Rust
use graph_flow::{Channels, ChannelReducer, ChannelConfig};

let mut channels = Channels::new();
channels.register("messages", ChannelReducer::Append);
channels.register("count", ChannelReducer::Custom(Arc::new(|a, b| {
    let a = a.as_i64().unwrap_or(0);
    let b = b.as_i64().unwrap_or(0);
    serde_json::json!(a + b)
})));
channels.register("name", ChannelReducer::LastValue);
```

### Subgraphs

```python
# Python
inner = StateGraph(InnerState)
inner.add_node("process", process_fn)
inner.add_edge(START, "process")
inner_compiled = inner.compile()

outer = StateGraph(OuterState)
outer.add_node("sub", inner_compiled)  # Subgraph as node
outer.add_edge(START, "sub")
```

```rust
// Rust
use graph_flow::subgraph::SubgraphTask;

let inner_graph = Arc::new(
    GraphBuilder::new("inner")
        .add_task(Arc::new(ProcessTask))
        .build()
);

let subgraph = SubgraphTask::new("sub", inner_graph);

let outer_graph = GraphBuilder::new("outer")
    .add_task(subgraph)
    .set_start_task("sub")
    .build();
```

### ReAct Agent

```python
# Python
from langgraph.prebuilt import create_react_agent

agent = create_react_agent(
    model=ChatOpenAI(model="gpt-4"),
    tools=[search_tool, calculator_tool],
    prompt="You are a helpful assistant"
)
result = agent.invoke({"messages": [("user", "What is 2+2?")]})
```

```rust
// Rust
use graph_flow::create_react_agent;

let agent_graph = create_react_agent(
    Arc::new(LlmTask::new("gpt-4")),
    vec![
        Arc::new(SearchTool) as Arc<dyn Task>,
        Arc::new(CalculatorTool) as Arc<dyn Task>,
    ],
    10, // max iterations
);

let mut session = Session::new_from_task("thread".to_string(), "llm");
session.context.add_user_message("What is 2+2?".to_string()).await;
let result = agent_graph.execute_session(&mut session).await?;
```

### RetryPolicy

```python
# Python
from langgraph.types import RetryPolicy

policy = RetryPolicy(
    initial_interval=1.0,
    max_interval=10.0,
    backoff_multiplier=2.0,
    max_attempts=3
)
```

```rust
// Rust
use graph_flow::{RetryPolicy, BackoffStrategy};
use std::time::Duration;

let policy = RetryPolicy::exponential(
    3,                             // max_retries
    Duration::from_secs(1),        // base delay
    Duration::from_secs(10),       // max delay
);

// Or fixed backoff
let policy = RetryPolicy::fixed(3, Duration::from_secs(2));
```

### HTTP API

```python
# Python (LangGraph Platform client)
from langgraph_sdk import get_client

client = get_client(url="http://localhost:8123")
thread = client.threads.create()
run = client.runs.create(thread["thread_id"], assistant_id="agent", input={"messages": [...]})
state = client.threads.get_state(thread["thread_id"])
```

```rust
// Rust (server side)
use graph_flow_server::create_router;

let app = create_router(graph, storage);
let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await?;
axum::serve(listener, app).await?;

// Client side (HTTP requests)
// POST /threads          → CreateThreadRequest { start_task, context }
// POST /threads/{id}/runs → (no body) → RunResponse
// GET  /threads/{id}/state → StateResponse
// DELETE /threads/{id}    → 204 No Content
```

---

## Error Mapping

| Python Exception | Rust Error | Pattern |
|-----------------|-----------|---------|
| `GraphRecursionError` | `GraphError::TaskExecutionFailed("exceeded recursion limit")` | Via RunConfig |
| `InvalidUpdateError` | `GraphError::TaskExecutionFailed(msg)` | |
| `GraphInterrupt` | `NextAction::WaitForInput` + `ExecutionStatus::WaitingForInput` | |
| `NodeInterrupt` | MISSING — use `NextAction::WaitForInput` with context data | |
| `TaskNotFound` | `GraphError::TaskNotFound(id)` | |
| `EmptyChannelError` | `GraphError::ContextError(msg)` | |
| Generic errors | `GraphError::Other(anyhow::Error)` | Via `#[from]` |

---

## Idiom Mapping

| Python Pattern | Rust Pattern |
|---------------|-------------|
| `state["key"]` | `ctx.get::<T>("key").await` |
| `return {"key": value}` | `ctx.set("key", value).await` |
| `config["configurable"]["thread_id"]` | `session.id` |
| `@tool` decorator | `impl Task for MyTool` |
| `yield` in generator | `sender.send(StreamChunk{...}).await` |
| `async for chunk in stream` | `while let Some(chunk) = stream.next().await` |
| `TypedDict` with `Annotated[T, reducer]` | `Channels::register(key, ChannelReducer)` |
| `Runnable.with_config()` | `FlowRunner::run_with_config(id, &config)` |
| `MemorySaver()` | `InMemorySessionStorage::new()` |
| `interrupt("question")` | `NextAction::WaitForInput` + response in context |
