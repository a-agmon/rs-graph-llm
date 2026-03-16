# Crate Structure Decision

## Recommendation: Monorepo (Option A)

### Rationale

The flow engine NEEDS the graph for knowledge access.
Agent patterns NEED both flow and graph.
Separate repos = version coordination hell.
Monorepo = one cargo workspace, one version, one CI.

### Target Structure

```
lance-graph/
├── crates/
│   ├── lance-graph-core/         — graph algebra, semirings, storage
│   ├── lance-graph-flow/         — graph-flow execution engine (from rs-graph-llm)
│   │   ├── src/
│   │   │   ├── task.rs           — Task trait, TaskResult, NextAction
│   │   │   ├── graph.rs          — GraphBuilder, Graph, ExecutionStatus
│   │   │   ├── context.rs        — Context (thread-safe key-value + chat history)
│   │   │   ├── session.rs        — Session, SessionStorage trait
│   │   │   ├── fanout.rs         — FanOutTask (parallel execution)
│   │   │   ├── runner.rs         — FlowRunner (load-execute-save)
│   │   │   ├── streaming.rs      — StreamingTask, StreamChunk, StreamingRunner
│   │   │   ├── typed_context.rs  — TypedContext<S> (generic typed state)
│   │   │   ├── subgraph.rs       — SubgraphTask (hierarchical composition)
│   │   │   ├── mcp_tool.rs       — McpToolTask (MCP protocol integration)
│   │   │   ├── lance_storage.rs  — LanceSessionStorage (time travel)
│   │   │   ├── thinking.rs       — 10-layer thinking orchestration graph
│   │   │   └── agents/
│   │   │       ├── agent_card.rs — Agent Card YAML → GraphBuilder compiler
│   │   │       └── langgraph_import.rs — LangGraph JSON/YAML import
│   │   └── Cargo.toml
│   ├── lance-graph-tools/        — MCP server, external tool bridges
│   └── lance-graph-agents/       — Pre-built agent patterns, YAML registry
├── examples/
│   ├── insurance-claims/
│   ├── recommendation/
│   └── thinking-demo/
└── Cargo.toml
```

### Migration Plan

1. **Phase 1** (current): Develop features in rs-graph-llm (this repo)
2. **Phase 2**: Copy `graph-flow/` crate into `lance-graph/crates/lance-graph-flow/`
3. **Phase 3**: Add cross-crate dependencies (flow → core for graph algebra)
4. **Phase 4**: Move examples into unified workspace
5. **Phase 5**: Deprecate rs-graph-llm, redirect to lance-graph

### Dependencies Between Crates

```
lance-graph-agents → lance-graph-flow → lance-graph-core
                   → lance-graph-tools → lance-graph-core
```

### Feature Flags

```toml
[features]
default = ["mcp"]
rig = ["dep:rig-core"]      # LLM integration via Rig
mcp = ["dep:reqwest"]       # MCP tool calling
lance = ["dep:lance"]       # Lance dataset storage
full = ["rig", "mcp", "lance"]
```
