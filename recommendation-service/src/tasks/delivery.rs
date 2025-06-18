use async_trait::async_trait;
use graph_flow::GraphError::TaskExecutionFailed;
use graph_flow::{Context, NextAction, Task, TaskResult};
use tracing::info;

/// Task to deliver the final validated answer
pub struct DeliveryTask;

#[async_trait]
impl Task for DeliveryTask {
    fn id(&self) -> &str {
        std::any::type_name::<Self>()
    }

    async fn run(&self, context: Context) -> graph_flow::Result<TaskResult> {
        info!("Starting delivery task");

        let answer: String = context
            .get_sync("answer")
            .ok_or_else(|| TaskExecutionFailed("answer not found in context".into()))?;

        let retry_count: u32 = context
            .get_sync("retry_count")
            .ok_or_else(|| TaskExecutionFailed("retry_count not found in context".into()))?;

        info!("Delivering final answer after {} retries", retry_count);

        Ok(TaskResult::new(Some(answer), NextAction::End))
    }
} 