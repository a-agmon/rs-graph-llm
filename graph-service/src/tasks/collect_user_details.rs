use async_trait::async_trait;
use graph_flow::{Context, GraphError, NextAction, Result, Task, TaskResult};
use rig::completion::{Chat, Message};
use tracing::info;

use crate::tasks::session_keys;

use super::{types::UserDetails, utils::get_llm_agent};

const COLLECT_USER_DETAILS_PROMPT: &str = r#"You are a banking assistant collecting username and bank number.

ANALYZE THE CONVERSATION HISTORY AND EXTRACT:
- Username: any name the user provides
- Bank number: any number sequence (8-15 digits) the user provides

WHEN USER SAYS:
- "My username is john" → username = "john"
- "My number 1234567891" → bank_number = "1234567891"
- "My bank number is 9876543210" → bank_number = "9876543210"
- "The number is 1122334455" → bank_number = "1122334455"

IF YOU HAVE BOTH username AND bank_number, respond with ONLY this JSON:
{
  "username": "extracted_username",
  "bank_number": "extracted_number"
}

IF MISSING INFO, ask for what's needed.
"#;

/// Task that collects user details (username and bank number)
/// May require multiple interactions if user provides incomplete information
pub struct CollectUserDetailsTask;

#[async_trait]
impl Task for CollectUserDetailsTask {
    fn id(&self) -> &str {
        std::any::type_name::<Self>()
    }

    async fn run(&self, context: Context) -> Result<TaskResult> {
        info!("running task: {}", self.id());

        let user_input: String = context
            .get(session_keys::USER_INPUT)
            .await
            .ok_or_else(|| GraphError::ContextError("user_query not found".to_string()))?;

        info!("Collecting user details from input: {}", user_input);

        // Get message history from context (if any)
        let mut chat_history: Vec<Message> = context
            .get(session_keys::CHAT_HISTORY)
            .await
            .unwrap_or_default();

        // Create agent with collection prompt
        let agent = get_llm_agent(COLLECT_USER_DETAILS_PROMPT)?;

        // Use chat to get response with history
        let response = agent
            .chat(&user_input, chat_history.clone())
            .await
            .map_err(|e| GraphError::TaskExecutionFailed(e.to_string()))?;

        // Add user message and assistant response to history
        chat_history.push(Message::user(user_input));
        chat_history.push(Message::assistant(response.clone()));

        // Store updated chat history
        context.set(session_keys::CHAT_HISTORY, chat_history).await;

        // Try to parse JSON from response to check if we have complete details
        if let Ok(user_details) = serde_json::from_str::<UserDetails>(&response) {
            if user_details.username.is_some() && user_details.bank_number.is_some() {
                // We have complete details, store them and continue
                context
                    .set(session_keys::USER_DETAILS, user_details.clone())
                    .await;
                info!(
                    "All user details collected: {:?} - {:?}",
                    user_details.username, user_details.bank_number
                );

                return Ok(TaskResult::new(None, NextAction::ContinueAndExecute));
            }
        }

        // If we don't have complete details or couldn't parse JSON,
        // the response should be a guiding question
        Ok(TaskResult::new(Some(response), NextAction::WaitForInput))
    }
}
