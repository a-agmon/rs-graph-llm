use async_trait::async_trait;
use graph_flow::{Context, GraphError, NextAction, Result, Task, TaskResult};
use tracing::info;

use super::{types::UserDetails, utils::fetch_account_details};

/// Task that fetches account details using the collected user information
pub struct FetchAccountDetailsTask {
    id: String,
}

impl FetchAccountDetailsTask {
    pub fn new() -> Self {
        Self {
            id: "fetch_account_details".to_string(),
        }
    }
}

#[async_trait]
impl Task for FetchAccountDetailsTask {
    fn id(&self) -> &str {
        &self.id
    }

    async fn run(&self, context: Context) -> Result<TaskResult> {
        info!("running task: {}", self.id);
        let user_details: UserDetails = context
            .get("user_details")
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
            .set("account_details", account_details.clone())
            .await;

        Ok(TaskResult::new(
            Some(format!(
                "Account details retrieved successfully! Your {} account ending in {} has a balance of ${:.2}. How can I help you today?",
                account_details.account_type,
                &bank_number[bank_number.len() - 4..],
                account_details.account_balance
            )),
            NextAction::Continue,
        ))
    }
}
