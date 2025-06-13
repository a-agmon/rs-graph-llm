use async_trait::async_trait;
use graph_flow::{
    Context, GraphBuilder, NextAction, Task, TaskResult,
    InMemorySessionStorage, InMemoryGraphStorage, Session, SessionStorage, GraphStorage
};
use std::sync::Arc;

// We have 2 tasks in this simple example:
// 1. HelloTask - greets the user by name
// 2. ExcitementTask - adds excitement to the greeting
struct HelloTask;

#[async_trait]
impl Task for HelloTask {
    fn id(&self) -> &str {
        // Use the type name as the unique identifier for this task
        // This is a simple way to ensure uniqueness in this example
        // In a real application, you might want to use a more structured ID
        std::any::type_name::<Self>()
    }

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
    fn id(&self) -> &str {
         std::any::type_name::<Self>()
    }

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

    let graph = Arc::new(GraphBuilder::new("greeting_workflow")
        .add_task(hello_task)
        .add_task(excitement_task)
        .add_edge(&hello_task_id, &excitement_task_id) // Connect the tasks
        .build());

    // Store the graph in graph storage
    graph_storage.save("greeting_workflow".to_string(), graph.clone()).await?;

    // Create a session with initial context
    let session_id = "session_001".to_string();
    let session = Session::new_from_task(session_id.clone(), &hello_task_id);{


    // Set up context with input data
    session.context.set("name", "Batman".to_string()).await;
    // Save the session
    session_storage.save(session.clone()).await?;

    println!("Starting simple workflow with session management\n");
    println!("Session ID: {}", session.id);
    println!("Initial task: {}\n", session.current_task_id);

    // Execute the workflow using session management
    // we will loop through tasks until completio
    // The execution is stateful so that this can be managed and resumed across multiple calls
    loop {
        // Load session from storage
        let mut current_session = session_storage.get(&session_id).await?
            .ok_or("Session not found")?;

        println!("-------");
        println!("Executing task: {}", current_session.current_task_id);

        // Execute current task in session
        let execution_result = graph.execute_session(&mut current_session).await?;

        // Save updated session - this will persist the state after task execution
        session_storage.save(current_session.clone()).await?;

        // Print results
        if let Some(response) = &execution_result.response {
            println!("Task response: {}", response);
        }

        if let Some(status_msg) = &current_session.status_message {
            println!("Status: {}", status_msg);
        }

        println!("Execution status: {:?}", execution_result.status);
        println!("Next task: {}\n", current_session.current_task_id);

        // Check if workflow is completed
        match execution_result.status {
            graph_flow::ExecutionStatus::Completed => {
                println!("Workflow completed successfully!");
                break;
            }
            graph_flow::ExecutionStatus::WaitingForInput => {
                println!("Workflow is waiting for input. Please provide the next input.");
                // In this simple example, we'll continue automatically
                // In a real application, you might wait for user input here
                continue;
            }
            graph_flow::ExecutionStatus::Error(err) => {
                println!("Error occurred: {}", err);
                break;
            }

           
        }
         
    }

    // Demonstrate session persistence by retrieving final session
    let final_session = session_storage.get(&session_id).await?
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
}
