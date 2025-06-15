# rs-graph-llm: A LangGraph-like Stateful Graph Execution Framework for Rust

A lean, powerful stateful graph execution framework for building and running interactive agentic workflows with LLM integration, inspired by LangGraph but built from the ground up in Rust.

## Why This Framework?

LangGraph is awesome. It makes it easy to design stateful, graph-based agent workflows with built-in persistence and retries, so you can focus on the logic instead of the infrastructure. But when it comes to heavy production workloads, its Python core can become a bottleneck—performance is limited, and it imposes runtime overhead that can be costly at scale. Additionally, debugging complex async flows is often challenging, and the persistence layer—while flexible—is not always transparent or easy to integrate, with a schema that can be difficult to evolve or query directly.

*rs-graph-llm* takes the best ideas from LangGraph and tries to build them in Rust: fast, memory-efficient, and designed for production. You still get a simple graph-based model and resumable execution, but with strong typing, clear and queryable persistence (Postgres), full observability, and the reliability of a single compiled binary that can run anywhere.

## Architecture Overview

The framework consists of two main components:

- **[`graph-flow`](graph-flow/)**: Core library for defining and executing stateful task graphs
- **[`graph-service`](graph-service/)**: HTTP service that exposes graph execution via REST APIs

## Quick Start with Simple Example

Let's start with the basics using [`examples/simple_example.rs`](examples/simple_example.rs):

### 1. Define Tasks

Tasks are the building blocks of your workflow. Each task implements the [`Task`](graph-flow/src/task.rs) trait:

```rust
use async_trait::async_trait;
use graph_flow::{Context, Task, TaskResult, NextAction};

struct HelloTask;

#[async_trait]
impl Task for HelloTask {
    fn id(&self) -> &str {
        std::any::type_name::<Self>() // Use type name as unique ID
    }

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
use graph_flow::{GraphBuilder, InMemorySessionStorage, InMemoryGraphStorage};
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

### 3. Execute with Session Management

The framework provides **stateful execution** - workflows can be paused, resumed, and managed across multiple interactions:

```rust
// Create storage for graphs and sessions
let session_storage = Arc::new(InMemorySessionStorage::new());
let graph_storage = Arc::new(InMemoryGraphStorage::new());

// Create a session starting from the first task
let session = Session::new_from_task("session_001".to_string(), hello_task.id());
session.context.set("name", "Batman".to_string()).await;

// Execute step by step
loop {
    // Load current session state
    let mut current_session = session_storage.get(&session_id).await?
        .ok_or("Session not found")?;

    // Execute one step
    // Because the step returns: NextAction::Continue it will return after executing one task
    // If you return NextAction::ContinueAndExecute then it will NOT return and immediatly execute the next one
    let result = graph.execute_session(&mut current_session).await?;

    // Save updated state
    session_storage.save(current_session.clone()).await?;

    // Check completion status
    match result.status {
        ExecutionStatus::Completed => break,
        ExecutionStatus::WaitingForInput => continue, // Wait for more input
        ExecutionStatus::Error(err) => return Err(err),
    }
}
```

**Key Benefits of This Approach:**
- **Resumable**: Sessions can be saved and resumed later
- **Stateful**: Context persists across task executions
- **Controllable**: Execute one step at a time or run continuously
- **Persistent**: Session state survives service restarts (with proper storage)

## Advanced Features

### Conditional Edges

Create dynamic workflows that branch based on runtime decisions:

```rust
let graph = GraphBuilder::new("conditional_workflow")
    .add_task(classifier_task)
    .add_task(car_task)
    .add_task(apartment_task)
    // Route based on insurance_type: "car" -> car_task, else -> apartment_task
    .add_conditional_edge(
        "classifier",
        |context| {
            context.get_sync::<String>("insurance_type")
                .map(|t| t == "car")
                .unwrap_or(false)
        },
        "car_task",        // yes branch
        "apartment_task",  // else branch
    )
    .build();
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

The [`graph-service`](graph-service/) demonstrates a complete agentic workflow for processing insurance claims. This showcases the framework's power in building complex, multi-step AI-driven processes.

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

#### 1. Initial Claim Query ([`initial_claim_query.rs`](graph-service/src/tasks/initial_claim_query.rs))
- Welcomes users and gathers basic claim information
- Uses LLM to have natural conversations
- Extracts structured data from free-form input

#### 2. Insurance Type Classifier ([`insurance_type_classifier.rs`](graph-service/src/tasks/insurance_type_classifier.rs))
- Analyzes claim description to determine insurance type
- Uses conditional edges to route to appropriate detail collector
- Demonstrates intelligent content-based routing

#### 3. Detail Collectors
- **Car Insurance Details** ([`car_insurance_details.rs`](graph-service/src/tasks/car_insurance_details.rs))
- **Apartment Insurance Details** ([`apartment_insurance_details.rs`](graph-service/src/tasks/apartment_insurance_details.rs))
- Each specialized for their domain
- Collect comprehensive information through AI conversations

#### 4. Smart Claim Validator ([`smart_claim_validator.rs`](graph-service/src/tasks/smart_claim_validator.rs))
- **Intelligent Processing**: Auto-approves claims under $1,000
- **Human-in-the-Loop**: Requests manual approval for larger claims
- **Stateful Waiting**: Can pause workflow awaiting human decision
- **Status Messaging**: Comprehensive logging and status tracking

#### 5. Final Summary ([`final_summary.rs`](graph-service/src/tasks/final_summary.rs))
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

```rust
// State includes:
// - Chat history (full conversation)
// - Structured data (claim details, validations)
// - Workflow position (current task)
// - Status messages (audit trail)
```

### Running the Insurance Claims Service

```bash
# Set up environment
export OPENROUTER_API_KEY="your-key"
export DATABASE_URL="postgresql://user:pass@localhost/db" # Optional

# Start the service
cargo run --bin graph-service

# Test with curl
curl -X POST http://localhost:3000/execute \
  -H "Content-Type: application/json" \
  -d '{"content": "I need to file a claim for my car accident"}'
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

### Scaling Patterns

- **Horizontal**: Multiple service instances share PostgreSQL storage
- **Vertical**: Single instance handles multiple concurrent sessions
- **Hybrid**: Load balancer + multiple instances + shared storage

### Error Handling

The framework provides comprehensive error handling:

```rust
// Tasks can return errors that pause workflows
Err(graph_flow::Error::TaskError("Invalid input".to_string()))

// Errors are captured in session state and can be recovered
match result.status {
    ExecutionStatus::Error(err) => {
        // Handle error, potentially retry or redirect
    }
}
```

### Monitoring and Observability

- **Structured Logging**: Built-in tracing support
- **Session Tracking**: Full audit trail of workflow execution
- **Status Messages**: Detailed status information at each step

## Comparison with LangGraph

| Feature | LangGraph (Python) | RS-Inter-Task (Rust) |
|---------|-------------------|----------------------|
| **Type Safety** | Runtime | Compile-time |
| **Performance** | Interpreted | Compiled |
| **Memory Safety** | GC + Runtime errors | Compile-time guarantees |
| **Concurrency** | GIL limitations | Native async/await |
| **State Management** | Dict-based | Strongly typed |
| **Error Handling** | Exception-based | Result-based |
| **LLM Integration** | LangChain ecosystem | Rig crate |
| **Persistence** | External required | Built-in abstractions |

## Future Enhancements

- **WebSocket Support**: Real-time streaming responses
- **Graph Visualization**: Visual workflow design and monitoring  
- **Distributed Execution**: Multi-node workflow execution
- **Advanced Routing**: ML-based conditional logic
- **Workflow Analytics**: Performance and usage insights
- **Template Library**: Pre-built workflow patterns

## Getting Started

1. **Clone and explore**:
   ```bash
   git clone <repo-url>
   cd rs-inter-task
   ```

2. **Run the simple example**:
   ```bash
   cargo run --example simple_example
   ```

3. **Try the insurance service**:
   ```bash
   export OPENROUTER_API_KEY="your-key"
   cargo run --bin graph-service
   ```

4. **Build your own workflow**:
   - Define tasks implementing the `Task` trait
   - Use `GraphBuilder` to connect them
   - Add LLM integration with Rig agents
   - Deploy with session persistence

## License

MIT License - see [LICENSE](LICENSE) for details.