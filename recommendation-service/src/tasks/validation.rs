use async_trait::async_trait;
use graph_flow::GraphError::TaskExecutionFailed;
use graph_flow::{Context, NextAction, Task, TaskResult};
use rig::completion::Chat;
use tracing::{error, info};

use super::types::{ValidationResult, MAX_RETRIES};
use super::utils::get_llm_agent;

/// Task to validate generated answers
pub struct ValidationTask;

#[async_trait]
impl Task for ValidationTask {
    fn id(&self) -> &str {
        std::any::type_name::<Self>()
    }

    async fn run(&self, context: Context) -> graph_flow::Result<TaskResult> {
        info!("Starting validation task");

        let answer: String = context
            .get_sync("answer")
            .ok_or_else(|| TaskExecutionFailed("answer not found in context".into()))?;

        let user_query: String = context
            .get_sync("user_query")
            .ok_or_else(|| TaskExecutionFailed("user_query not found in context".into()))?;

        let retry_count: u32 = context
            .get_sync("retry_count")
            .ok_or_else(|| TaskExecutionFailed("retry_count not found in context".into()))?;

        info!(
            "Validating answer (attempt {} of {})",
            retry_count + 1,
            MAX_RETRIES + 1
        );

        let prompt = format!(
            r#"
            You are a movie recommendation evaluator.
            Evaluate the following recommendation against the user query.
            Guidelines:
            1 - A good recommendation is relevant to the user query.
            2 - A good recommendation is reasoned.
            3 - A good recommendation includes what the user asked for, and excludes what the user did not ask for.
            4 - If the recommendation is not good, explain why it is not good.
            5 - If the recommendation is good, explain why it is good.
            Respond **only** with JSON of the form \n{{ \"passed\": true/false, \"comment\": \"...\" }}.\n\n
            Query: {user_query}
            Answer: {answer}"#
        );

        let agent = get_llm_agent()
            .map_err(|e| TaskExecutionFailed(format!("Failed to initialize LLM agent: {}", e)))?;

        let raw = agent
            .chat(&prompt, vec![])
            .await
            .map_err(|e| TaskExecutionFailed(format!("LLM chat failed: {}", e)))?;

        // Clean JSON response (remove code blocks if present)
        let cleaned_raw = raw
            .trim()
            .strip_prefix("```json")
            .unwrap_or(&raw)
            .strip_suffix("```")
            .unwrap_or(&raw)
            .trim();

        let validation_result =
            serde_json::from_str::<ValidationResult>(cleaned_raw).map_err(|e| {
                TaskExecutionFailed(format!(
                    "Could not parse validator response: {}. Raw response: {}",
                    e, raw
                ))
            })?;

        context
            .set("validation_passed", &validation_result.passed)
            .await;
        if validation_result.passed {
            info!("Validation passed");
            return Ok(TaskResult::new(None, NextAction::ContinueAndExecute));
        }

        // if we are here, the validation failed - first we get the comment
        if validation_result.comment.is_none() {
            // something went wrong, we should not continue
            return Err(TaskExecutionFailed("No validation comment".into()));
        }
        let comment = validation_result.comment.clone().unwrap();
        info!(comment = %comment, "Validation failed");

        // first we check if we are above the max retries
        if retry_count >= MAX_RETRIES {
            error!(
                "Maximum retry attempts ({}) exceeded. Failing the workflow.",
                MAX_RETRIES
            );
            return Err(TaskExecutionFailed(format!(
                "Maximum retry attempts ({}) exceeded. Last validation comment: {:?}",
                MAX_RETRIES, &validation_result.comment
            )));
        }

        // we still have another chance to try
        // add the comment to the chat history with a explanation of what went wrong
        let validation_message = format!("The answer is not good enough. Reason: {}", comment);
        context.add_user_message(validation_message).await;

        // Increment retry count for the next attempt
        context.set("retry_count", retry_count + 1).await;
        Ok(TaskResult::new(None, NextAction::ContinueAndExecute))
    }
} 