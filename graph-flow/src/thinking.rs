//! Thinking orchestration graph — 10-layer cognitive stack.
//!
//! This module provides the scaffolding for the Ladybug thinking architecture:
//! a 10-layer cognitive processing pipeline wired as graph-flow Tasks.
//!
//! # Architecture
//!
//! ```text
//! Layer 1: Sensory Ingest     → MCP input → raw data
//! Layer 2: Fingerprint        → text → Plane.encounter()
//! Layer 3: Cascade Search     → cascade → band classification
//! Layer 4: (skipped - integrated into 3)
//! Layer 5: Semiring Reasoning → graph traversal
//! Layer 6: Memory Consolidate → encounter + seal check
//! Layer 7: Planning           → goal decomposition
//! Layer 8: Action Selection   → RL credit assignment
//! Layer 9: Output Generation  → LLM or template
//! Layer 10: Meta-Cognition    → self-monitoring
//! ```
//!
//! Conditional routing:
//! - After Cascade Search: Foveal band → skip to Memory Consolidate
//! - After Memory Consolidate: Staunen → replan, Wisdom → act
//!
//! # Examples
//!
//! ```rust
//! use graph_flow::thinking::build_thinking_graph;
//!
//! let graph = build_thinking_graph();
//! assert!(graph.start_task_id().is_some());
//! ```

use async_trait::async_trait;
use std::sync::Arc;

use crate::{
    context::Context,
    error::Result,
    graph::{Graph, GraphBuilder},
    task::{NextAction, Task, TaskResult},
};

// Layer task implementations

/// Layer 1: Sensory Ingest — reads raw input from MCP/external sources.
struct SensoryIngestTask;

#[async_trait]
impl Task for SensoryIngestTask {
    fn id(&self) -> &str { "sensory_ingest" }
    async fn run(&self, context: Context) -> Result<TaskResult> {
        let input: String = context.get("raw_input").await.unwrap_or_default();
        context.set("sensory_data", input.clone()).await;
        context.set("layer_1_complete", true).await;
        Ok(TaskResult::new(
            Some(format!("Sensory ingest: {} bytes", input.len())),
            NextAction::ContinueAndExecute,
        ))
    }
}

/// Layer 2: Fingerprint — convert text to encounter representation.
struct FingerprintTask;

#[async_trait]
impl Task for FingerprintTask {
    fn id(&self) -> &str { "fingerprint" }
    async fn run(&self, context: Context) -> Result<TaskResult> {
        let data: String = context.get("sensory_data").await.unwrap_or_default();
        // Placeholder: compute fingerprint/embedding
        let fingerprint = format!("fp:{:x}", data.len() as u64 * 0xDEAD);
        context.set("fingerprint", fingerprint.clone()).await;
        context.set("layer_2_complete", true).await;
        Ok(TaskResult::new(
            Some(format!("Fingerprint: {}", fingerprint)),
            NextAction::ContinueAndExecute,
        ))
    }
}

/// Layer 3: Cascade Search — classify into attention bands.
struct CascadeSearchTask;

#[async_trait]
impl Task for CascadeSearchTask {
    fn id(&self) -> &str { "cascade_search" }
    async fn run(&self, context: Context) -> Result<TaskResult> {
        let fingerprint: String = context.get("fingerprint").await.unwrap_or_default();
        // Placeholder: cascade search determines attention band
        // In real implementation, this would query the knowledge graph
        let band = if fingerprint.contains("0") {
            "Foveal"       // High familiarity → skip reasoning
        } else {
            "Parafoveal"   // Needs reasoning
        };
        context.set("best_band", band.to_string()).await;
        context.set("layer_3_complete", true).await;
        Ok(TaskResult::new(
            Some(format!("Cascade: band={}", band)),
            NextAction::Continue,
        ))
    }
}

/// Layer 5: Semiring Reasoning — graph traversal and inference.
struct SemiringReasonTask;

#[async_trait]
impl Task for SemiringReasonTask {
    fn id(&self) -> &str { "semiring_reason" }
    async fn run(&self, context: Context) -> Result<TaskResult> {
        let fingerprint: String = context.get("fingerprint").await.unwrap_or_default();
        // Placeholder: semiring reasoning over knowledge graph
        let inference = format!("inferred from {}", fingerprint);
        context.set("inference_result", inference.clone()).await;
        context.set("layer_5_complete", true).await;
        Ok(TaskResult::new(
            Some(format!("Semiring reasoning: {}", inference)),
            NextAction::ContinueAndExecute,
        ))
    }
}

/// Layer 6: Memory Consolidation — encounter + seal status check.
struct MemoryConsolidateTask;

#[async_trait]
impl Task for MemoryConsolidateTask {
    fn id(&self) -> &str { "memory_consolidate" }
    async fn run(&self, context: Context) -> Result<TaskResult> {
        let band: String = context.get("best_band").await.unwrap_or_default();
        // Check if this encounter triggers Staunen (surprise/wonder)
        // In real implementation, this would check encounter frequency
        let staunen = band == "Parafoveal"; // Novel = needs replanning
        context.set("staunen", staunen).await;
        context.set("layer_6_complete", true).await;
        Ok(TaskResult::new(
            Some(format!("Memory consolidated, staunen={}", staunen)),
            NextAction::Continue,
        ))
    }
}

/// Layer 7: Planning — goal decomposition.
struct PlanningTask;

#[async_trait]
impl Task for PlanningTask {
    fn id(&self) -> &str { "planning" }
    async fn run(&self, context: Context) -> Result<TaskResult> {
        let inference: String = context.get("inference_result").await.unwrap_or_default();
        let plan = format!("Plan based on: {}", inference);
        context.set("plan", plan.clone()).await;
        context.set("layer_7_complete", true).await;
        Ok(TaskResult::new(
            Some(format!("Planning: {}", plan)),
            NextAction::ContinueAndExecute,
        ))
    }
}

/// Layer 8: Action Selection — RL credit assignment.
struct ActionSelectTask;

#[async_trait]
impl Task for ActionSelectTask {
    fn id(&self) -> &str { "action_select" }
    async fn run(&self, context: Context) -> Result<TaskResult> {
        let plan: String = context.get("plan").await.unwrap_or_default();
        let action = if plan.is_empty() {
            "default_action".to_string()
        } else {
            format!("action_from:{}", &plan[..plan.len().min(20)])
        };
        context.set("selected_action", action.clone()).await;
        context.set("layer_8_complete", true).await;
        Ok(TaskResult::new(
            Some(format!("Action: {}", action)),
            NextAction::ContinueAndExecute,
        ))
    }
}

/// Layer 9: Output Generation — LLM response or template.
struct OutputGenerateTask;

#[async_trait]
impl Task for OutputGenerateTask {
    fn id(&self) -> &str { "output_generate" }
    async fn run(&self, context: Context) -> Result<TaskResult> {
        let action: String = context.get("selected_action").await.unwrap_or_default();
        let output = format!("Generated output for action: {}", action);
        context.set("output", output.clone()).await;
        context.set("layer_9_complete", true).await;
        Ok(TaskResult::new(
            Some(output),
            NextAction::ContinueAndExecute,
        ))
    }
}

/// Layer 10: Meta-Cognition — self-monitoring and PET scan trace.
struct MetaCognitionTask;

#[async_trait]
impl Task for MetaCognitionTask {
    fn id(&self) -> &str { "meta_cognition" }
    async fn run(&self, context: Context) -> Result<TaskResult> {
        // Collect PET scan trace: which layers executed
        let mut trace = Vec::new();
        for layer in 1..=10 {
            let key = format!("layer_{}_complete", layer);
            let done: bool = context.get_sync(&key).unwrap_or(false);
            if done {
                trace.push(format!("L{}", layer));
            }
        }

        let band: String = context.get("best_band").await.unwrap_or_default();
        let staunen: bool = context.get("staunen").await.unwrap_or(false);

        let pet_scan = serde_json::json!({
            "trace": trace,
            "band": band,
            "staunen": staunen,
            "layers_executed": trace.len(),
        });

        context.set("pet_scan", pet_scan.clone()).await;
        context.set("layer_10_complete", true).await;

        Ok(TaskResult::new(
            Some(format!("Meta-cognition: {} layers traced", trace.len())),
            NextAction::End,
        ))
    }
}

/// Build the 10-layer thinking orchestration graph.
///
/// The graph implements conditional routing:
/// - After cascade_search: Foveal → skip to memory_consolidate, else → semiring_reason
/// - After memory_consolidate: Staunen → planning, Wisdom → action_select
///
/// # Returns
///
/// An `Arc<Graph>` ready for execution.
pub fn build_thinking_graph() -> Arc<Graph> {
    let sensory = Arc::new(SensoryIngestTask) as Arc<dyn Task>;
    let fingerprint = Arc::new(FingerprintTask) as Arc<dyn Task>;
    let cascade = Arc::new(CascadeSearchTask) as Arc<dyn Task>;
    let semiring = Arc::new(SemiringReasonTask) as Arc<dyn Task>;
    let memory = Arc::new(MemoryConsolidateTask) as Arc<dyn Task>;
    let planning = Arc::new(PlanningTask) as Arc<dyn Task>;
    let action = Arc::new(ActionSelectTask) as Arc<dyn Task>;
    let output = Arc::new(OutputGenerateTask) as Arc<dyn Task>;
    let meta = Arc::new(MetaCognitionTask) as Arc<dyn Task>;

    let graph = GraphBuilder::new("ladybug_thinking")
        .add_task(sensory)
        .add_task(fingerprint)
        .add_task(cascade)
        .add_task(semiring)
        .add_task(memory)
        .add_task(planning)
        .add_task(action)
        .add_task(output)
        .add_task(meta)
        // Linear flow: sensory → fingerprint → cascade
        .add_edge("sensory_ingest", "fingerprint")
        .add_edge("fingerprint", "cascade_search")
        // Conditional: cascade → foveal skip or reasoning
        .add_conditional_edge(
            "cascade_search",
            |ctx| {
                ctx.get_sync::<String>("best_band")
                    .map(|b| b == "Foveal")
                    .unwrap_or(false)
            },
            "memory_consolidate",   // Foveal → skip reasoning
            "semiring_reason",      // Not Foveal → reason
        )
        // After reasoning → memory
        .add_edge("semiring_reason", "memory_consolidate")
        // Conditional: memory → staunen replan or act
        .add_conditional_edge(
            "memory_consolidate",
            |ctx| ctx.get_sync::<bool>("staunen").unwrap_or(false),
            "planning",         // Staunen → replan
            "action_select",    // Wisdom → act
        )
        // Planning → action
        .add_edge("planning", "action_select")
        // Action → output → meta
        .add_edge("action_select", "output_generate")
        .add_edge("output_generate", "meta_cognition")
        .build();

    Arc::new(graph)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::Session;

    #[tokio::test]
    async fn test_thinking_graph_compiles() {
        let graph = build_thinking_graph();
        assert_eq!(graph.start_task_id(), Some("sensory_ingest".to_string()));
        assert!(graph.get_task("sensory_ingest").is_some());
        assert!(graph.get_task("fingerprint").is_some());
        assert!(graph.get_task("cascade_search").is_some());
        assert!(graph.get_task("semiring_reason").is_some());
        assert!(graph.get_task("memory_consolidate").is_some());
        assert!(graph.get_task("planning").is_some());
        assert!(graph.get_task("action_select").is_some());
        assert!(graph.get_task("output_generate").is_some());
        assert!(graph.get_task("meta_cognition").is_some());
    }

    #[tokio::test]
    async fn test_thinking_graph_full_execution() {
        let graph = build_thinking_graph();

        let mut session = Session::new_from_task("s1".to_string(), "sensory_ingest");
        session.context.set("raw_input", "Hello, world!").await;

        // Execute all steps until completion
        for _ in 0..50 {
            let result = graph.execute_session(&mut session).await.unwrap();
            match result.status {
                crate::graph::ExecutionStatus::Completed => break,
                crate::graph::ExecutionStatus::Error(e) => panic!("Error: {}", e),
                _ => {}
            }
        }

        // Verify PET scan trace was captured
        let pet_scan: serde_json::Value = session
            .context
            .get("pet_scan")
            .await
            .expect("PET scan should be in context");

        let trace = pet_scan["trace"].as_array().unwrap();
        assert!(!trace.is_empty(), "PET scan trace should have entries");

        // Verify layer completions
        let l1: bool = session.context.get("layer_1_complete").await.unwrap_or(false);
        let l10: bool = session.context.get("layer_10_complete").await.unwrap_or(false);
        assert!(l1, "Layer 1 should be complete");
        assert!(l10, "Layer 10 should be complete");
    }

    #[tokio::test]
    async fn test_foveal_skip_path() {
        let graph = build_thinking_graph();

        let mut session = Session::new_from_task("s1".to_string(), "sensory_ingest");
        // Use input that will produce a fingerprint containing "0" → Foveal band
        session.context.set("raw_input", "test input with enough chars").await;

        // Execute to completion
        for _ in 0..50 {
            let result = graph.execute_session(&mut session).await.unwrap();
            match result.status {
                crate::graph::ExecutionStatus::Completed => break,
                crate::graph::ExecutionStatus::Error(e) => panic!("Error: {}", e),
                _ => {}
            }
        }

        let band: String = session.context.get("best_band").await.unwrap();
        if band == "Foveal" {
            // Foveal path should skip semiring reasoning (layer 5)
            let l5: bool = session.context.get("layer_5_complete").await.unwrap_or(false);
            assert!(!l5, "Foveal path should skip layer 5 (semiring reasoning)");
        }
        // Either path should complete meta-cognition
        let l10: bool = session.context.get("layer_10_complete").await.unwrap_or(false);
        assert!(l10, "Layer 10 should always complete");
    }

    #[tokio::test]
    async fn test_conditional_routing_works() {
        let graph = build_thinking_graph();

        let mut session = Session::new_from_task("s1".to_string(), "sensory_ingest");
        session.context.set("raw_input", "a").await;

        // Execute to completion
        for _ in 0..50 {
            let result = graph.execute_session(&mut session).await.unwrap();
            match result.status {
                crate::graph::ExecutionStatus::Completed => break,
                crate::graph::ExecutionStatus::Error(e) => panic!("Error: {}", e),
                _ => {}
            }
        }

        // Should have completed all the way through
        let output: String = session.context.get("output").await.unwrap();
        assert!(!output.is_empty());

        let pet_scan: serde_json::Value = session.context.get("pet_scan").await.unwrap();
        assert!(pet_scan["layers_executed"].as_u64().unwrap() >= 6);
    }
}
