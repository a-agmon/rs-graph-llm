<div align="center">
  <img src="image.png" alt="graph-flow" width="200">
  
  # graph-flow
  
  ### High-Performance Multi-Agent Workflow Framework
  
  *A type-safe and flexible framework for building complex and interactive AI agent workflows in Rust*
  
  ---
</div>

**graph-flow** is a high-performance, type-safe framework for building multi-agent workflow systems in Rust. This repository showcases real-world implementations and examples demonstrating how graph-flow enables complex, stateful AI agent orchestration for production environments.

## Why graph-flow? Why another frameowrk? 

The beauty of **LangGraph** lies in its elegant combination of two powerful concepts:
- **Graph execution library** - for orchestrating complex, stateful workflows
- **LLM ecosystem integration** (via LangChain) - for seamless AI agent capabilities

This framework follows the same philosophy, but built from the ground up in Rust:
- **[`graph-flow`](graph-flow/)** - Core graph execution library for stateful task orchestration
- **[Rig crate](https://github.com/0xPlaygrounds/rig)** - Rust-native LLM integration and agent capabilities

The result aspires to be a production-ready framework that combines LangGraph's workflow design patterns with Rust's **performance** and **type safety**, in addition to a **clean database schema**, and **flexible execution models** (step-by-step, batch, or mixed) with intuitive human-in-the-loop capabilities.

## What's in This Repository

This repository contains **graph-flow** - the core framework - along with comprehensive examples and production-ready services demonstrating real-world applications:

### 🏗️ **Core Framework**
- **[`graph-flow/`](graph-flow/)** - The complete framework library providing:
  - Graph execution engine with stateful task orchestration
  - Session management with pluggable storage backends
  - Context system for thread-safe state sharing
  - Conditional routing and workflow control
  - Built-in chat history and LLM integration support

### 🏭 **Production Services**
Real-world implementations showing how to build production systems:

- **[`insurance-claims-service/`](insurance-claims-service/)** - Complete insurance workflow with:
  - Multi-step claim processing with conditional routing
  - LLM-driven natural language interactions  
  - Human-in-the-loop approval for high-value claims
  - Business rule validation and automated decision-making

- **[`recommendation-service/`](recommendation-service/)** - RAG-based recommendation system featuring:
  - Vector search integration for semantic matching
  - Multi-step reasoning and context accumulation
  - Structured data extraction from unstructured input

### 📚 **Learning Examples**  
- **[`examples/`](examples/)** - Progressive examples from basic to advanced:
  - `simple_example.rs` - Basic workflow concepts
  - `complex_example.rs` - Conditional routing and branching
  - `recommendation_flow.rs` - Complete RAG workflow demonstration

> **Getting Started**: Begin with `examples/simple_example.rs` to understand core concepts, then explore the production services to see real-world patterns and best practices.

## Core Framework Architecture

**graph-flow** combines the best ideas from LangGraph with Rust's strengths for production systems. You get the same intuitive graph-based workflow model with resumable execution, but with compile-time safety, native performance, and production-ready persistence.

### Key Components

#### 🔧 **Graph Execution Engine**
- **Task Orchestration**: Define and connect tasks in a directed graph
- **Conditional Routing**: Dynamic workflow branching based on runtime data  
- **Execution Control**: Fine-grained control over workflow progression (step-by-step vs batch)
- **Error Handling**: Comprehensive error propagation and recovery

#### 🗃️ **Session Management**  
- **Pluggable Storage**: In-memory for development, PostgreSQL for production
- **State Persistence**: Complete workflow state survives restarts and failures
- **Resume Anywhere**: Workflows can be paused and resumed at any task
- **Query-Friendly Schema**: Direct SQL access to workflow state and history

#### 🧠 **Context System**
- **Thread-Safe State**: Concurrent access to shared workflow data
- **Type-Safe Storage**: Compile-time guarantees for data access
- **Built-in Chat History**: LLM conversation management with full serialization
- **Rig Integration**: Native support for LLM agents and tool calling

## Beyond the Examples: Real Production Use

The examples in this repository demonstrate practical applications of the framework:
- **Complex multi-agent workflows** (insurance claims with conditional approval)
- **RAG-based systems** (recommendation service with vector search)
- **Human-in-the-loop processing** (approval workflows with state persistence)
- **API service integration** (HTTP services wrapping graph execution)

Each service showcases different aspects of building production-ready LLM agent systems with clear separation between business logic (tasks) and orchestration (graph).

## Quick Start Guide

Let's start with the basics using [`examples/simple_example.rs`](examples/simple_example.rs):

### 1. Define Tasks

Tasks are the building blocks of your workflow. Each task implements the [`Task`](graph-flow/src/task.rs) trait:

```rust
use async_trait::async_trait;
use graph_flow::{Context, Task, TaskResult, NextAction};

struct HelloTask;

#[async_trait]
impl Task for HelloTask {
    // The id() method now has a default implementation that uses std::any::type_name::<Self>()
    // You can override it if you need a custom identifier

    async fn run(&self, context: Context) -> graph_flow::Result<TaskResult> {
        // Get input from context
        let name: String = context.get_sync("name").unwrap();
        let greeting = format!("Hello, {}", name);
        
        // Store result for next task
        context.set("greeting", greeting.clone()).await;

        // Control flow: Continue to next task 
        // but give control back to workflow manager, to return response to client
        Ok(TaskResult::new(Some(greeting), NextAction::Continue))
    }
}
```

### 2. Build the Graph

Use [`GraphBuilder`](graph-flow/src/graph.rs) to construct your workflow:

```rust
use graph_flow::{GraphBuilder, InMemorySessionStorage, FlowRunner};
use std::sync::Arc;

// Create task instances
let hello_task = Arc::new(HelloTask);
let excitement_task = Arc::new(ExcitementTask);

// Build the graph
let graph = Arc::new(GraphBuilder::new("greeting_workflow")
    .add_task(hello_task.clone())
    .add_task(excitement_task.clone())
    .add_edge(hello_task.id(), excitement_task.id()) // Connect tasks
    .build());
```

### 3. Execute the Workflow

The framework provides **stateful execution** - workflows can be paused, resumed, and managed across multiple interactions:

```rust
// Create storage and runner
let session_storage = Arc::new(InMemorySessionStorage::new());
let flow_runner = FlowRunner::new(graph.clone(), session_storage.clone());

// Create a session starting from the first task
let session = Session::new_from_task("session_001".to_string(), hello_task.id());
session.context.set("name", "Batman".to_string()).await;
session_storage.save(session).await?;

// Execute step by step - FlowRunner handles load/execute/save automatically
loop {
    let result = flow_runner.run("session_001").await?;
    println!("Response: {:?}", result.response);
    
    match result.status {
        ExecutionStatus::Completed => break,
        ExecutionStatus::WaitingForInput => continue,
        ExecutionStatus::Error(err) => return Err(err),
    }
}
```

Alternatively, you can use the lower-level API for more control:

```rust
// Manual session management
loop {
    let mut session = session_storage.get("session_001").await?.unwrap();
    let result = graph.execute_session(&mut session).await?;
    session_storage.save(session).await?;
    
    match result.status {
        ExecutionStatus::Completed => break,
        ExecutionStatus::WaitingForInput => continue,
        ExecutionStatus::Error(err) => return Err(err),
    }
}
```

## Advanced Features

### Conditional Edges

The [`complex_example.rs`](examples/complex_example.rs) demo shows how you can branch **at runtime** based on data stored in the `Context`.  It classifies the user's input as _positive_ or _negative_ and then follows the matching branch.

#### SentimentAnalysisTask (minimal)

```rust
struct SentimentAnalysisTask;

#[async_trait]
impl Task for SentimentAnalysisTask {
    async fn run(&self, ctx: Context) -> graph_flow::Result<TaskResult> {
        // Very naive heuristic so the example works without an LLM
        let input: String = ctx.get_sync("user_input").unwrap_or_default();
        let sentiment = if input.to_lowercase().contains("good") {
            "positive"
        } else {
            "negative"
        };
        ctx.set("sentiment", sentiment.to_string()).await;
        Ok(TaskResult::new(None, NextAction::Continue))
    }
}
```

#### Building the graph

```rust
let graph = GraphBuilder::new("sentiment_flow")
    .add_task(sentiment_task)          // Detect sentiment
    .add_task(positive_task)           // Reply for happy mood
    .add_task(negative_task)           // Reply for unhappy mood
    .add_conditional_edge(
        sentiment_task.id(),
        |ctx| ctx.get_sync::<String>("sentiment")
                 .map(|s| s == "positive")
                 .unwrap_or(false),
        positive_task.id(),            // yes branch → positive
        negative_task.id(),            // else      → negative
    )
    .build();
```

```mermaid
graph TD
    SA["SentimentAnalysisTask"] --> S{Sentiment?}
    S -->|"positive"| P["PositiveResponseTask"]
    S -->|"negative"| N["NegativeResponseTask"]
```

### LLM Integration with Rig

The framework seamlessly integrates with LLM agents using the [Rig](https://github.com/0xPlaygrounds/rig) crate:

```rust
use rig::{agent::Agent, providers::openrouter};

async fn run(&self, context: Context) -> graph_flow::Result<TaskResult> {
    // Get user input
    let user_input: String = context.get_sync("user_input").unwrap();
    
    // Create LLM agent
    let client = openrouter::Client::new(&api_key);
    let agent = client.agent("openai/gpt-4o-mini")
        .preamble("You are a helpful insurance assistant")
        .build();
    
    // Get chat history for context
    let chat_history = context.get_rig_messages().await;
    
    // Generate response
    let response = agent.chat(&user_input, chat_history).await?;
    
    // Store conversation
    context.add_user_message(user_input).await;
    context.add_assistant_message(response.clone()).await;
    
    Ok(TaskResult::new(Some(response), NextAction::Continue))
}
```

### Chat History Management

Built-in conversation management with full serialization:

```rust
// Add messages to conversation
context.add_user_message("What's my claim status?".to_string()).await;
context.add_assistant_message("Your claim is being processed".to_string()).await;
context.add_system_message("Claim updated".to_string()).await;

// Retrieve conversation history
let history = context.get_chat_history().await;
let recent = context.get_last_messages(5).await;

// Chat history is automatically serialized with session state
let serialized = serde_json::to_string(&context).unwrap();
```

### Storage Abstraction

Pluggable storage backends for production deployment:

```rust
// In-memory storage (development)
let session_storage = Arc::new(InMemorySessionStorage::new());

// PostgreSQL storage (production)
let session_storage = Arc::new(
    PostgresSessionStorage::connect(&database_url).await?
);

// Both implement the same SessionStorage trait
```

## Real-World Use Case: Insurance Claims Processing

The [`insurance-claims-service`](insurance-claims-service/) demonstrates a complete agentic workflow for processing insurance claims. This showcases the framework's power in building complex, multi-step AI-driven processes.

### The Insurance Claims Workflow

```mermaid
graph TD
    A["Initial Claim Query<br/>• Welcome user<br/>• Gather basic info<br/>• LLM conversation"] --> B[" Insurance Type Classifier<br/>• Analyze claim description<br/>• Extract insurance type<br/>"]
    
    B --> C{Insurance Type?}
    C -->|"Car"| D["Car Insurance Details<br/>• Accident details<br/>• Cost estimation"]
    C -->|"Apartment"| E["Apartment Insurance Details<br/>• Property information<br/>• Cost estimation"]
    
    D --> F["Smart Claim Validator<br/>• Apply business rules<br/>• Route for approval"]
    E --> F
    
    F --> G{Claim Amount?}
    G -->|"< $1000"| H["Auto-Approve<br/>• Update context"]
    G -->|"≥ $1000"| I["Manual Review Required<br/>• Wait for human input<br/>• Pause workflow<br/>"]
    
    I --> K{Approval Decision?}
    K -->|"'approved'"| L["Manual Approval<br/>• Mark as approved<br/>• Generate decision<br/>"]
    K -->|"Other response"| M["Final Summary<br/>• Generate comprehensive report<br/>• Complete workflow"]
    
    H --> M
    L --> M
    
    %% Styling
    classDef startEnd fill:#e1f5fe,stroke:#01579b,stroke-width:2px
    classDef process fill:#f3e5f5,stroke:#4a148c,stroke-width:2px
    classDef decision fill:#fff3e0,stroke:#e65100,stroke-width:2px
    classDef approval fill:#e8f5e8,stroke:#1b5e20,stroke-width:2px
    classDef waiting fill:#fff8e1,stroke:#f57f17,stroke-width:2px
    
    class A,M startEnd
    class B,D,E,F process
    class C,G,K decision
    class H,L approval
    class I waiting
```

**Key Features Illustrated:**
- **LLM-Driven Interactions**: Each task uses AI for natural language processing / understanding
- **Conditional Routing**: Dynamic branching based on insurance type and claim amount
- **Human-in-the-Loop**: Manual approval process for high-value claims
- **Stateful Waiting**: Workflow pauses and resumes based on user input
- **Business Logic**: $1000 threshold for automatic vs manual approval
- **Comprehensive Context**: State maintained throughout entire workflow

### Task Breakdown

#### 1. Initial Claim Query ([`initial_claim_query.rs`](insurance-claims-service/src/tasks/initial_claim_query.rs))
- Welcomes users and gathers basic claim information
- Uses LLM to have natural conversations
- Extracts structured data from free-form input

#### 2. Insurance Type Classifier ([`insurance_type_classifier.rs`](insurance-claims-service/src/tasks/insurance_type_classifier.rs))
- Analyzes claim description to determine insurance type
- Uses conditional edges to route to appropriate detail collector
- Demonstrates intelligent content-based routing

#### 3. Detail Collectors
- **Car Insurance Details** ([`car_insurance_details.rs`](insurance-claims-service/src/tasks/car_insurance_details.rs))
- **Apartment Insurance Details** ([`apartment_insurance_details.rs`](insurance-claims-service/src/tasks/apartment_insurance_details.rs))
- Each specialized for their domain
- Collect comprehensive information through AI conversations

#### 4. Smart Claim Validator ([`smart_claim_validator.rs`](insurance-claims-service/src/tasks/smart_claim_validator.rs))
- **Intelligent Processing**: Auto-approves claims under $1,000
- **Human-in-the-Loop**: Requests manual approval for larger claims
- **Stateful Waiting**: Can pause workflow awaiting human decision
- **Status Messaging**: Comprehensive logging and status tracking

#### 5. Final Summary ([`final_summary.rs`](insurance-claims-service/src/tasks/final_summary.rs))
- Generates comprehensive claim summaries
- Handles both approved and rejected outcomes
- Provides clear next steps to users

### Key Architectural Patterns

#### 1. **LLM-First Design**
Every interactive task uses LLM agents for natural language processing:

```rust
// From insurance_type_classifier.rs
let agent = get_llm_agent(
    "You are an insurance classifier. Analyze claim descriptions and classify them as either 'car' or 'apartment' insurance claims."
)?;

let response = agent.chat(&user_input, chat_history).await?;
```

#### 2. **Structured Data Extraction**
Tasks extract structured data from conversational input:

```rust
// Parse JSON from LLM response
let claim_details: ClaimDetails = serde_json::from_str(&json_response)?;
context.set(session_keys::CLAIM_DETAILS, claim_details).await;
```

#### 3. **Conditional Workflow Routing**
Dynamic graph traversal based on runtime state:

```rust
// Route based on insurance type determined by LLM
.add_conditional_edge(
    classifier_id,
    |context| {
        context.get_sync::<String>(session_keys::INSURANCE_TYPE)
            .map(|t| t == "car")
            .unwrap_or(false)
    },
    car_details_id,
    apartment_details_id,
)
```

#### 4. **Human-in-the-Loop Processing**
Tasks can pause and wait for human intervention:

```rust
// In smart_claim_validator.rs
if estimated_cost >= 1000.0 {
    // Wait for manual approval
    return Ok(TaskResult::new(
        Some("Claim requires manual approval. Please review and approve/reject.".to_string()),
        NextAction::WaitForInput
    ));
}
```

#### 5. **Session State Management**
Complex state persists across multiple interactions:

 - Chat history (full conversation)
 - Structured data (claim details, validations)
 - Workflow position (current task)
 - Status messages (audit trail)


### Running the Insurance Claims Service

```bash
# Set up environment
export OPENROUTER_API_KEY="your-key"
export DATABASE_URL="postgresql://user:pass@localhost/db" # Optional

# Start the service
cargo run --bin insurance-claims-service

# Test with curl
curl -X POST http://localhost:3000/execute \
  -H "Content-Type: application/json" \
  -d '{"content": "I need to file a claim for my car accident"}'
```

The service demonstrates a complete HTTP API integration:

```rust
// main.rs - Create components once at startup
struct AppState {
    session_storage: Arc<dyn SessionStorage>,
    flow_runner: FlowRunner,
}

// HTTP handler - execute workflow step
async fn execute_graph(
    State(state): State<AppState>,
    Json(request): Json<ExecuteRequest>,
) -> Result<Json<ExecuteResponse>, StatusCode> {
    // Set user input in session context
    let session = state.session_storage.get(&session_id).await?;
    session.context.set("user_input", request.content).await;
    state.session_storage.save(session).await?;
    
    // Execute one workflow step
    let result = state.flow_runner.run(&session_id).await?;
    
    Ok(Json(ExecuteResponse {
        session_id,
        response: result.response,
        status: format!("{:?}", result.status),
    }))
}
```

### API Usage Examples

#### Starting a New Claim
```bash
POST /execute
{
  "content": "I had a car accident and need to file a claim"
}

Response:
{
  "session_id": "uuid-here",
  "response": "I'm sorry to hear about your accident. I'm here to help you file your claim...",
  "status": "Continue"
}
```

#### Continuing the Conversation
```bash
POST /execute  
{
  "session_id": "uuid-here",
  "content": "It happened yesterday on Main Street"
}
```

#### Checking Session State
```bash
GET /session/{session_id}

Response:
{
  "id": "uuid-here",
  "current_task_id": "InsuranceTypeClassifierTask",
  "context": {...},
  "status_message": "Determining insurance type based on claim description"
}
```

## Core Framework Concepts

### Tasks and Task Results

Every task returns a [`TaskResult`](graph-flow/src/task.rs) that controls workflow execution:

```rust
pub struct TaskResult {
    pub response: Option<String>,      // Response to user
    pub next_action: NextAction,       // What to do next
}

pub enum NextAction {
    Continue,           // Move to next task, return control to caller
    ContinueAndExecute, // Move to next task and execute it immediately  
    WaitForInput,       // Pause and wait for more user input
    End,               // Complete the workflow
}
```

#### Step-by-Step vs `ContinueAndExecute`

The **default execution model** is *step-wise*. After each task finishes the engine:

1. Stores any updates the task made to the `Context` / session.
2. Decides what the next task *would* be.
3. Returns control back to the caller **without** running that next task – you remain in charge of when to resume.

That behaviour is triggered by returning `NextAction::Continue` (or `WaitForInput`, `End`, etc.).

If you prefer a *fire-and-forget* flow for a particular step you can return `NextAction::ContinueAndExecute` instead. In that case the graph **immediately** calls the next task within the same request cycle, propagating the same `Context` – useful for fully automated branches where no external input is needed.

Put differently:

• **Continue** ⇒ *advance one edge, then stop* (the service responds after every hop).  
• **ContinueAndExecute** ⇒ *advance and keep running* until a task chooses a different action.

This fine-grained control lets you blend synchronous chains (multiple tasks auto-executed) with interactive pauses (waiting for user input) in the same workflow.

### Context and State Management

The [`Context`](graph-flow/src/context.rs) provides thread-safe state management:

```rust
// Store typed data
context.set("user_name", "Alice".to_string()).await;
context.set("claim_amount", 1500.0).await;

// Retrieve typed data
let name: String = context.get_sync("user_name").unwrap();
let amount: f64 = context.get("claim_amount").await.unwrap();

// Manage conversations
context.add_user_message("Hello".to_string()).await;
context.add_assistant_message("Hi there!".to_string()).await;
```

### Session Lifecycle

Sessions maintain workflow state across interactions:

1. **Creation**: New session starts at specified task
2. **Execution**: Tasks run and update session state
3. **Persistence**: Session state saved between interactions
4. **Resumption**: Sessions can be loaded and continued
5. **Completion**: Sessions end when workflow finishes

### Graph Construction

Graphs define the workflow structure:

```rust
let graph = GraphBuilder::new("workflow_name")
    .add_task(task1)           // Add tasks
    .add_task(task2)
    .add_edge("task1", "task2") // Linear connections
    .add_conditional_edge(      // Conditional routing
        "task2", 
        |ctx| condition_check(ctx),
        "task3_yes",   // yes branch
        "task3_no",    // else branch
    )
    .build();
```

## Production Considerations

### Storage Backends

- **Development**: [`InMemorySessionStorage`](graph-flow/src/storage.rs) - Fast, non-persistent
- **Production**: [`PostgresSessionStorage`](graph-flow/src/storage_postgres.rs) - Persistent, scalable





## Getting Started

1. **Quick Start**:
    - clone the repo 
    - add the LLM key and database key to the env
    - Simply re-write the solution with your own task flow, while only chnaging the tasks files  and graph structure in main.rs.
    - you have a agent orchestration flow wrapper in an Axum service. 

2. **Start on my own**:
   - import the graph execution crate 
   ```toml
   graph-flow = { git = "https://github.com/a-agmon/rs-graph-llm.git", package = "graph-flow", rev = "main", features = ["rig"] }
   ```
   - import rig 
   - write your tasks and flow

## License

MIT License - see [LICENSE](LICENSE).

## Execution Approaches

The framework provides two ways to execute workflows:

```rust
// High-level: FlowRunner handles session loading/saving automatically
let runner = FlowRunner::new(graph.clone(), session_storage.clone());
let result = runner.run(&session_id).await?;

// Low-level: Manual session management for custom control
let mut session = session_storage.get(&session_id).await?.unwrap();
let result = graph.execute_session(&mut session).await?;
session_storage.save(session).await?;
```

Both approaches are fully compatible and return the same `ExecutionResult`. Choose based on your needs - FlowRunner for convenience, manual for custom persistence logic or batch processing.
