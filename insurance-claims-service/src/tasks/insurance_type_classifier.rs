use async_trait::async_trait;
use graph_flow::{Context, GraphError, NextAction, Result, Task, TaskResult};
use rig::completion::Chat;
use serde::Deserialize;
use tracing::info;

use crate::tasks::session_keys;

use super::{types::ClaimDetails, utils::get_llm_agent};

#[derive(Deserialize)]
struct InsuranceTypeResponse {
    insurance_type: String,
}

const INSURANCE_TYPE_PROMPT: &str = r#"You are an insurance claims assistant specialized in determining the type of insurance claim.

ANALYZE THE CONVERSATION HISTORY AND DETERMINE:
- Is this a CAR insurance claim (auto, vehicle, collision, etc.)?
- Is this an APARTMENT insurance claim (home, property, renters, etc.)?

IF YOU CAN CLEARLY DETERMINE THE INSURANCE TYPE, respond with ONLY this JSON:
{
  "insurance_type": "car"
}
OR
{
  "insurance_type": "apartment"
}

IF UNCLEAR, ask a clarifying question to determine which type of insurance this claim relates to.
Be specific and helpful in your questions.
Do not mix text and JSON in your response. If you know the type, respond with the JSON format above ONLY.
"#;

/// Attempts to parse insurance type from LLM response
fn parse_insurance_type_from_response(response: &str) -> Option<String> {
    let parsed = serde_json::from_str::<InsuranceTypeResponse>(response.trim()).ok()?;
    if parsed.insurance_type == "car" || parsed.insurance_type == "apartment" {
        info!("Parsed insurance type: {}", parsed.insurance_type);
        Some(parsed.insurance_type)
    } else {
        None
    }
}

/// Task that determines whether this is a car or apartment insurance claim
pub struct InsuranceTypeClassifierTask;

#[async_trait]
impl Task for InsuranceTypeClassifierTask {
    fn id(&self) -> &str {
        std::any::type_name::<Self>()
    }

    async fn run(&self, context: Context) -> Result<TaskResult> {
        let session_id = context
            .get::<String>("session_id")
            .await
            .unwrap_or_else(|| "unknown".to_string());

        info!(
            session_id = %session_id,
            task_id = %self.id(),
            "Starting insurance type classification"
        );

        let user_input: String = context
            .get(session_keys::USER_INPUT)
            .await
            .ok_or_else(|| GraphError::ContextError("user_input not found".to_string()))?;

        // Get message history from context in rig format
        let chat_history = context.get_rig_messages().await;
        context.add_user_message(user_input.clone()).await;

        // Create agent with classification prompt
        let agent = get_llm_agent(INSURANCE_TYPE_PROMPT)?;

        // Use chat to get response with history
        let response = agent
            .chat(&user_input, chat_history)
            .await
            .map_err(|e| GraphError::TaskExecutionFailed(e.to_string()))?;

        // Try to parse insurance type from response
        if let Some(insurance_type) = parse_insurance_type_from_response(&response) {
            info!("Insurance type determined: {}", insurance_type);

            // Store insurance type in session
            context
                .set(session_keys::INSURANCE_TYPE, insurance_type.clone())
                .await;

            // Update claim details with insurance type
            let mut claim_details: ClaimDetails = context
                .get(session_keys::CLAIM_DETAILS)
                .await
                .unwrap_or_default();
            claim_details.insurance_type = Some(insurance_type.clone());
            context
                .set(session_keys::CLAIM_DETAILS, claim_details)
                .await;

            let status_message = format!(
                "Insurance type classified as: {} - proceeding to collect specific details",
                insurance_type
            );

            info!(
                task_id = %self.id(),
                insurance_type = %insurance_type,
                next_step = "collect_details",
                "Classification complete, proceeding to details collection"
            );

            return Ok(TaskResult::new_with_status(
                None,
                NextAction::ContinueAndExecute,
                Some(status_message),
            ));
        }

        // If we couldn't determine the type, the response should be a clarifying question
        context.add_assistant_message(response.clone()).await;
        let status_message =
            "Waiting for insurance type classification - need more information".to_string();
        Ok(TaskResult::new_with_status(
            Some(response),
            NextAction::WaitForInput,
            Some(status_message),
        ))
    }
}
