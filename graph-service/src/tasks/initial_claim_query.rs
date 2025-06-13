use async_trait::async_trait;
use graph_flow::{Context, GraphError, NextAction, Result, Task, TaskResult};
use rig::completion::Prompt;
use tracing::info;

use crate::tasks::session_keys;

use super::{types::ClaimDetails, utils::get_llm_agent};

const INITIAL_CLAIM_PROMPT: &str = r#"You are a helpful insurance claims assistant. Welcome the user and help them start their insurance claim process.

Your goal is to:
1. Greet the user warmly and explain that you'll help them with their insurance claim
2. Ask them to briefly describe what happened that led to their claim
3. Gather initial information about their situation

Be friendly, professional, and reassuring. Let them know this is the beginning of the claims process and you're here to guide them through it step by step.

If they provide initial claim information, acknowledge it and let them know you'll help them provide more details in the next steps."#;

/// Task that handles the initial claim query and welcomes the user
pub struct InitialClaimQueryTask;

#[async_trait]
impl Task for InitialClaimQueryTask {
    fn id(&self) -> &str {
        std::any::type_name::<Self>()
    }

    async fn run(&self, context: Context) -> Result<TaskResult> {
        info!("running task: {}", self.id());
        
        let user_input: String = context
            .get(session_keys::USER_INPUT)
            .await
            .ok_or_else(|| GraphError::ContextError("user_input not found".to_string()))?;

        info!("Processing initial claim query: {}", user_input);

        // Use LLM to welcome the user and gather initial information
        let response = process_initial_claim(&user_input)
            .await
            .map_err(|e| GraphError::TaskExecutionFailed(e.to_string()))?;

        // Initialize claim details in context
        let claim_details = ClaimDetails::default();
        context
            .set(session_keys::CLAIM_DETAILS, claim_details)
            .await;

        // Add user message and assistant response to chat history
        context.add_user_message(user_input.clone()).await;
        context.add_assistant_message(response.clone()).await;

        Ok(TaskResult::new_with_status(
            Some(response),
            NextAction::Continue,
            Some("Claim processing started - proceeding to insurance type classification".to_string()),
        ))
    }
}

async fn process_initial_claim(user_input: &str) -> anyhow::Result<String> {
    let agent = get_llm_agent(INITIAL_CLAIM_PROMPT)?;
    let response = agent.prompt(user_input).await?;
    Ok(response)
}