use async_trait::async_trait;
use graph_flow::GraphError::TaskExecutionFailed;
use graph_flow::{Context, NextAction, Task, TaskResult};
use rig::completion::Chat;
use tracing::info;

use super::utils::get_llm_agent;

/// Task to refine user queries for better vector search
pub struct QueryRefinementTask;

#[async_trait]
impl Task for QueryRefinementTask {

    async fn run(&self, context: Context) -> graph_flow::Result<TaskResult> {
        info!("Starting query refinement task");
        let user_query: String = context
            .get_sync("user_query")
            .ok_or_else(|| TaskExecutionFailed("user_query not found in context".into()))?;

        info!("Original user query: {}", user_query);

        let agent = get_llm_agent()
            .map_err(|e| TaskExecutionFailed(format!("Failed to initialize LLM agent: {}", e)))?;

        let refined = agent
            .chat(
                &format!(
                    r#"
                    You are a helpful movie recommendation assistant that rewrites user queries for vector search.
                    Rewrite the following user query so that it is optimised for vector search. Only return the rewritten query.
                    Query: {user_query}"#
                ),
                vec![],
            )
            .await
            .map_err(|e| TaskExecutionFailed(format!("LLM chat failed: {}", e)))?
            .trim()
            .to_string();

        info!("Refined query: {}", refined);
        context.set("refined_query", refined.clone()).await;
        // Initialize retry count
        context.set("retry_count", 0u32).await;

        Ok(TaskResult::new(None, NextAction::ContinueAndExecute))
    }
} 