//! Example: Researcher Agent
//!
//! A multi-step agent that searches for information, analyzes it,
//! and produces a summary. Demonstrates agent card YAML compilation
//! and graph-flow execution.
//!
//! Run with: `cargo run --bin researcher_agent`

use async_trait::async_trait;
use graph_flow::{
    Context, FlowRunner, GraphBuilder, InMemorySessionStorage, NextAction, Session,
    SessionStorage, Task, TaskResult,
};
use std::sync::Arc;

/// Search task — simulates web search and retrieves documents.
struct SearchTask;

#[async_trait]
impl Task for SearchTask {
    fn id(&self) -> &str {
        "search"
    }

    async fn run(&self, context: Context) -> graph_flow::Result<TaskResult> {
        let query: String = context
            .get("query")
            .await
            .unwrap_or_else(|| "default query".to_string());

        println!("[Search] Searching for: {}", query);

        // Simulate search results
        let results = vec![
            format!("Result 1: Introduction to {}", query),
            format!("Result 2: Advanced concepts in {}", query),
            format!("Result 3: Best practices for {}", query),
        ];

        context.set("search_results", results.clone()).await;
        context.set("search_count", results.len()).await;

        Ok(TaskResult::new(
            Some(format!("Found {} results for '{}'", results.len(), query)),
            NextAction::ContinueAndExecute,
        ))
    }
}

/// Analyze task — processes search results and extracts key findings.
struct AnalyzeTask;

#[async_trait]
impl Task for AnalyzeTask {
    fn id(&self) -> &str {
        "analyze"
    }

    async fn run(&self, context: Context) -> graph_flow::Result<TaskResult> {
        let results: Vec<String> = context.get("search_results").await.unwrap_or_default();

        println!("[Analyze] Analyzing {} results...", results.len());

        // Simulate analysis
        let findings: Vec<String> = results
            .iter()
            .enumerate()
            .map(|(i, r)| format!("Finding {}: Key insight from '{}'", i + 1, r))
            .collect();

        let quality_score = if results.len() >= 3 { 0.95 } else { 0.6 };

        context.set("findings", findings.clone()).await;
        context.set("quality_score", quality_score).await;
        context.set("analysis_complete", true).await;

        Ok(TaskResult::new(
            Some(format!(
                "Analyzed {} results, quality score: {:.2}",
                results.len(),
                quality_score
            )),
            NextAction::Continue, // Pause before summarizing
        ))
    }
}

/// Summarize task — produces a final summary from analyzed findings.
struct SummarizeTask;

#[async_trait]
impl Task for SummarizeTask {
    fn id(&self) -> &str {
        "summarize"
    }

    async fn run(&self, context: Context) -> graph_flow::Result<TaskResult> {
        let findings: Vec<String> = context.get("findings").await.unwrap_or_default();
        let query: String = context.get("query").await.unwrap_or_default();
        let quality: f64 = context.get("quality_score").await.unwrap_or(0.0);

        println!("[Summarize] Creating summary from {} findings...", findings.len());

        let summary = format!(
            "Research Summary for '{}'\n\
             Quality Score: {:.0}%\n\
             Key Findings:\n{}",
            query,
            quality * 100.0,
            findings
                .iter()
                .map(|f| format!("  - {}", f))
                .collect::<Vec<_>>()
                .join("\n")
        );

        context.set("summary", summary.clone()).await;

        Ok(TaskResult::new(Some(summary), NextAction::End))
    }
}

#[tokio::main]
async fn main() -> graph_flow::Result<()> {
    println!("=== Researcher Agent Example ===\n");

    // Build the researcher agent graph
    let search = Arc::new(SearchTask) as Arc<dyn Task>;
    let analyze = Arc::new(AnalyzeTask) as Arc<dyn Task>;
    let summarize = Arc::new(SummarizeTask) as Arc<dyn Task>;

    let graph = Arc::new(
        GraphBuilder::new("researcher")
            .add_task(search.clone())
            .add_task(analyze.clone())
            .add_task(summarize.clone())
            .add_edge("search", "analyze")
            .add_conditional_edge(
                "analyze",
                |ctx| {
                    ctx.get_sync::<f64>("quality_score")
                        .map(|q| q >= 0.8)
                        .unwrap_or(false)
                },
                "summarize",  // High quality → summarize
                "search",     // Low quality → search again
            )
            .build(),
    );

    // Set up session
    let storage = Arc::new(InMemorySessionStorage::new());
    let runner = FlowRunner::new(graph, storage.clone());

    let session = Session::new_from_task("research_session".to_string(), "search");
    session
        .context
        .set("query", "Rust async programming patterns".to_string())
        .await;
    storage.save(session).await?;

    // Execute the workflow
    println!("Starting researcher agent...\n");
    loop {
        let result = runner.run("research_session").await?;

        if let Some(response) = &result.response {
            println!(">>> {}\n", response);
        }

        match result.status {
            graph_flow::ExecutionStatus::Completed => {
                println!("\n=== Research Complete ===");
                break;
            }
            graph_flow::ExecutionStatus::Paused {
                next_task_id,
                reason,
            } => {
                println!("  [Paused] Next: {}, Reason: {}", next_task_id, reason);
            }
            graph_flow::ExecutionStatus::WaitingForInput => {
                println!("  [Waiting for input]");
                break;
            }
            graph_flow::ExecutionStatus::Error(e) => {
                println!("  [Error] {}", e);
                break;
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_researcher_agent_runs() {
        let search = Arc::new(SearchTask) as Arc<dyn Task>;
        let analyze = Arc::new(AnalyzeTask) as Arc<dyn Task>;
        let summarize = Arc::new(SummarizeTask) as Arc<dyn Task>;

        let graph = Arc::new(
            GraphBuilder::new("researcher")
                .add_task(search)
                .add_task(analyze)
                .add_task(summarize)
                .add_edge("search", "analyze")
                .add_edge("analyze", "summarize")
                .build(),
        );

        let storage = Arc::new(InMemorySessionStorage::new());
        let runner = FlowRunner::new(graph, storage.clone());

        let session = Session::new_from_task("test".to_string(), "search");
        session.context.set("query", "test query").await;
        storage.save(session).await.unwrap();

        // Run through the workflow
        for _ in 0..10 {
            let result = runner.run("test").await.unwrap();
            if matches!(result.status, graph_flow::ExecutionStatus::Completed) {
                // Verify summary was produced
                let session = storage.get("test").await.unwrap().unwrap();
                let summary: Option<String> = session.context.get("summary").await;
                assert!(summary.is_some());
                return;
            }
        }
        panic!("Workflow did not complete");
    }
}
