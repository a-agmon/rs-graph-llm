//! Example: Classifier Agent
//!
//! A multi-step agent that classifies input text into categories
//! with confidence scoring and conditional routing based on confidence.
//! Demonstrates conditional edges, typed state, and human-in-the-loop.
//!
//! Run with: `cargo run --bin classifier_agent`

use async_trait::async_trait;
use graph_flow::{
    Context, FlowRunner, GraphBuilder, InMemorySessionStorage, NextAction, Session,
    SessionStorage, Task, TaskResult,
};
use std::sync::Arc;

/// Preprocess task — normalizes and tokenizes input text.
struct PreprocessTask;

#[async_trait]
impl Task for PreprocessTask {
    fn id(&self) -> &str {
        "preprocess"
    }

    async fn run(&self, context: Context) -> graph_flow::Result<TaskResult> {
        let input: String = context
            .get("input_text")
            .await
            .unwrap_or_else(|| "".to_string());

        println!("[Preprocess] Processing: '{}'", &input[..input.len().min(50)]);

        let normalized = input.to_lowercase().trim().to_string();
        let tokens: Vec<String> = normalized.split_whitespace().map(String::from).collect();
        let token_count = tokens.len();

        context.set("normalized_text", normalized).await;
        context.set("tokens", tokens).await;
        context.set("token_count", token_count).await;

        Ok(TaskResult::new(
            Some(format!("Preprocessed: {} tokens", token_count)),
            NextAction::ContinueAndExecute,
        ))
    }
}

/// Classify task — assigns a category based on keyword matching.
struct ClassifyTask;

#[async_trait]
impl Task for ClassifyTask {
    fn id(&self) -> &str {
        "classify"
    }

    async fn run(&self, context: Context) -> graph_flow::Result<TaskResult> {
        let text: String = context.get("normalized_text").await.unwrap_or_default();

        println!("[Classify] Classifying text...");

        // Simple keyword-based classification
        let (category, confidence) = if text.contains("error") || text.contains("bug") || text.contains("crash") {
            ("bug_report", 0.92)
        } else if text.contains("feature") || text.contains("request") || text.contains("add") {
            ("feature_request", 0.88)
        } else if text.contains("help") || text.contains("how") || text.contains("question") {
            ("support_question", 0.85)
        } else if text.contains("great") || text.contains("thanks") || text.contains("love") {
            ("positive_feedback", 0.90)
        } else {
            ("uncategorized", 0.3)
        };

        context.set("category", category.to_string()).await;
        context.set("confidence", confidence).await;
        context.set("high_confidence", confidence >= 0.7).await;

        Ok(TaskResult::new(
            Some(format!(
                "Classified as '{}' (confidence: {:.0}%)",
                category,
                confidence * 100.0
            )),
            NextAction::Continue,
        ))
    }
}

/// Route task — routes to appropriate handler based on category.
struct RouteTask;

#[async_trait]
impl Task for RouteTask {
    fn id(&self) -> &str {
        "route"
    }

    async fn run(&self, context: Context) -> graph_flow::Result<TaskResult> {
        let category: String = context.get("category").await.unwrap_or_default();
        let confidence: f64 = context.get("confidence").await.unwrap_or(0.0);

        println!("[Route] Routing '{}' (conf: {:.0}%)", category, confidence * 100.0);

        let action = match category.as_str() {
            "bug_report" => "Create bug ticket in issue tracker",
            "feature_request" => "Add to feature backlog for review",
            "support_question" => "Route to support team queue",
            "positive_feedback" => "Archive and send thank-you",
            _ => "Flag for manual review",
        };

        context.set("routing_action", action.to_string()).await;

        Ok(TaskResult::new(
            Some(format!("Routed: {}", action)),
            NextAction::ContinueAndExecute,
        ))
    }
}

/// Manual review task — for low-confidence classifications.
struct ManualReviewTask;

#[async_trait]
impl Task for ManualReviewTask {
    fn id(&self) -> &str {
        "manual_review"
    }

    async fn run(&self, context: Context) -> graph_flow::Result<TaskResult> {
        let category: String = context.get("category").await.unwrap_or_default();
        let confidence: f64 = context.get("confidence").await.unwrap_or(0.0);

        println!(
            "[Manual Review] Low confidence ({:.0}%) for '{}' — flagging for review",
            confidence * 100.0,
            category
        );

        context.set("needs_review", true).await;
        context
            .set(
                "review_reason",
                format!(
                    "Low confidence classification: {} ({:.0}%)",
                    category,
                    confidence * 100.0
                ),
            )
            .await;

        Ok(TaskResult::new(
            Some(format!(
                "Flagged for manual review: {} ({:.0}% confidence)",
                category,
                confidence * 100.0
            )),
            NextAction::ContinueAndExecute,
        ))
    }
}

/// Output task — produces final classification result.
struct OutputTask;

#[async_trait]
impl Task for OutputTask {
    fn id(&self) -> &str {
        "output"
    }

    async fn run(&self, context: Context) -> graph_flow::Result<TaskResult> {
        let category: String = context.get("category").await.unwrap_or_default();
        let confidence: f64 = context.get("confidence").await.unwrap_or(0.0);
        let action: String = context.get("routing_action").await.unwrap_or_default();
        let needs_review: bool = context.get("needs_review").await.unwrap_or(false);
        let token_count: usize = context.get("token_count").await.unwrap_or(0);

        let result = serde_json::json!({
            "category": category,
            "confidence": confidence,
            "action": action,
            "needs_review": needs_review,
            "token_count": token_count,
        });

        context.set("classification_result", result.clone()).await;

        let output = format!(
            "Classification Result:\n\
             Category:    {}\n\
             Confidence:  {:.0}%\n\
             Action:      {}\n\
             Review:      {}",
            category,
            confidence * 100.0,
            action,
            if needs_review { "YES" } else { "No" }
        );

        println!("\n{}", output);

        Ok(TaskResult::new(Some(output), NextAction::End))
    }
}

#[tokio::main]
async fn main() -> graph_flow::Result<()> {
    println!("=== Classifier Agent Example ===\n");

    let preprocess = Arc::new(PreprocessTask) as Arc<dyn Task>;
    let classify = Arc::new(ClassifyTask) as Arc<dyn Task>;
    let route = Arc::new(RouteTask) as Arc<dyn Task>;
    let manual_review = Arc::new(ManualReviewTask) as Arc<dyn Task>;
    let output = Arc::new(OutputTask) as Arc<dyn Task>;

    let graph = Arc::new(
        GraphBuilder::new("classifier")
            .add_task(preprocess)
            .add_task(classify)
            .add_task(route)
            .add_task(manual_review)
            .add_task(output)
            // preprocess → classify
            .add_edge("preprocess", "classify")
            // classify → high confidence: route, low confidence: manual review
            .add_conditional_edge(
                "classify",
                |ctx| ctx.get_sync::<bool>("high_confidence").unwrap_or(false),
                "route",          // High confidence → auto-route
                "manual_review",  // Low confidence → manual review
            )
            // Both paths converge to output
            .add_edge("route", "output")
            .add_edge("manual_review", "output")
            .build(),
    );

    // Test with different inputs
    let test_inputs = vec![
        "There's a critical error when I try to save my work",
        "Could you add dark mode as a feature?",
        "How do I configure the settings?",
        "This tool is something I'm not sure about",
    ];

    let storage = Arc::new(InMemorySessionStorage::new());
    let runner = FlowRunner::new(graph, storage.clone());

    for (i, input) in test_inputs.iter().enumerate() {
        let session_id = format!("classify_{}", i);
        println!("\n--- Input {}: \"{}\" ---\n", i + 1, input);

        let session = Session::new_from_task(session_id.clone(), "preprocess");
        session.context.set("input_text", input.to_string()).await;
        storage.save(session).await?;

        loop {
            let result = runner.run(&session_id).await?;

            if let Some(response) = &result.response {
                println!("  >>> {}", response);
            }

            match result.status {
                graph_flow::ExecutionStatus::Completed => break,
                graph_flow::ExecutionStatus::Error(e) => {
                    println!("  ERROR: {}", e);
                    break;
                }
                _ => continue,
            }
        }
    }

    println!("\n=== All Classifications Complete ===");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn run_classifier(input: &str) -> (String, f64, bool) {
        let graph = Arc::new(
            GraphBuilder::new("classifier")
                .add_task(Arc::new(PreprocessTask) as Arc<dyn Task>)
                .add_task(Arc::new(ClassifyTask) as Arc<dyn Task>)
                .add_task(Arc::new(RouteTask) as Arc<dyn Task>)
                .add_task(Arc::new(ManualReviewTask) as Arc<dyn Task>)
                .add_task(Arc::new(OutputTask) as Arc<dyn Task>)
                .add_edge("preprocess", "classify")
                .add_conditional_edge(
                    "classify",
                    |ctx| ctx.get_sync::<bool>("high_confidence").unwrap_or(false),
                    "route",
                    "manual_review",
                )
                .add_edge("route", "output")
                .add_edge("manual_review", "output")
                .build(),
        );

        let storage = Arc::new(InMemorySessionStorage::new());
        let runner = FlowRunner::new(graph, storage.clone());

        let session = Session::new_from_task("test".to_string(), "preprocess");
        session.context.set("input_text", input.to_string()).await;
        storage.save(session).await.unwrap();

        for _ in 0..20 {
            let result = runner.run("test").await.unwrap();
            if matches!(result.status, graph_flow::ExecutionStatus::Completed) {
                break;
            }
        }

        let session = storage.get("test").await.unwrap().unwrap();
        let category: String = session.context.get("category").await.unwrap_or_default();
        let confidence: f64 = session.context.get("confidence").await.unwrap_or(0.0);
        let needs_review: bool = session.context.get("needs_review").await.unwrap_or(false);
        (category, confidence, needs_review)
    }

    #[tokio::test]
    async fn test_bug_report_classification() {
        let (category, confidence, needs_review) =
            run_classifier("There's an error in the application").await;
        assert_eq!(category, "bug_report");
        assert!(confidence > 0.8);
        assert!(!needs_review);
    }

    #[tokio::test]
    async fn test_feature_request_classification() {
        let (category, confidence, needs_review) =
            run_classifier("Please add a new feature for export").await;
        assert_eq!(category, "feature_request");
        assert!(confidence > 0.8);
        assert!(!needs_review);
    }

    #[tokio::test]
    async fn test_low_confidence_classification() {
        let (category, confidence, needs_review) =
            run_classifier("random text without keywords").await;
        assert_eq!(category, "uncategorized");
        assert!(confidence < 0.5);
        assert!(needs_review); // Should be flagged for review
    }
}
