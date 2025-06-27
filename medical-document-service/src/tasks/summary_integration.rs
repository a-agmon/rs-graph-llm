use super::utils::get_llm_agent;
use crate::models::MedicalDocument;
use async_trait::async_trait;
use graph_flow::{Context, GraphError, NextAction, Result, Task, TaskResult};
use rig::completion::Prompt;
use tracing::{error, info};

pub struct SummaryIntegrationTask;

#[async_trait]
impl Task for SummaryIntegrationTask {
    async fn run(&self, context: Context) -> Result<TaskResult> {
        info!("Starting summary integration with human feedback");

        let document: MedicalDocument = context
            .get("document")
            .await
            .ok_or_else(|| GraphError::ContextError("Document not found in context".to_string()))?;

        let initial_summary = document
            .initial_summary
            .as_ref()
            .ok_or_else(|| GraphError::ContextError("Initial summary not found".to_string()))?;

        let human_feedback = document
            .human_feedback
            .as_ref()
            .ok_or_else(|| GraphError::ContextError("Human feedback not found".to_string()))?;

        info!("Integrating human feedback with initial summary");

        // Use LLM to integrate human feedback with initial summary
        let integrated_summary =
            match integrate_feedback_with_summary(initial_summary, human_feedback).await {
                Ok(summary) => summary,
                Err(e) => {
                    error!("Failed to integrate feedback: {}", e);
                    return Err(GraphError::TaskExecutionFailed(format!(
                        "Summary integration failed: {}",
                        e
                    )));
                }
            };

        // Update document with integrated summary
        let mut updated_document = document;
        updated_document.integrated_summary = Some(integrated_summary);
        context.set("document", updated_document).await;

        info!("Summary integration completed successfully");

        Ok(TaskResult::new_with_status(
            None,
            NextAction::ContinueAndExecute,
            Some("Human feedback integrated with initial summary".to_string()),
        ))
    }
}

async fn integrate_feedback_with_summary(
    initial_summary: &str,
    human_feedback: &str,
) -> anyhow::Result<String> {
    let prompt = format!(
        "You are a medical AI assistant. Please integrate the human feedback into the initial medical summary to create an improved, comprehensive summary.

        Guidelines:
        1. Incorporate all relevant feedback and corrections
        2. Maintain clinical accuracy and medical terminology
        3. Preserve the original structure but enhance based on feedback
        4. Address any specific concerns or questions raised in the feedback
        5. Keep the same section headers but improve content quality

        Initial Summary:
        {}

        Human Feedback:
        {}

        Please provide the integrated summary that incorporates the feedback while maintaining medical accuracy:",
                initial_summary,
                human_feedback
            );

    let agent = get_llm_agent(
        "You are a medical AI assistant specializing in document analysis and human feedback integration.",
    )?;
    let response = agent.prompt(&prompt).await?;
    Ok(response)
}
