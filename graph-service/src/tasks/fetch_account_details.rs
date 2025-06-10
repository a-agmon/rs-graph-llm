use async_trait::async_trait;
use graph_flow::{Context, GraphError, NextAction, Result, Task, TaskResult};
use tracing::info;

use crate::tasks::session_keys;

use super::{types::UserDetails, utils::fetch_account_details};

/// Task that fetches account details using the collected user information
pub struct FetchAccountDetailsTask;

#[async_trait]
impl Task for FetchAccountDetailsTask {
    fn id(&self) -> &str {
        std::any::type_name::<Self>()
    }

    async fn run(&self, context: Context) -> Result<TaskResult> {
        info!("running task: {}", self.id());

        let user_details: UserDetails = context
            .get(session_keys::USER_DETAILS)
            .await
            .ok_or_else(|| GraphError::ContextError("user_details not found".to_string()))?;

        let username = user_details.username.ok_or_else(|| {
            GraphError::ContextError("username not found in user_details".to_string())
        })?;
        let bank_number = user_details.bank_number.ok_or_else(|| {
            GraphError::ContextError("bank_number not found in user_details".to_string())
        })?;

        info!(
            "Fetching account details for: {} - {}",
            username, bank_number
        );

        // Simulate fetching account details from a banking API
        let account_details = fetch_account_details(&username, &bank_number)
            .await
            .map_err(|e| GraphError::TaskExecutionFailed(e.to_string()))?;

        // Store account details in context
        context
            .set(session_keys::ACCOUNT_DETAILS, account_details.clone())
            .await;

        let response = format!(
            "Account details retrieved successfully! Your {} account ending in {} has a balance of ${:.2}. How can I help you today?",
            account_details.account_type,
            &bank_number[bank_number.len() - 4..],
            account_details.account_balance
        );

        let status_message = format!(
            "Successfully fetched account details for user {} - {} account with balance ${:.2}",
            username,
            account_details.account_type,
            account_details.account_balance
        );

        Ok(TaskResult::new_with_status(Some(response), NextAction::Continue, Some(status_message)))
    }
}
