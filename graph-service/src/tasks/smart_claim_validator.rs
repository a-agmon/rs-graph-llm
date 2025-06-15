use async_trait::async_trait;
use graph_flow::{Context, GraphError, NextAction, Result, Task, TaskResult};
use tracing::info;

use crate::tasks::session_keys;

use super::types::{ClaimDetails, ClaimDecision};

/// Simple task that checks claim amount and routes based on $1000 threshold
pub struct SmartClaimValidatorTask;

#[async_trait]
impl Task for SmartClaimValidatorTask {
    fn id(&self) -> &str {
        std::any::type_name::<Self>()
    }

    async fn run(&self, context: Context) -> Result<TaskResult> {
        let session_id = context.get::<String>("session_id").await.unwrap_or_else(|| "unknown".to_string());
        
        info!(
            session_id = %session_id,
            task_id = %self.id(),
            "Starting claim validation task"
        );

        // Check if we're waiting for approval
        let approval_state: Option<String> = context
            .get(session_keys::APPROVAL_STATE)
            .await;

        if let Some("pending") = approval_state.as_deref() {
            // We're waiting for approval, process the user input
            let user_input: String = context
                .get(session_keys::USER_INPUT)
                .await
                .ok_or_else(|| GraphError::ContextError("user_input not found".to_string()))?;
            
            info!(
                session_id = %session_id,
                task_id = %self.id(),
                "Processing approval decision from user"
            );
            
            return self.handle_approval_decision(context, user_input).await;
        }

        // Get claim details
        let claim_details: ClaimDetails = context
            .get(session_keys::CLAIM_DETAILS)
            .await
            .ok_or_else(|| GraphError::ContextError("claim_details not found".to_string()))?;

        let claim_amount = claim_details.estimated_cost.unwrap_or(0.0);
        
        info!(
            session_id = %session_id,
            task_id = %self.id(),
            claim_amount = %claim_amount,
            threshold = 1000.0,
            "Processing claim validation"
        );

        if claim_amount < 1000.0 {
            // Auto-approve for amounts under $1000
            let decision = ClaimDecision {
                approved: true,
                decision_reason: "Auto-approved: claim amount under $1000 threshold".to_string(),
                timestamp: chrono::Utc::now().to_rfc3339(),
            };

            context.set(session_keys::CLAIM_DECISION, decision).await;

            let status_message = format!(
                "Claim auto-approved - Amount: ${:.2} (under $1000) - proceeding to final summary",
                claim_amount
            );

            info!(
                session_id = %session_id,
                task_id = %self.id(),
                claim_amount = %claim_amount,
                decision = "auto_approved",
                reason = "under_threshold",
                "Claim automatically approved"
            );

            Ok(TaskResult::new_with_status(
                Some(String::from("Your claim has been auto-approved. Do you want to proceed to the final summary?")),
                NextAction::Continue, 
                Some(status_message),
            ))
        } else {
            // Requires manual approval for amounts $1000 and above
            context.set(session_keys::APPROVAL_STATE, "pending".to_string()).await;

            let insurance_type = claim_details.insurance_type.as_deref().unwrap_or("insurance");
            
            let approval_request = format!(
                "Your {} claim for ${:.2} requires approval. Please respond with 'approved' to approve this claim.",
                insurance_type,
                claim_amount
            );

            let status_message = format!(
                "Manual approval required - Amount: ${:.2} (over $1000) - waiting for approval decision",
                claim_amount
            );

            info!(
                session_id = %session_id,
                task_id = %self.id(),
                claim_amount = %claim_amount,
                decision = "manual_review_required",
                reason = "over_threshold",
                "Claim requires manual approval"
            );

            Ok(TaskResult::new_with_status(
                Some(approval_request),
                NextAction::WaitForInput,
                Some(status_message),
            ))
        }
    }
}

impl SmartClaimValidatorTask {
    async fn handle_approval_decision(&self, context: Context, user_input: String) -> Result<TaskResult> {
        let input_lower = user_input.to_lowercase();
        let approved = input_lower.contains("approved");

        if approved {
            let decision = ClaimDecision {
                approved: true,
                decision_reason: "Claim approved by manual review".to_string(),
                timestamp: chrono::Utc::now().to_rfc3339(),
            };

            context.set(session_keys::CLAIM_DECISION, decision).await;
            context.set(session_keys::APPROVAL_STATE, "completed".to_string()).await;

            let status_message = "Manual approval received - proceeding to final summary".to_string();
            info!("{}", status_message);

            Ok(TaskResult::new_with_status(
                None,
                NextAction::Continue,
                Some(status_message),
            ))
        } else {
            let status_message = "Waiting for approval decision - please respond with 'approved' to approve".to_string();
            Ok(TaskResult::new_with_status(
                Some("Waiting for approval decision.".to_string()),
                NextAction::WaitForInput,
                Some(status_message),
            ))
        }
    }
}