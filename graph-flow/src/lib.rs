pub mod context;
pub mod error;
pub mod graph;
pub mod storage;
pub mod task;

// Re-export commonly used types
pub use context::Context;
pub use error::{GraphError, Result};
pub use graph::{ExecutionResult, ExecutionStatus, Graph, GraphBuilder};
pub use storage::{
    GraphStorage, InMemoryGraphStorage, InMemorySessionStorage, Session, SessionStorage,
};
pub use task::{NextAction, Task, TaskResult};

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
            let input: String = context.get("input").await.unwrap_or_default();
            context.set("output", format!("Processed: {}", input)).await;

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

        let graph = GraphBuilder::new("test_graph").add_task(task).build();

        let context = Context::new();
        context.set("input", "Hello, World!").await;

        let result = graph.execute("test_task", context.clone()).await.unwrap();

        assert!(result.response.is_some());
        assert!(matches!(result.next_action, NextAction::End));

        let output: String = context.get("output").await.unwrap();
        assert_eq!(output, "Processed: Hello, World!");
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

        let session = Session {
            id: "session1".to_string(),
            graph_id: "test".to_string(),
            current_task_id: "task1".to_string(),
            context: Context::new(),
        };

        session_storage.save(session.clone()).await.unwrap();
        let retrieved_session = session_storage.get("session1").await.unwrap();
        assert!(retrieved_session.is_some());
    }
}
