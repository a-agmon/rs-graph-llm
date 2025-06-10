use async_trait::async_trait;
use graph_flow::{Context, GraphError, NextAction, Result, Task, TaskResult};
use rig::completion::Chat;
use tracing::info;

use crate::{chat_bridge::ContextRigExt, tasks::session_keys};

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

/// Attempts to parse UserDetails from LLM response
/// First tries direct JSON parsing, then extracts JSON block if needed
fn parse_user_details_from_response(response: &str) -> Option<UserDetails> {
    // Try parsing entire response as JSON first
    if let Ok(details) = serde_json::from_str::<UserDetails>(response) {
        info!("Parsed response as direct JSON: {:?}", details);
        return Some(details);
    }

    // Extract JSON block from response if direct parsing fails
    let start = response.find('{')?;
    let end = response.rfind('}')?;
    let json_str = &response[start..=end];
    
    match serde_json::from_str::<UserDetails>(json_str) {
        Ok(details) => {
            info!("Extracted and parsed JSON from response: {:?}", details);
            Some(details)
        }
        Err(e) => {
            info!("Failed to parse JSON from response: {}", e);
            None
        }
    }
}

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

        // Get message history from context in rig format
        let chat_history = context.get_rig_messages().await;

        // Create agent with collection prompt
        let agent = get_llm_agent(COLLECT_USER_DETAILS_PROMPT)?;

        // Use chat to get response with history
        let response = agent
            .chat(&user_input, chat_history)
            .await
            .map_err(|e| GraphError::TaskExecutionFailed(e.to_string()))?;

        // Add user message and assistant response to chat history
        context.add_user_message(user_input.clone()).await;
        context.add_assistant_message(response.clone()).await;

        // Try to parse JSON from response to check if we have complete details
        let user_details = parse_user_details_from_response(&response);

        if let Some(user_details) = user_details {
            info!("Checking if details are complete: username={:?}, bank_number={:?}",
                  user_details.username, user_details.bank_number);
            if user_details.username.is_some() && user_details.bank_number.is_some() {
                // We have complete details, store them and continue
                context
                    .set(session_keys::USER_DETAILS, user_details.clone())
                    .await;
                info!(
                    "All user details collected: {:?} - {:?}",
                    user_details.username, user_details.bank_number
                );

                let status_message = format!(
                    "User details collection completed - Username: {}, Bank number: {}",
                    user_details.username.as_ref().unwrap(),
                    user_details.bank_number.as_ref().unwrap()
                );

                info!("Moving to next task with status: {}", status_message);
                return Ok(TaskResult::new_with_status(None, NextAction::ContinueAndExecute, Some(status_message)));
            } else {
                info!("Details incomplete, staying in collection phase");
            }
        } else {
            info!("No valid user details found in response");
        }

        // If we don't have complete details or couldn't parse JSON,
        // the response should be a guiding question
        let status_message = "Collecting user details - waiting for complete username and bank number".to_string();
        Ok(TaskResult::new_with_status(Some(response), NextAction::WaitForInput, Some(status_message)))
    }
}
