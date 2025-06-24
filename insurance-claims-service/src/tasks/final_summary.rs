use async_trait::async_trait;
use graph_flow::{Context, GraphError, NextAction, Result, Task, TaskResult};
use tracing::info;

use crate::tasks::session_keys;

use super::types::{ClaimDetails, ClaimDecision};

/// Single endpoint task for all claim outcomes (approved/rejected)
pub struct FinalSummaryTask;

#[async_trait]
impl Task for FinalSummaryTask {

    async fn run(&self, context: Context) -> Result<TaskResult> {
        info!("running task: {}", self.id());

        let claim_details: ClaimDetails = context
            .get(session_keys::CLAIM_DETAILS)
            .await
            .ok_or_else(|| GraphError::ContextError("claim_details not found".to_string()))?;

        let claim_decision: ClaimDecision = context
            .get(session_keys::CLAIM_DECISION)
            .await
            .ok_or_else(|| GraphError::ContextError("claim_decision not found".to_string()))?;

        let insurance_type = claim_details.insurance_type.as_deref().unwrap_or("unknown");
        let description = claim_details.description.as_deref().unwrap_or("No description provided");
        let additional_info = claim_details.additional_info.as_deref().unwrap_or("");
        let claim_amount = claim_details.estimated_cost.unwrap_or(0.0);

        let summary = if claim_decision.approved {
            // Generate approved summary
            info!("Generating approved summary for amount: ${:.2}", claim_amount);
            
            format!(
                "🎉 **CLAIM APPROVED** 🎉

Your {} insurance claim has been **APPROVED**!

**Claim Details:**
- Type: {} Insurance
- Amount: ${:.2}
- Description: {}
{}

**Approval Information:**
- Status: ✅ APPROVED
- Decision: {}
- Reference Number: CLM-{:08X}
- Approval Date: {}

**Next Steps:**
✅ Your claim has been processed and approved
✅ Payment will be initiated within 1-3 business days
✅ You will receive a confirmation email with all details
✅ Keep this reference number for your records

**Payment Details:**
- Approved Amount: ${:.2}
- Processing Time: 1-3 business days
- Payment Method: Direct deposit to registered account

**Contact Information:**
If you have any questions about your claim, please reference number CLM-{:08X} when contacting our support team.

Thank you for choosing our insurance services!",
                insurance_type,
                insurance_type.to_uppercase(),
                claim_amount,
                description,
                if !additional_info.is_empty() { format!("\n- Additional Info: {}", additional_info) } else { String::new() },
                claim_decision.decision_reason,
                rand::random::<u32>(),
                chrono::Utc::now().format("%Y-%m-%d %H:%M UTC"),
                claim_amount,
                rand::random::<u32>()
            )
        } else {
            // Generate rejected summary
            info!("Generating rejected summary for amount: ${:.2}", claim_amount);
            
            format!(
                "❌ **CLAIM REJECTED** ❌

Your {} insurance claim has been **REJECTED**.

**Claim Details:**
- Type: {} Insurance
- Amount: ${:.2}
- Description: {}
{}

**Rejection Information:**
- Status: ❌ REJECTED
- Reason: {}
- Reference Number: CLM-{:08X}
- Decision Date: {}

**Next Steps:**
📞 **Appeal Process:**
- You may appeal this decision within 30 days
- Contact our appeals department at 1-800-APPEALS
- Reference number CLM-{:08X} when calling

📋 **Additional Documentation:**
- If you have additional evidence or documentation
- You may submit a new claim with supporting materials
- Our team will review any new information provided

📧 **Documentation Required for Appeal:**
- Additional proof of damage/loss
- Professional assessments or estimates
- Photos or other supporting evidence
- Any relevant receipts or documentation

**Contact Information:**
- Appeals Department: 1-800-APPEALS
- Email: appeals@insurance.com
- Reference Number: CLM-{:08X}

We understand this may be disappointing. Our decision was made after careful review of all available information. If you believe this decision was made in error, please don't hesitate to contact our appeals department.

Thank you for choosing our insurance services.",
                insurance_type,
                insurance_type.to_uppercase(),
                claim_amount,
                description,
                if !additional_info.is_empty() { format!("\n- Additional Info: {}", additional_info) } else { String::new() },
                claim_decision.decision_reason,
                rand::random::<u32>(),
                chrono::Utc::now().format("%Y-%m-%d %H:%M UTC"),
                rand::random::<u32>(),
                rand::random::<u32>()
            )
        };

        let status_message = format!(
            "Claim processing completed - {} insurance claim {} for ${:.2}",
            insurance_type,
            if claim_decision.approved { "APPROVED" } else { "REJECTED" },
            claim_amount
        );

        Ok(TaskResult::new_with_status(
            Some(summary),
            NextAction::End,
            Some(status_message),
        ))
    }
}