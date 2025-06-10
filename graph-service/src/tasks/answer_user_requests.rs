use async_trait::async_trait;
use graph_flow::{Context, GraphError, NextAction, Result, Task, TaskResult};
use rig::completion::Prompt;
use tracing::info;

use crate::tasks::session_keys;

use super::{types::AccountDetails, utils::get_llm_agent};

const ANSWER_REQUEST_PROMPT: &str = r#"You are a helpful banking assistant. Answer the user's question about their account using the provided account details.
Be friendly, professional, and provide accurate information based on the account data.
If the user asks about something not available in the account details, politely explain what information you have access to."#;

/// Task that answers user requests about their account
pub struct AnswerUserRequestsTask;

#[async_trait]
impl Task for AnswerUserRequestsTask {
    fn id(&self) -> &str {
        std::any::type_name::<Self>()
    }

    async fn run(&self, context: Context) -> Result<TaskResult> {
        info!("running task: {}", self.id());
        let user_query: String = context
            .get(session_keys::USER_INPUT)
            .await
            .ok_or_else(|| GraphError::ContextError("user_query not found".to_string()))?;

        let account_details: AccountDetails = context
            .get(session_keys::ACCOUNT_DETAILS)
            .await
            .ok_or_else(|| GraphError::ContextError("account_details not found".to_string()))?;

        info!("Answering user request: {}", user_query);

        // Use LLM to answer the user's question about their account
        let response = answer_user_request(&user_query, &account_details)
            .await
            .map_err(|e| GraphError::TaskExecutionFailed(e.to_string()))?;

        Ok(TaskResult::new(
            Some(response),
            NextAction::WaitForInput, // Keep the conversation going
        ))
    }
}

async fn answer_user_request(
    user_query: &str,
    account_details: &AccountDetails,
) -> anyhow::Result<String> {
    let agent = get_llm_agent(ANSWER_REQUEST_PROMPT)?;
    let context = format!(
        "Account Details:
        - Username: {}
        - Account Type: {}
        - Balance: ${:.2}
        - Last Transaction: {}

        User Question: {}",
        account_details.username,
        account_details.account_type,
        account_details.account_balance,
        account_details.last_transaction,
        user_query
    );

    let response = agent.prompt(&context).await?;
    Ok(response)
}
