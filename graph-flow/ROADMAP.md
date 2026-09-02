# graph-flow API Roadmap

This document tracked the **planned breaking changes** identified during the
engine code review (2026-07). The review's non-breaking fixes shipped in
`0.5.2`; **all ten breaking items below shipped in `0.6.0`**. A migration
guide follows the item list.

Status legend: ✅ = shipped in 0.6.0.

## 1. ✅ Remove `Graph::execute`

`Graph::execute` treated `NextAction` inconsistently with `execute_session`
(`Continue` recursed, `ContinueAndExecute` did nothing). Deprecated in 0.5.2,
removed in 0.6.0. Use `Graph::execute_session` or `FlowRunner::run`.

## 2. ✅ `Context::set` returns `Result`

`set` no longer panics on unserializable values; it returns
`Err(GraphError::ContextError)`. The 0.5.2 `try_set` / `try_set_sync`
stopgaps are removed.

## 3. ✅ `Context` methods are synchronous

The context is an in-memory map guarded by short critical sections - nothing
ever awaited. All `Context` methods (data and chat history) are now sync, and
the redundant `get_sync` / `set_sync` duplicates are gone. Context calls work
identically in async tasks and edge-condition closures.

## 4. ✅ Graph construction reports failure

`GraphBuilder::build()` returns `Result<Graph>`: edges referencing unknown
tasks fail with `GraphError::InvalidEdge`, and an unknown `set_start_task`
target fails with `GraphError::TaskNotFound` (both were silent or
warning-only before). `NextAction::GoTo` to a missing task remains a runtime
error since the target is a runtime string.

## 5. ✅ Remove `ExecutionStatus::Error`

The engine reports failures via `Err(GraphError)`; the never-constructed
`Error(String)` variant is gone, so matches on `ExecutionStatus` no longer
need a dead arm.

## 6. ✅ Remove `NextAction::GoBack`

Never implemented (behaved like `WaitForInput`). Deprecated in 0.5.2, removed
in 0.6.0. Note: `NextAction` is serializable - persisted `TaskResult` values
containing `GoBack` will fail to deserialize after upgrading.

## 7. ✅ `Graph` is immutable after `build()`

`GraphBuilder` now owns all graph data (plain `HashMap`s, consuming builder
methods) and `build()` produces an immutable `Graph` with no interior
mutability - no `DashMap`, no `Mutex`, no lock poisoning, no per-lookup lock
overhead. The mutating methods (`add_task`, `add_edge`,
`add_conditional_edge`, `set_start_task`) exist only on the builder.
`Graph::start_task_id()` now returns `Option<&str>`.

## 8. ✅ `ChatHistory` ring buffer

Messages are stored in a `VecDeque`: at the message cap, appending drops the
oldest message in O(1) instead of shifting the whole buffer.
`ChatHistory::messages()` and `last_messages(n)` now return iterators instead
of slices. The serialized JSON format is unchanged (still an array), so
persisted 0.5 sessions load fine.

## 9. ✅ Optimistic locking for session storage

`Session` has a `version` field (serde-defaulted to 0 for pre-0.6 rows), and
both storage backends enforce it: a save whose version doesn't match the
stored session fails with the new `GraphError::SessionConflict` instead of
silently overwriting newer data. `FlowRunner::run` therefore fails loudly if
two runs of the same session race; retry the losing call to re-run against
the fresh state.

## 10. ✅ Session ids are free-form strings in Postgres

The `sessions.id` column is now `TEXT` instead of `UUID`, so human-readable
session ids (e.g. `"session_001"`) work with `PostgresSessionStorage` just
like with the in-memory backend. The migration converts existing tables in
place. `Session::with_graph_id` replaces hand-writing struct literals when a
session belongs to a non-default graph.

---

# Migrating from 0.6 to 0.7

`0.7.0` carries one breaking change, and it is entirely about the `rig` feature:
the feature now builds against **`rig-core` 0.42** instead of 0.35. Nothing in
graph-flow's own API changed — but `get_rig_messages()` / `get_last_rig_messages()`
return `Vec<rig_core::completion::Message>`, so that type's identity moved with it.
Crates that don't enable `rig` are unaffected.

## Pin rig-core 0.42 in your own crate

A `rig-core 0.35` requirement of your own would link a *second* rig-core and make
`get_rig_messages()` fail to typecheck against it ("two types with similar names").

```toml
# 0.6
graph-flow = { version = "0.6", features = ["rig"] }
rig-core = "0.35"

# 0.7
graph-flow = { version = "0.7", features = ["rig"] }
rig-core = "0.42"
rig-agent = "0.42"   # only if you build agents; see below
```

## The agent runtime moved to `rig-agent`

rig 0.42 split the classic runtime out of `rig-core`. graph-flow depends on
`rig-core` alone (it only needs `Message`); add `rig-agent` yourself for agents.

```rust
// 0.6
use rig::completion::Chat;
use rig::providers::openrouter;

// 0.7
use rig_agent::prelude::*;          // Chat, Prompt, AgentClientExt, Message
use rig_core::providers::openrouter;
```

The crate's lib name also changed: `rig::` paths become `rig_core::`.

## `Chat::chat` borrows the history mutably

It now takes `&mut Vec<Message>` and appends the turn's committed messages to it,
instead of consuming any `IntoIterator`.

```rust
// 0.6
let history = context.get_rig_messages();
let response = agent.chat(&user_input, history).await?;

// 0.7
let mut history = context.get_rig_messages();
let response = agent.chat(&user_input, &mut history).await?;
```

---

# Migrating from 0.5 to 0.6

## Context calls: drop `.await`, handle `Result` on `set`

```rust
// 0.5
let name: String = context.get("name").await.unwrap_or_default();
let flag = ctx.get_sync::<bool>("flag").unwrap_or(false);
context.set("greeting", greeting.clone()).await;
context.add_user_message(input).await;
let history = context.get_rig_messages().await;

// 0.6
let name: String = context.get("name").unwrap_or_default();
let flag = ctx.get::<bool>("flag").unwrap_or(false);
context.set("greeting", greeting.clone())?;   // now fallible
context.add_user_message(input);
let history = context.get_rig_messages();
```

In functions that don't return `Result`, handle `set` explicitly
(`.expect(...)` or `let _ = ...` if the value is known-serializable).

## Graph building: `build()` returns `Result<Graph>`

```rust
// 0.5
let graph = GraphBuilder::new("wf").add_task(t).build();

// 0.6
let graph = GraphBuilder::new("wf").add_task(t).build()?;
```

## `ExecutionStatus` matches: delete the `Error` arm

```rust
match result.status {
    ExecutionStatus::Completed => { ... }
    ExecutionStatus::Paused { .. } => { ... }
    ExecutionStatus::WaitingForInput => { ... }
    // ExecutionStatus::Error(e) arm no longer exists - errors arrive as Err(GraphError)
}
```

## Session literals: use the constructor (new `version` field)

```rust
// 0.5
let session = Session { id, graph_id, current_task_id, status_message: None, context };

// 0.6
let mut session = Session::new_from_task(id, &task_id).with_graph_id(graph_id);
session.context = context; // if you built a custom context
```

## Handle `GraphError::SessionConflict`

If your service can run the same session concurrently, treat
`SessionConflict` from `save` / `FlowRunner::run` as "reload and retry".
Single-writer services will never see it.

## Removed items and their replacements

| Removed | Use instead |
|---|---|
| `Graph::execute` | `Graph::execute_session` / `FlowRunner::run` |
| `NextAction::GoBack` | `NextAction::GoTo(task_id)` |
| `ExecutionStatus::Error` | `Err(GraphError)` from the execution call |
| `Context::get_sync` / `set_sync` | `Context::get` / `set` (now sync) |
| `Context::try_set` / `try_set_sync` | `Context::set` (now fallible) |
| `Graph::add_task` / `add_edge` / `add_conditional_edge` / `set_start_task` | the same methods on `GraphBuilder` |
| `ChatHistory::messages()` slice indexing | iterate (`.messages()`, `.last_messages(n)`) or collect |
