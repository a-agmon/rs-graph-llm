use async_trait::async_trait;
use graph_flow::{Context, GraphError, NextAction, Result, Task, TaskResult};
use rig::completion::Chat;
use serde::Deserialize;
use tracing::info;

use crate::{chat_bridge::ContextRigExt, tasks::session_keys};

use super::{types::ClaimDetails, utils::get_llm_agent};

#[derive(Deserialize)]
struct ApartmentDetailsResponse {
    description: String,
    estimated_cost: f64,
    additional_info: Option<String>,
}

const APARTMENT_INSURANCE_DETAILS_PROMPT: &str = r#"You are an apartment/home insurance claims specialist. Help the user provide complete details about their apartment insurance claim.

You need to collect:
1. DESCRIPTION: Detailed description of what happened (damage, theft, fire, flood, etc.)
2. ESTIMATED COST: The estimated cost for repairs or replacement

WHEN YOU HAVE COMPLETE INFORMATION, respond with ONLY this JSON:
{
  "description": "detailed description of the incident",
  "estimated_cost": 2500.00,
  "additional_info": "any extra relevant details"
}

GUIDELINES:
- Ask specific questions about the property damage/loss
- Help them estimate repair/replacement costs if they're unsure
- Be thorough but efficient
- Ask about: what happened, when, extent of damage, affected items/areas
- Common apartment claims: water damage, fire, theft, vandalism, storm damage

IF MISSING INFO, ask clear questions to get what's needed for the claim.
Do not mix text and JSON in your response. If you know the type, respond with the JSON format above ONLY.
"#;

/// Attempts to parse apartment insurance details from LLM response
fn parse_apartment_details_from_response(response: &str) -> Option<(String, f64, Option<String>)> {
    let parsed = serde_json::from_str::<ApartmentDetailsResponse>(response.trim()).ok()?;
    info!("Parsed apartment details: desc={}, cost={}", parsed.description, parsed.estimated_cost);
    Some((parsed.description, parsed.estimated_cost, parsed.additional_info))
}

/// Task that collects detailed information for apartment insurance claims
pub struct ApartmentInsuranceDetailsTask;

#[async_trait]
impl Task for ApartmentInsuranceDetailsTask {
    fn id(&self) -> &str {
        std::any::type_name::<Self>()
    }

    async fn run(&self, context: Context) -> Result<TaskResult> {
        info!("running task: {}", self.id());

        let user_input: String = context
            .get(session_keys::USER_INPUT)
            .await
            .ok_or_else(|| GraphError::ContextError("user_input not found".to_string()))?;

        info!("Collecting apartment insurance details from input: {}", user_input);

        // Get message history from context in rig format
        let chat_history = context.get_rig_messages().await;
        // Create agent with apartment details collection prompt
        let agent = get_llm_agent(APARTMENT_INSURANCE_DETAILS_PROMPT)?;

        // Use chat to get response with history
        let response = agent
            .chat(&user_input, chat_history)
            .await
            .map_err(|e| GraphError::TaskExecutionFailed(e.to_string()))?;

        // Add user message and assistant response to chat history
        context.add_user_message(user_input.clone()).await;


        // Try to parse details from response
        if let Some((description, estimated_cost, additional_info)) = parse_apartment_details_from_response(&response) {
            // Get existing claim details and update them
            let mut claim_details: ClaimDetails = context
                .get(session_keys::CLAIM_DETAILS)
                .await
                .unwrap_or_default();

            claim_details.description = Some(description.clone());
            claim_details.estimated_cost = Some(estimated_cost);
            claim_details.additional_info = additional_info.clone();

            // Store updated claim details
            context
                .set(session_keys::CLAIM_DETAILS, claim_details)
                .await;

            let status_message = format!(
                "Apartment insurance details collected - Description: {}, Cost: ${:.2} - proceeding to validation",
                description, estimated_cost
            );

            return Ok(TaskResult::new_with_status(
                None,
                NextAction::ContinueAndExecute,
                Some(status_message),
            ));
        }

        context.add_assistant_message(response.clone()).await;
        // If we don't have complete details, the response should be a guiding question
        let status_message = "Collecting apartment insurance details - waiting for complete description and cost estimate".to_string();
        Ok(TaskResult::new_with_status(
            Some(response),
            NextAction::WaitForInput,
            Some(status_message),
        ))
    }
}