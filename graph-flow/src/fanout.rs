//! FanOutTask – a composite task that runs multiple child tasks in parallel
//!
//! This task provides simple parallelism within a single graph node. It executes
//! a fixed set of child tasks concurrently, waits for them to finish, aggregates
//! their responses into the shared `Context`, and then returns control back to
//! the graph with `NextAction::Continue` (by default).
//!
//! Design goals:
//! - Keep engine changes minimal (no changes to `Graph` needed)
//! - Keep semantics simple and predictable
//! - Make context aggregation explicit and easy to consume by downstream tasks
//!
//! Important caveats:
//! - Child tasks' `NextAction` is ignored by `FanOutTask`. Children are treated as
//!   units-of-work that produce outputs and/or write to context, not as control-flow
//!   steps. The `FanOutTask` itself controls the next step of the graph.
//! - By default, all children share the same `Context` (concurrent writes must be
//!   coordinated by the user). To avoid key collisions, you can set a prefix so that
//!   each child’s output is stored under `"<prefix>.<child_id>.*"`.
//! - Error policy is conservative: if any child fails, `FanOutTask` fails.
//!
//! Example:
//! ```rust
//! use graph_flow::{Context, Task, TaskResult, NextAction};
//! use graph_flow::fanout::FanOutTask;
//! use async_trait::async_trait;
//! use std::sync::Arc;
//!
//! struct ChildA;
//! struct ChildB;
//!
//! #[async_trait]
//! impl Task for ChildA {
//!     fn id(&self) -> &str { "child_a" }
//!     async fn run(&self, ctx: Context) -> graph_flow::Result<TaskResult> {
//!         ctx.set("a", 1_i32).await;
//!         Ok(TaskResult::new(Some("A done".to_string()), NextAction::End))
//!     }
//! }
//!
//! #[async_trait]
//! impl Task for ChildB {
//!     fn id(&self) -> &str { "child_b" }
//!     async fn run(&self, ctx: Context) -> graph_flow::Result<TaskResult> {
//!         ctx.set("b", 2_i32).await;
//!         Ok(TaskResult::new(Some("B done".to_string()), NextAction::End))
//!     }
//! }
//!
//! # #[tokio::main]
//! # async fn main() -> graph_flow::Result<()> {
//! let fan = FanOutTask::new("fan", vec![Arc::new(ChildA), Arc::new(ChildB)])
//!     .with_prefix("fanout");
//! let ctx = Context::new();
//! let _ = fan.run(ctx.clone()).await?;
//! // Aggregated entries under prefix:
//! // fanout.child_a.response, fanout.child_b.response
//! # Ok(())
//! # }
//! ```

use async_trait::async_trait;
use std::sync::Arc;
use tokio::task::JoinSet;

use crate::{Context, Result, Task, TaskResult, NextAction, GraphError};

/// Composite task that executes multiple child tasks concurrently and aggregates results.
#[derive(Clone)]
pub struct FanOutTask {
    id: String,
    children: Vec<Arc<dyn Task>>, // executed in parallel
    prefix: Option<String>,        // context aggregation prefix
    next_action: NextAction,       // default: Continue
}

impl FanOutTask {
    /// Create a new `FanOutTask` with an explicit id and a list of child tasks.
    pub fn new(id: impl Into<String>, children: Vec<Arc<dyn Task>>) -> Arc<Self> {
        Arc::new(Self {
            id: id.into(),
            children,
            prefix: None,
            next_action: NextAction::Continue,
        })
    }

    /// Set a context prefix for storing aggregated child results.
    ///
    /// Aggregation keys will be written as `<prefix>.<child_id>.<field>`.
    pub fn with_prefix(mut self: Arc<Self>, prefix: impl Into<String>) -> Arc<Self> {
        Arc::make_mut(&mut self).prefix = Some(prefix.into());
        self
    }

    /// Override the `NextAction` returned by the `FanOutTask` (default: `Continue`).
    pub fn with_next_action(mut self: Arc<Self>, next: NextAction) -> Arc<Self> {
        Arc::make_mut(&mut self).next_action = next;
        self
    }

    fn key(&self, child_id: &str, field: &str) -> String {
        if let Some(p) = &self.prefix {
            format!("{}.{}.{}", p, child_id, field)
        } else {
            format!("fanout.{}.{}", child_id, field)
        }
    }
}

#[async_trait]
impl Task for FanOutTask {
    fn id(&self) -> &str { &self.id }

    async fn run(&self, context: Context) -> Result<TaskResult> {
        let mut set = JoinSet::new();

        // Spawn children concurrently
        for child in &self.children {
            let child = child.clone();
            let ctx = context.clone();
            set.spawn(async move {
                let cid = child.id().to_string();
                let res = child.run(ctx.clone()).await;
                (cid, res)
            });
        }

        let mut had_error = None;
        let mut completed = 0usize;

        while let Some(joined) = set.join_next().await {
            match joined {
                Err(join_err) => {
                    had_error = Some(GraphError::TaskExecutionFailed(format!(
                        "FanOut child join error: {}", join_err
                    )));
                }
                Ok((child_id, outcome)) => match outcome {
                    Err(e) => {
                        had_error = Some(GraphError::TaskExecutionFailed(format!(
                            "FanOut child '{}' failed: {}", child_id, e
                        )));
                    }
                    Ok(tr) => {
                        // Store child outputs under prefixed keys
                        if let Some(resp) = tr.response.clone() {
                            context.set(self.key(&child_id, "response"), resp).await;
                        }
                        if let Some(status) = tr.status_message.clone() {
                            context.set(self.key(&child_id, "status"), status).await;
                        }
                        // Always store the reported next_action for diagnostics
                        context
                            .set(self.key(&child_id, "next_action"), format!("{:?}", tr.next_action))
                            .await;
                        completed += 1;
                    }
                },
            }
        }

        if let Some(err) = had_error {
            return Err(err);
        }

        let summary = format!(
            "FanOutTask '{}' completed {} child task(s)",
            self.id, completed
        );

        Ok(TaskResult::new_with_status(
            Some(summary.clone()),
            self.next_action.clone(),
            Some(summary),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use tokio::time::{sleep, Duration};

    struct OkTask { name: &'static str }
    struct FailingTask { name: &'static str }

    #[async_trait]
    impl Task for OkTask {
        fn id(&self) -> &str { self.name }
        async fn run(&self, ctx: Context) -> Result<TaskResult> {
            ctx.set(format!("out.{}", self.name), true).await;
            sleep(Duration::from_millis(10)).await;
            Ok(TaskResult::new(Some(format!("{} ok", self.name)), NextAction::End))
        }
    }

    #[async_trait]
    impl Task for FailingTask {
        fn id(&self) -> &str { self.name }
        async fn run(&self, _ctx: Context) -> Result<TaskResult> {
            Err(GraphError::TaskExecutionFailed(format!("{} failed", self.name)))
        }
    }

    #[tokio::test]
    async fn fanout_all_success_aggregates() {
        let a: Arc<dyn Task> = Arc::new(OkTask { name: "a" });
        let b: Arc<dyn Task> = Arc::new(OkTask { name: "b" });
        let fan = FanOutTask::new("fan", vec![a, b]).with_prefix("agg");

        let ctx = Context::new();
        let res = fan.run(ctx.clone()).await.unwrap();

        assert_eq!(res.next_action, NextAction::Continue);

        let ar: Option<String> = ctx.get("agg.a.response").await;
        let br: Option<String> = ctx.get("agg.b.response").await;
        assert_eq!(ar, Some("a ok".to_string()));
        assert_eq!(br, Some("b ok".to_string()));

        // also store next_action diagnostic
        let an: Option<String> = ctx.get("agg.a.next_action").await;
        assert_eq!(an, Some(format!("{:?}", NextAction::End)));
    }

    #[tokio::test]
    async fn fanout_failure_bubbles_up() {
        let a: Arc<dyn Task> = Arc::new(OkTask { name: "a" });
        let f: Arc<dyn Task> = Arc::new(FailingTask { name: "bad" });
        let fan = FanOutTask::new("fan", vec![a, f]);

        let ctx = Context::new();
        let err = fan.run(ctx.clone()).await.err().unwrap();
        match err {
            GraphError::TaskExecutionFailed(msg) => assert!(msg.contains("bad")),
            other => panic!("Unexpected error variant: {other:?}"),
        }
    }
}
