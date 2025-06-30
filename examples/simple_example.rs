use async_trait::async_trait;
use graph_flow::{
    Context, ExecutionStatus, FlowRunner, GraphBuilder, GraphStorage, InMemoryGraphStorage,
    InMemorySessionStorage, NextAction, Session, SessionStorage, Task, TaskResult,
};
use std::sync::Arc;

// We have 2 tasks in this simple example:
// 1. HelloTask - greets the user by name
// 2. ExcitementTask - adds excitement to the greeting
struct HelloTask;

#[async_trait]
impl Task for HelloTask {
    async fn run(&self, context: Context) -> graph_flow::Result<TaskResult> {
        let name: String = context.get_sync("name").unwrap();
        let greeting = format!("Hello, {}", name);
        // Store result for next task
        context.set("greeting", greeting.clone()).await;

        // using NextAction::Continue to indicate we want to proceed to the next task,
        // but we want to advance just one step and give control back to the workflow manager
        // We can also use NextAction::ContinueAndExecute if we want to execute the next task immediately
        Ok(TaskResult::new(Some(greeting), NextAction::Continue))
    }
}

// Define a task that adds excitement
struct ExcitementTask;

#[async_trait]
impl Task for ExcitementTask {
    async fn run(&self, context: Context) -> graph_flow::Result<TaskResult> {
        let greeting: String = context.get_sync("greeting").unwrap();
        let excited = format!("{} !!!", greeting);

        Ok(TaskResult::new(Some(excited), NextAction::End))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create storage instances
    let session_storage = Arc::new(InMemorySessionStorage::new());
    let graph_storage = Arc::new(InMemoryGraphStorage::new());

    // Build a simple workflow
    let hello_task = Arc::new(HelloTask);
    let hello_task_id = hello_task.id().to_string();
    let excitement_task = Arc::new(ExcitementTask);
    let excitement_task_id = excitement_task.id().to_string();

    let graph = Arc::new(
        GraphBuilder::new("greeting_workflow")
            .add_task(hello_task)
            .add_task(excitement_task)
            .add_edge(&hello_task_id, &excitement_task_id) // Connect the tasks
            .build(),
    );

    // Store the graph in graph storage
    graph_storage
        .save("greeting_workflow".to_string(), graph.clone())
        .await?;

    // Create a session with initial context
    let session_id = "session_001".to_string();
    let session = Session::new_from_task(session_id.clone(), &hello_task_id);

    // Set up context with input data
    session.context.set("name", "Batman".to_string()).await;
    // Save the session
    session_storage.save(session.clone()).await?;

    println!("Starting simple workflow with FlowRunner\n");
    println!("Session ID: {}", session.id);
    println!("Initial task: {}\n", session.current_task_id);

    // Create a FlowRunner that hides the load / execute / save boilerplate
    let runner = FlowRunner::new(graph.clone(), session_storage.clone());

    // Execute until completion
    loop {
        let execution_result = runner.run(&session_id).await?;

        if let Some(response) = &execution_result.response {
            println!("Task response: {}", response);
        }

        println!("Execution status: {:?}", execution_result.status);

        match execution_result.status {
            ExecutionStatus::Completed => {
                println!("Workflow completed successfully!");
                break;
            }
            ExecutionStatus::Paused { next_task_id } => {
                println!("Workflow paused, will continue to task: {} – continuing...\n", next_task_id);
                continue;
            }
            ExecutionStatus::WaitingForInput => {
                println!("Workflow waiting for user input – continuing...\n");
                continue;
            }
            ExecutionStatus::Error(err) => {
                println!("Error occurred: {}", err);
                break;
            }
        }
    }

    // Demonstrate session persistence by retrieving final session
    let final_session = session_storage
        .get(&session_id)
        .await?
        .ok_or("Session not found")?;

    println!("\nFinal session state:");
    println!("Session ID: {}", final_session.id);
    println!("Current task: {}", final_session.current_task_id);
    if let Some(status) = &final_session.status_message {
        println!("Final status: {}", status);
    }

    // Demonstrate retrieving stored values from context
    if let Some(greeting) = final_session.context.get::<String>("greeting").await {
        println!("Stored greeting: {}", greeting);
    }

    println!("\nWorkflow execution finished");
    Ok(())
}
