# Event-Driven Agent Workflow Execution System

## Overview

This document outlines the design for an event-driven agent workflow execution system. The system enables scalable, distributed execution of workflows through events published to a message queue. Tasks are event handlers that respond to specific events and can raise new events to continue execution.

## Core Architecture Principles

1. **Event-Driven Execution**: Workers pull events from the queue and route them to registered task handlers
2. **Tasks as Event Handlers**: Each task registers for specific event types and contains code to handle those events
3. **Database as State Store**: All workflow context, state, and execution history is persisted in the database
4. **Stateless Workers**: Workers are stateless and can be scaled horizontally
5. **Simple Event Flow**: Tasks receive events, update context, and raise new events to continue workflow execution

## Architecture Overview

```
┌─────────────┐    ┌─────────────┐    ┌─────────────┐    ┌─────────────┐
│   Client    │───▶│ Workflow    │───▶│   Event     │───▶│   Worker    │
│             │    │ API Service │    │   Queue     │    │ Pool        │
└─────────────┘    └─────────────┘    └─────────────┘    └─────────────┘
                           │                                      │
                           │          ┌─────────────┐             │
                           └─────────▶│  Database   │◀────────────┘
                                      │ (Workflow   │
                                      │  Context)   │
                                      └─────────────┘

Event Flow:
Client → StartWorkflow → WorkflowStarted → ProcessClaim → ClaimProcessed → ValidateData → ...
```

## Main Entities

### 1. Event
Events are lightweight messages that trigger actions in the workflow.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowEvent {
    pub event_id: String,
    pub event_type: String,
    pub workflow_instance_id: String,
    pub payload: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub correlation_id: Option<String>,
    pub priority: u8,
}

// Example event types:
// "workflow.started"
// "claim.processed" 
// "validation.completed"
// "user.input.required"
// "workflow.completed"
```

### 2. Task (Event Handler)
Tasks are event handlers that respond to specific event types.

```rust
#[async_trait]
pub trait Task: Send + Sync {
    /// Returns the event types this task handles
    fn handles_events(&self) -> Vec<String>;
    
    /// Process an event and return new events to publish
    async fn handle_event(
        &self, 
        event: &WorkflowEvent, 
        context: &mut WorkflowContext
    ) -> Result<Vec<WorkflowEvent>>;
    
    /// Task identifier
    fn task_id(&self) -> &str;
}
```

### 3. Workflow Context
Contains all state and data for a workflow instance, persisted in database.

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowContext {
    pub workflow_instance_id: String,
    pub workflow_type: String,
    pub status: WorkflowStatus,
    pub data: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub event_history: Vec<ProcessedEvent>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessedEvent {
    pub event_id: String,
    pub event_type: String,
    pub processed_at: DateTime<Utc>,
    pub processed_by: String, // task_id
    pub result: EventProcessResult,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WorkflowStatus {
    Active,
    WaitingForInput,
    Completed,
    Failed,
    Cancelled,
}
```

### 4. Event Registry
Maps event types to task handlers.

```rust
pub struct EventRegistry {
    handlers: HashMap<String, Vec<Arc<dyn Task>>>,
}

impl EventRegistry {
    pub fn register_task(&mut self, task: Arc<dyn Task>) {
        for event_type in task.handles_events() {
            self.handlers.entry(event_type)
                .or_insert_with(Vec::new)
                .push(task.clone());
        }
    }
    
    pub fn get_handlers(&self, event_type: &str) -> Vec<Arc<dyn Task>> {
        self.handlers.get(event_type).cloned().unwrap_or_default()
    }
}
```

## Simple Insurance Claim Workflow Example

Let's demonstrate the event-driven approach with a simple insurance claim processing workflow:

**Event Flow**: `workflow.started` → `claim.processed` → `validation.completed` → `workflow.completed`

### Example Task Implementations

```rust
// Task 1: Process initial claim
pub struct ClaimProcessorTask;

#[async_trait]
impl Task for ClaimProcessorTask {
    fn handles_events(&self) -> Vec<String> {
        vec!["workflow.started".to_string()]
    }
    
    async fn handle_event(
        &self, 
        event: &WorkflowEvent, 
        context: &mut WorkflowContext
    ) -> Result<Vec<WorkflowEvent>> {
        // Extract claim data from event payload
        let claim_data = event.payload.get("claim_data")
            .ok_or("Missing claim data")?;
        
        // Process the claim (LLM call, data extraction, etc.)
        let processed_claim = process_claim_with_llm(claim_data).await?;
        
        // Update context with processed data
        context.data["processed_claim"] = processed_claim.clone();
        context.data["claim_type"] = json!(processed_claim.claim_type);
        
        // Raise next event
        Ok(vec![WorkflowEvent {
            event_id: generate_id(),
            event_type: "claim.processed".to_string(),
            workflow_instance_id: context.workflow_instance_id.clone(),
            payload: json!({
                "claim_type": processed_claim.claim_type,
                "amount": processed_claim.amount
            }),
            created_at: Utc::now(),
            correlation_id: event.correlation_id.clone(),
            priority: 100,
        }])
    }
    
    fn task_id(&self) -> &str {
        "claim_processor"
    }
}

// Task 2: Validate claim
pub struct ClaimValidatorTask;

#[async_trait]
impl Task for ClaimValidatorTask {
    fn handles_events(&self) -> Vec<String> {
        vec!["claim.processed".to_string()]
    }
    
    async fn handle_event(
        &self, 
        event: &WorkflowEvent, 
        context: &mut WorkflowContext
    ) -> Result<Vec<WorkflowEvent>> {
        // Get claim data from context
        let processed_claim = context.data.get("processed_claim")
            .ok_or("Missing processed claim")?;
        
        // Validate the claim
        let validation_result = validate_claim(processed_claim).await?;
        
        // Update context
        context.data["validation_result"] = json!(validation_result);
        
        // Determine next event based on validation
        let next_event_type = if validation_result.is_valid {
            "validation.completed"
        } else {
            "validation.failed"
        };
        
        Ok(vec![WorkflowEvent {
            event_id: generate_id(),
            event_type: next_event_type.to_string(),
            workflow_instance_id: context.workflow_instance_id.clone(),
            payload: json!({
                "is_valid": validation_result.is_valid,
                "errors": validation_result.errors
            }),
            created_at: Utc::now(),
            correlation_id: event.correlation_id.clone(),
            priority: 100,
        }])
    }
    
    fn task_id(&self) -> &str {
        "claim_validator"
    }
}

// Task 3: Complete workflow
pub struct WorkflowCompletionTask;

#[async_trait]
impl Task for WorkflowCompletionTask {
    fn handles_events(&self) -> Vec<String> {
        vec!["validation.completed".to_string()]
    }
    
    async fn handle_event(
        &self, 
        event: &WorkflowEvent, 
        context: &mut WorkflowContext
    ) -> Result<Vec<WorkflowEvent>> {
        // Generate final summary
        let summary = generate_claim_summary(context).await?;
        
        // Update context and mark as completed
        context.data["final_summary"] = json!(summary);
        context.status = WorkflowStatus::Completed;
        context.completed_at = Some(Utc::now());
        
        // Raise completion event
        Ok(vec![WorkflowEvent {
            event_id: generate_id(),
            event_type: "workflow.completed".to_string(),
            workflow_instance_id: context.workflow_instance_id.clone(),
            payload: json!({
                "summary": summary,
                "status": "completed"
            }),
            created_at: Utc::now(),
            correlation_id: event.correlation_id.clone(),
            priority: 100,
        }])
    }
    
    fn task_id(&self) -> &str {
        "workflow_completion"
    }
}
```

### Task Registration and Worker Implementation

```rust
// Worker that processes events
pub struct EventWorker {
    event_registry: Arc<EventRegistry>,
    database: Arc<dyn Database>,
    event_queue: Arc<dyn EventQueue>,
}

impl EventWorker {
    pub fn new() -> Self {
        let mut registry = EventRegistry::new();
        
        // Register all tasks
        registry.register_task(Arc::new(ClaimProcessorTask));
        registry.register_task(Arc::new(ClaimValidatorTask));
        registry.register_task(Arc::new(WorkflowCompletionTask));
        
        Self {
            event_registry: Arc::new(registry),
            database: Arc::new(PostgresDatabase::new()),
            event_queue: Arc::new(RedisEventQueue::new()),
        }
    }
    
    pub async fn run(&self) -> Result<()> {
        loop {
            // Pull event from queue
            if let Some(event) = self.event_queue.pull_event().await? {
                if let Err(e) = self.process_event(event).await {
                    error!("Failed to process event: {}", e);
                }
            }
            
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }
    
    async fn process_event(&self, event: WorkflowEvent) -> Result<()> {
        // Load workflow context
        let mut context = self.database
            .get_workflow_context(&event.workflow_instance_id)
            .await?;
        
        // Find handlers for this event type
        let handlers = self.event_registry.get_handlers(&event.event_type);
        
        for handler in handlers {
            // Execute handler
            let new_events = handler.handle_event(&event, &mut context).await?;
            
            // Record event processing
            context.event_history.push(ProcessedEvent {
                event_id: event.event_id.clone(),
                event_type: event.event_type.clone(),
                processed_at: Utc::now(),
                processed_by: handler.task_id().to_string(),
                result: EventProcessResult::Success,
            });
            
            // Save updated context
            self.database.save_workflow_context(&context).await?;
            
            // Publish new events
            for new_event in new_events {
                self.event_queue.publish_event(new_event).await?;
            }
        }
        
        Ok(())
    }
}
```

### Complete Workflow Execution Flow

**Scenario**: Starting a workflow and processing through completion

**Flow**:
1. Client starts workflow via API
2. API creates workflow context and publishes `workflow.started` event
3. Worker picks up event, routes to `ClaimProcessorTask`
4. Task processes claim, updates context, publishes `claim.processed` event
5. Worker routes to `ClaimValidatorTask`
6. Task validates claim, publishes `validation.completed` event
7. Worker routes to `WorkflowCompletionTask`
8. Task completes workflow and publishes `workflow.completed` event

```rust
// API endpoint to start workflow
async fn start_claim_workflow(
    Json(request): Json<StartClaimRequest>,
) -> Result<Json<WorkflowResponse>> {
    let workflow_instance_id = generate_id();
    
    // Create workflow context
    let context = WorkflowContext {
        workflow_instance_id: workflow_instance_id.clone(),
        workflow_type: "insurance_claim".to_string(),
        status: WorkflowStatus::Active,
        data: json!({
            "user_input": request.claim_description,
            "user_id": request.user_id
        }),
        created_at: Utc::now(),
        updated_at: Utc::now(),
        completed_at: None,
        event_history: vec![],
    };
    
    // Save to database
    database.save_workflow_context(&context).await?;
    
    // Publish initial event
    let event = WorkflowEvent {
        event_id: generate_id(),
        event_type: "workflow.started".to_string(),
        workflow_instance_id: workflow_instance_id.clone(),
        payload: json!({
            "claim_data": request.claim_description,
            "initiated_by": request.user_id
        }),
        created_at: Utc::now(),
        correlation_id: Some(generate_id()),
        priority: 100,
    };
    
    event_queue.publish_event(event).await?;
    
    Ok(Json(WorkflowResponse {
        workflow_instance_id,
        status: "started".to_string(),
    }))
}
```

### Workflow Interruption (Waiting for Input)

**Scenario**: Workflow needs to pause for external input

```rust
// Task that waits for user input
pub struct UserInputRequiredTask;

#[async_trait]
impl Task for UserInputRequiredTask {
    fn handles_events(&self) -> Vec<String> {
        vec!["validation.failed".to_string()]
    }
    
    async fn handle_event(
        &self, 
        event: &WorkflowEvent, 
        context: &mut WorkflowContext
    ) -> Result<Vec<WorkflowEvent>> {
        // Check if additional info is already provided
        if context.data.get("additional_info").is_some() {
            // Continue with processing
            return Ok(vec![WorkflowEvent {
                event_id: generate_id(),
                event_type: "user.input.received".to_string(),
                workflow_instance_id: context.workflow_instance_id.clone(),
                payload: json!({}),
                created_at: Utc::now(),
                correlation_id: event.correlation_id.clone(),
                priority: 100,
            }]);
        }
        
        // Need to wait for input
        context.status = WorkflowStatus::WaitingForInput;
        context.data["waiting_for"] = json!("additional_claim_information");
        
        // Publish event indicating input is required (doesn't continue workflow)
        Ok(vec![WorkflowEvent {
            event_id: generate_id(),
            event_type: "user.input.required".to_string(),
            workflow_instance_id: context.workflow_instance_id.clone(),
            payload: json!({
                "message": "Please provide additional claim information",
                "required_fields": ["additional_details", "supporting_documents"]
            }),
            created_at: Utc::now(),
            correlation_id: event.correlation_id.clone(),
            priority: 100,
        }])
    }
    
    fn task_id(&self) -> &str {
        "user_input_required"
    }
}

// API endpoint to provide input and resume workflow
async fn provide_user_input(
    Path(workflow_instance_id): Path<String>,
    Json(input): Json<UserInputRequest>,
) -> Result<Json<WorkflowResponse>> {
    // Load workflow context
    let mut context = database.get_workflow_context(&workflow_instance_id).await?;
    
    // Verify it's waiting for input
    if context.status != WorkflowStatus::WaitingForInput {
        return Err(Error::InvalidWorkflowState);
    }
    
    // Add user input to context
    context.data["additional_info"] = json!(input.additional_info);
    context.status = WorkflowStatus::Active;
    
    // Save updated context
    database.save_workflow_context(&context).await?;
    
    // Publish resume event
    let event = WorkflowEvent {
        event_id: generate_id(),
        event_type: "user.input.provided".to_string(),
        workflow_instance_id,
        payload: json!({
            "additional_info": input.additional_info
        }),
        created_at: Utc::now(),
        correlation_id: Some(generate_id()),
        priority: 100,
    };
    
    event_queue.publish_event(event).await?;
    
    Ok(Json(WorkflowResponse {
        workflow_instance_id,
        status: "resumed".to_string(),
    }))
}
```

### Parallel Execution (Fan-out)

**Scenario**: One event triggers multiple parallel tasks

```rust
// Task that triggers parallel processing
pub struct ParallelProcessorTask;

#[async_trait]
impl Task for ParallelProcessorTask {
    fn handles_events(&self) -> Vec<String> {
        vec!["claim.processed".to_string()]
    }
    
    async fn handle_event(
        &self, 
        event: &WorkflowEvent, 
        context: &mut WorkflowContext
    ) -> Result<Vec<WorkflowEvent>> {
        let claim_type = event.payload.get("claim_type")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        
        // Generate multiple events for parallel processing
        let mut events = vec![
            // Risk assessment (always runs)
            WorkflowEvent {
                event_id: generate_id(),
                event_type: "risk.assessment.requested".to_string(),
                workflow_instance_id: context.workflow_instance_id.clone(),
                payload: json!({ "claim_type": claim_type }),
                created_at: Utc::now(),
                correlation_id: event.correlation_id.clone(),
                priority: 100,
            },
            // Fraud detection (always runs)
            WorkflowEvent {
                event_id: generate_id(),
                event_type: "fraud.detection.requested".to_string(),
                workflow_instance_id: context.workflow_instance_id.clone(),
                payload: json!({ "claim_type": claim_type }),
                created_at: Utc::now(),
                correlation_id: event.correlation_id.clone(),
                priority: 100,
            },
        ];
        
        // Conditional processing based on claim type
        if claim_type == "car" {
            events.push(WorkflowEvent {
                event_id: generate_id(),
                event_type: "vehicle.inspection.requested".to_string(),
                workflow_instance_id: context.workflow_instance_id.clone(),
                payload: json!({ "claim_type": claim_type }),
                created_at: Utc::now(),
                correlation_id: event.correlation_id.clone(),
                priority: 100,
            });
        }
        
        // Track parallel tasks in context
        context.data["parallel_tasks"] = json!({
            "risk_assessment": "pending",
            "fraud_detection": "pending",
            "vehicle_inspection": if claim_type == "car" { "pending" } else { "not_required" }
        });
        
        Ok(events)
    }
    
    fn task_id(&self) -> &str {
        "parallel_processor"
    }
}

// Tasks that handle the parallel events
pub struct RiskAssessmentTask;
pub struct FraudDetectionTask;
pub struct VehicleInspectionTask;

// Each task handles its event and updates the parallel task status
#[async_trait]
impl Task for RiskAssessmentTask {
    fn handles_events(&self) -> Vec<String> {
        vec!["risk.assessment.requested".to_string()]
    }
    
    async fn handle_event(
        &self, 
        event: &WorkflowEvent, 
        context: &mut WorkflowContext
    ) -> Result<Vec<WorkflowEvent>> {
        // Perform risk assessment
        let risk_score = calculate_risk_score(&context.data).await?;
        
        // Update context
        context.data["risk_score"] = json!(risk_score);
        context.data["parallel_tasks"]["risk_assessment"] = json!("completed");
        
        // Check if all parallel tasks are done
        if all_parallel_tasks_completed(&context.data) {
            Ok(vec![WorkflowEvent {
                event_id: generate_id(),
                event_type: "parallel.processing.completed".to_string(),
                workflow_instance_id: context.workflow_instance_id.clone(),
                payload: json!({}),
                created_at: Utc::now(),
                correlation_id: event.correlation_id.clone(),
                priority: 100,
            }])
        } else {
            Ok(vec![]) // Wait for other parallel tasks
        }
    }
    
    fn task_id(&self) -> &str {
        "risk_assessment"
    }
}

fn all_parallel_tasks_completed(data: &serde_json::Value) -> bool {
    let parallel_tasks = data.get("parallel_tasks").unwrap();
    parallel_tasks.as_object().unwrap().values()
        .all(|status| status == "completed" || status == "not_required")
}
```

**Benefits of Event-Driven Approach**:

1. **Simplicity**: Tasks just handle events and raise new events
2. **Decoupling**: No complex workflow graph management 
3. **Natural Parallelism**: Multiple events can be published simultaneously
4. **Easy Testing**: Each task can be tested independently
5. **Flexible Routing**: Event handlers can be registered/unregistered dynamically
6. **Fault Tolerance**: Failed event processing doesn't affect other events
7. **Scalability**: Workers can specialize in specific event types

## Summary

This event-driven approach dramatically simplifies workflow execution:

### Key Advantages

1. **Simple Programming Model**: 
   - Tasks register for event types they handle
   - Tasks receive events, update context, raise new events
   - No complex graph traversal or edge evaluation logic

2. **Natural Scalability**:
   - Workers can specialize in specific event types
   - Multiple workers can process different events in parallel
   - Easy to add new task types by registering new event handlers

3. **Loose Coupling**:
   - Tasks don't need to know about other tasks
   - Workflow logic is distributed across individual tasks
   - Easy to modify or extend workflows by adding new event handlers

4. **Fault Tolerance**:
   - Failed event processing doesn't affect other events
   - Easy to implement retry logic at the event level
   - Dead letter queues for permanently failed events

5. **Testing & Debugging**:
   - Each task can be tested independently
   - Clear event history shows exactly what happened
   - Easy to replay events for debugging

### Architecture Summary

```
Events Flow: user.request → claim.processed → risk.assessment.requested
                        ↘                   ↗
                         validation.completed → workflow.completed

Tasks:       ClaimProcessor → RiskAssessment, FraudDetection → CompletionTask
```

**Core Components**:
- **Events**: Lightweight trigger messages in queue
- **Tasks**: Event handlers that process events and emit new ones
- **Context**: Workflow state persisted in database  
- **Workers**: Pull events and route to registered task handlers
- **Registry**: Maps event types to task handlers

This design provides maximum flexibility while keeping the implementation simple and the system highly scalable.