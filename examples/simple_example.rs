use async_trait::async_trait;
use graph_flow::{Context, Graph, GraphBuilder, NextAction, Task, TaskResult};
use std::sync::Arc;

// Define a simple task that adds "Hello" to the input
struct HelloTask;

#[async_trait]
impl Task for HelloTask {
    fn id(&self) -> &str {
        "hello"
    }

    async fn run(&self, context: Context) -> graph_flow::Result<TaskResult> {
        let name: String = context.get("name").await.unwrap_or("World".to_string());
        let greeting = format!("Hello, {}!", name);

        // Store result for next task
        context.set("greeting", greeting.clone()).await;

        Ok(TaskResult::new(Some(greeting), NextAction::Continue))
    }
}

// Define a task that adds excitement
struct ExcitementTask;

#[async_trait]
impl Task for ExcitementTask {
    fn id(&self) -> &str {
        "excitement"
    }

    async fn run(&self, context: Context) -> graph_flow::Result<TaskResult> {
        let greeting: String = context.get("greeting").await.unwrap_or_default();
        let excited = format!("{} 🎉✨", greeting);

        Ok(TaskResult::new(Some(excited), NextAction::End))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Build a simple workflow
    let graph = GraphBuilder::new("greeting_workflow")
        .add_task(Arc::new(HelloTask))
        .add_task(Arc::new(ExcitementTask))
        .add_edge("hello", "excitement") // Connect the tasks
        .build();

    // Set up context with input data
    let context = Context::new();
    context.set("name", "Rust Developer".to_string()).await;

    // Execute the workflow
    println!("🚀 Starting simple workflow...\n");
    let result = graph.execute("hello", context).await?;

    if let Some(response) = result.response {
        println!("✅ Final result: {}", response);
    }

    Ok(())
}
