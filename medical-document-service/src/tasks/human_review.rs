use crate::models::MedicalDocument;
use async_trait::async_trait;
use graph_flow::{Context, GraphError, NextAction, Result, Task, TaskResult};
use tracing::{error, info};

pub struct HumanReviewTask;

#[async_trait]
impl Task for HumanReviewTask {
    async fn run(&self, context: Context) -> Result<TaskResult> {
        info!("Starting human review checkpoint");

        let document: MedicalDocument = context
            .get("document")
            .await
            .ok_or_else(|| GraphError::ContextError("Document not found in context".to_string()))?;

        // Check if we have an initial summary to review
        if document.initial_summary.is_none() {
            error!("No initial summary available for human review");
            return Err(GraphError::TaskExecutionFailed(
                "Initial summary required for human review".to_string(),
            ));
        }

        // Check if human feedback has already been provided
        if let Some(feedback) = document.human_feedback {
            info!("Human feedback already provided: {}", feedback);

            return Ok(TaskResult::new_with_status(
                None,
                NextAction::ContinueAndExecute,
                Some("Human feedback received, proceeding to integration".to_string()),
            ));
        }
        // Store current state and wait for human input
        info!("Waiting for human review of initial summary");

        // Set the flag that the service looks for
        context.set("waiting_for_human_feedback", true).await;

        Ok(TaskResult::new_with_status(
            Some("Summary Ready, Waiting for Doctor Review".to_string()),
            NextAction::WaitForInput,
            Some("Please provide feedback on the initial summary".to_string()),
        ))
    }
}
