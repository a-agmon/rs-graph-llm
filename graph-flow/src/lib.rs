//! # graph-flow
//!
//! A high-performance, type-safe framework for building multi-agent workflow systems in Rust.
//!
//! ## Features
//!
//! - **Type-Safe Workflows**: Compile-time guarantees for workflow correctness
//! - **Flexible Execution**: Step-by-step, batch, or mixed execution modes
//! - **Built-in Persistence**: PostgreSQL and in-memory storage backends with
//!   optimistic locking
//! - **LLM Integration**: Optional integration with Rig for AI agent capabilities
//! - **Human-in-the-Loop**: Natural workflow interruption and resumption
//! - **Async/Await Native**: Built from the ground up for async Rust
//!
//! ## Quick Start
//!
//! ```rust
//! use graph_flow::{Context, Task, TaskResult, NextAction, GraphBuilder, FlowRunner, InMemorySessionStorage, Session, SessionStorage};
//! use async_trait::async_trait;
//! use std::sync::Arc;
//!
//! // Define a task
//! struct HelloTask;
//!
//! #[async_trait]
//! impl Task for HelloTask {
//!     fn id(&self) -> &str {
//!         "hello_task"
//!     }
//!
//!     async fn run(&self, context: Context) -> graph_flow::Result<TaskResult> {
//!         let name: String = context.get("name").unwrap_or("World".to_string());
//!         let greeting = format!("Hello, {}!", name);
//!
//!         context.set("greeting", greeting.clone())?;
//!         Ok(TaskResult::new(Some(greeting), NextAction::Continue))
//!     }
//! }
//!
//! # #[tokio::main]
//! # async fn main() -> graph_flow::Result<()> {
//! // Build the workflow
//! let hello_task = Arc::new(HelloTask);
//! let graph = Arc::new(
//!     GraphBuilder::new("greeting_workflow")
//!         .add_task(hello_task.clone())
//!         .build()?
//! );
//!
//! // Set up session storage and runner
//! let session_storage = Arc::new(InMemorySessionStorage::new());
//! let flow_runner = FlowRunner::new(graph.clone(), session_storage.clone());
//!
//! // Create and execute session
//! let session = Session::new_from_task("user_123".to_string(), hello_task.id());
//! session.context.set("name", "Alice".to_string())?;
//! session_storage.save(session).await?;
//!
//! let result = flow_runner.run("user_123").await?;
//! println!("Response: {:?}", result.response);
//! # Ok(())
//! # }
//! ```
//!
//! ## Core Concepts
//!
//! ### Tasks
//!
//! Tasks are the building blocks of your workflow. They implement the [`Task`] trait:
//!
//! ```rust
//! use graph_flow::{Task, TaskResult, NextAction, Context};
//! use async_trait::async_trait;
//!
//! struct MyTask;
//!
//! #[async_trait]
//! impl Task for MyTask {
//!     fn id(&self) -> &str {
//!         "my_task"
//!     }
//!
//!     async fn run(&self, context: Context) -> graph_flow::Result<TaskResult> {
//!         // Your task logic here
//!         Ok(TaskResult::new(Some("Done!".to_string()), NextAction::End))
//!     }
//! }
//! ```
//!
//! ### Context
//!
//! The [`Context`] provides thread-safe state management across your workflow.
//! Its methods are synchronous, so they work both in async tasks and in
//! edge-condition closures:
//!
//! ```rust
//! # use graph_flow::Context;
//! # fn main() -> graph_flow::Result<()> {
//! let context = Context::new();
//!
//! // Store and retrieve data
//! context.set("key", "value")?;
//! let value: Option<String> = context.get("key");
//!
//! // Chat history management
//! context.add_user_message("Hello!".to_string());
//! context.add_assistant_message("Hi there!".to_string());
//! # Ok(())
//! # }
//! ```
//!
//! ### Graph Building
//!
//! Use [`GraphBuilder`] to create workflows. [`GraphBuilder::build`] validates
//! the graph (edge endpoints and the start task must exist) and returns an
//! immutable [`Graph`]:
//!
//! ```rust
//! # use graph_flow::{GraphBuilder, Task, TaskResult, NextAction, Context};
//! # use async_trait::async_trait;
//! # use std::sync::Arc;
//! # struct Task1; struct Task2; struct Task3;
//! # #[async_trait] impl Task for Task1 { fn id(&self) -> &str { "task1" } async fn run(&self, _: Context) -> graph_flow::Result<TaskResult> { Ok(TaskResult::new(None, NextAction::End)) } }
//! # #[async_trait] impl Task for Task2 { fn id(&self) -> &str { "task2" } async fn run(&self, _: Context) -> graph_flow::Result<TaskResult> { Ok(TaskResult::new(None, NextAction::End)) } }
//! # #[async_trait] impl Task for Task3 { fn id(&self) -> &str { "task3" } async fn run(&self, _: Context) -> graph_flow::Result<TaskResult> { Ok(TaskResult::new(None, NextAction::End)) } }
//! # fn main() -> graph_flow::Result<()> {
//! # let task1 = Arc::new(Task1); let task2 = Arc::new(Task2); let task3 = Arc::new(Task3);
//! let graph = GraphBuilder::new("my_workflow")
//!     .add_task(task1.clone())
//!     .add_task(task2.clone())
//!     .add_task(task3.clone())
//!     .add_edge(task1.id(), task2.id())
//!     .add_conditional_edge(
//!         task2.id(),
//!         |ctx| ctx.get::<bool>("condition").unwrap_or(false),
//!         task3.id(),    // if true
//!         task1.id(),    // if false
//!     )
//!     .build()?;
//! # Ok(())
//! # }
//! ```
//!
//! ### Execution
//!
//! Use [`FlowRunner`] for convenient session-based execution:
//!
//! ```rust,no_run
//! # use graph_flow::{FlowRunner, InMemorySessionStorage, Session, Graph, SessionStorage};
//! # use std::sync::Arc;
//! # #[tokio::main]
//! # async fn main() -> graph_flow::Result<()> {
//! # let graph = Arc::new(Graph::new("test"));
//! let storage = Arc::new(InMemorySessionStorage::new());
//! let runner = FlowRunner::new(graph, storage.clone());
//!
//! // Create session
//! let session = Session::new_from_task("session_id".to_string(), "start_task");
//! storage.save(session).await?;
//!
//! // Execute workflow
//! let result = runner.run("session_id").await?;
//! # Ok(())
//! # }
//! ```
//!
//! ## Features
//!
//! - **Default**: Core workflow functionality
//! - **`rig`**: Enables LLM integration via the Rig crate
//!
//! ## Storage Backends
//!
//! - [`InMemorySessionStorage`]: For development and testing
//! - `PostgresSessionStorage`: PostgreSQL storage, available with the `postgres` feature

pub mod context;
pub mod error;
pub mod graph;
pub mod runner;
pub mod storage;
#[cfg(feature = "postgres")]
pub mod storage_postgres;
pub mod task;
pub mod fanout;

// Re-export commonly used types
pub use context::{ChatHistory, Context, MessageRole, SerializableMessage};
pub use error::{GraphError, Result};
pub use graph::{ExecutionResult, ExecutionStatus, Graph, GraphBuilder};
pub use runner::FlowRunner;
pub use storage::{
    GraphStorage, InMemoryGraphStorage, InMemorySessionStorage, Session, SessionStorage,
};
#[cfg(feature = "postgres")]
pub use storage_postgres::PostgresSessionStorage;
pub use task::{NextAction, Task, TaskResult};
pub use fanout::FanOutTask;

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use std::sync::Arc;

    struct TestTask {
        id: String,
    }

    #[async_trait]
    impl Task for TestTask {
        fn id(&self) -> &str {
            &self.id
        }

        async fn run(&self, context: Context) -> Result<TaskResult> {
            let input: String = context.get("input").unwrap_or_default();
            context.set("output", format!("Processed: {}", input))?;

            Ok(TaskResult::new(
                Some("Task completed".to_string()),
                NextAction::End,
            ))
        }
    }

    #[tokio::test]
    async fn test_simple_graph_execution() {
        let task = Arc::new(TestTask {
            id: "test_task".to_string(),
        });

        let graph = GraphBuilder::new("test_graph")
            .add_task(task)
            .build()
            .unwrap();

        let mut session = Session::new_from_task("s1".to_string(), "test_task");
        session.context.set("input", "Hello, World!").unwrap();

        let result = graph.execute_session(&mut session).await.unwrap();

        assert!(result.response.is_some());
        assert!(matches!(result.status, ExecutionStatus::Completed));

        let output: String = session.context.get("output").unwrap();
        assert_eq!(output, "Processed: Hello, World!");
    }

    #[tokio::test]
    async fn test_build_rejects_dangling_edge() {
        let task = Arc::new(TestTask {
            id: "only_task".to_string(),
        });

        let result = GraphBuilder::new("bad_graph")
            .add_task(task)
            .add_edge("only_task", "missing_task")
            .build();

        assert!(matches!(result, Err(GraphError::InvalidEdge(_))));
    }

    #[tokio::test]
    async fn test_build_rejects_unknown_start_task() {
        let task = Arc::new(TestTask {
            id: "only_task".to_string(),
        });

        let result = GraphBuilder::new("bad_graph")
            .add_task(task)
            .set_start_task("missing_task")
            .build();

        assert!(matches!(result, Err(GraphError::TaskNotFound(_))));
    }

    #[tokio::test]
    async fn test_storage() {
        let graph_storage = InMemoryGraphStorage::new();
        let session_storage = InMemorySessionStorage::new();

        let graph = Arc::new(Graph::new("test"));
        graph_storage
            .save("test".to_string(), graph.clone())
            .await
            .unwrap();

        let retrieved = graph_storage.get("test").await.unwrap();
        assert!(retrieved.is_some());

        let session =
            Session::new_from_task("session1".to_string(), "task1").with_graph_id("test");

        session_storage.save(session).await.unwrap();
        let retrieved_session = session_storage.get("session1").await.unwrap();
        assert!(retrieved_session.is_some());
        assert_eq!(retrieved_session.unwrap().graph_id, "test");
    }
}
