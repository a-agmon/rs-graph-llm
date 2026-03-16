//! Example: YAML-Defined Agent
//!
//! Demonstrates compiling an agent from a YAML agent card definition
//! and executing it as a graph-flow workflow.
//!
//! Run with: `cargo run --bin yaml_agent`

use graph_flow::agents::agent_card::compile_agent_card;
use graph_flow::{FlowRunner, InMemorySessionStorage, Session, SessionStorage};
use std::sync::Arc;

const RESEARCHER_YAML: &str = r#"
agent:
  name: yaml_researcher
  description: A researcher agent defined entirely in YAML
  capabilities:
    - search
    - analyze
    - summarize
  tools:
    - name: web_search
      mcp_server: "https://search.mcp.server/sse"
      description: Search the web for information
    - name: document_fetch
      mcp_server: "https://docs.mcp.server/sse"
      description: Fetch documents by URL
  workflow:
    - task: search
      next: analyze
    - task: analyze
      next: summarize
    - task: summarize
      next: end
"#;

const CLASSIFIER_YAML: &str = r#"
agent:
  name: yaml_classifier
  description: A classifier agent with conditional routing
  capabilities:
    - preprocess
    - classify
    - handle_positive
    - handle_negative
  planes:
    read:
      - input_text
      - tokens
    write:
      - category
      - confidence
  workflow:
    - task: preprocess
      next: classify
    - task: classify
      condition_key: high_confidence
      on_success: handle_positive
      on_failure: handle_negative
    - task: handle_positive
      next: end
    - task: handle_negative
      next: end
"#;

#[tokio::main]
async fn main() -> graph_flow::Result<()> {
    println!("=== YAML Agent Example ===\n");

    // Compile researcher agent from YAML
    println!("--- Researcher Agent ---");
    let researcher_graph = compile_agent_card(RESEARCHER_YAML)?;
    println!(
        "Compiled '{}', start: {:?}",
        "yaml_researcher",
        researcher_graph.start_task_id()
    );

    let storage = Arc::new(InMemorySessionStorage::new());
    let runner = FlowRunner::new(researcher_graph, storage.clone());

    let session = Session::new_from_task("r1".to_string(), "search");
    storage.save(session).await?;

    // Execute
    for step in 1..=5 {
        let result = runner.run("r1").await?;
        println!(
            "  Step {}: {:?} → {:?}",
            step,
            result.response.as_deref().unwrap_or("(none)"),
            result.status
        );
        if matches!(
            result.status,
            graph_flow::ExecutionStatus::Completed
                | graph_flow::ExecutionStatus::WaitingForInput
        ) {
            break;
        }
    }

    // Compile classifier agent from YAML
    println!("\n--- Classifier Agent ---");
    let classifier_graph = compile_agent_card(CLASSIFIER_YAML)?;
    println!(
        "Compiled '{}', start: {:?}",
        "yaml_classifier",
        classifier_graph.start_task_id()
    );

    let storage2 = Arc::new(InMemorySessionStorage::new());
    let runner2 = FlowRunner::new(classifier_graph, storage2.clone());

    let session = Session::new_from_task("c1".to_string(), "preprocess");
    storage2.save(session).await?;

    for step in 1..=5 {
        let result = runner2.run("c1").await?;
        println!(
            "  Step {}: {:?} → {:?}",
            step,
            result.response.as_deref().unwrap_or("(none)"),
            result.status
        );
        if matches!(
            result.status,
            graph_flow::ExecutionStatus::Completed
                | graph_flow::ExecutionStatus::WaitingForInput
        ) {
            break;
        }
    }

    println!("\n=== YAML Agents Complete ===");
    Ok(())
}
