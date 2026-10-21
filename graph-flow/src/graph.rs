use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::timeout;

use crate::{
    context::Context,
    error::{GraphError, Result},
    storage::Session,
    task::{NextAction, Task, TaskResult},
};

/// Type alias for edge condition functions
pub type EdgeCondition = Arc<dyn Fn(&Context) -> bool + Send + Sync>;

/// Edge between tasks in the graph
#[derive(Clone)]
pub struct Edge {
    pub from: String,
    pub to: String,
    pub condition: Option<EdgeCondition>,
}

/// A graph of tasks that can be executed.
///
/// A `Graph` is immutable: it is assembled with [`GraphBuilder`] and validated
/// by [`GraphBuilder::build`]. Because nothing can change after construction,
/// lookups are plain `HashMap` reads with no locking.
pub struct Graph {
    pub id: String,
    tasks: HashMap<String, Arc<dyn Task>>,
    /// Outgoing edges indexed by source task id. Within a bucket, edges keep
    /// insertion order so the first matching conditional edge (or the first
    /// unconditional fallback) wins deterministically.
    edges: HashMap<String, Vec<Edge>>,
    start_task_id: Option<String>,
    task_timeout: Duration,
    /// Maximum number of chained `ContinueAndExecute` steps allowed within a
    /// single `execute_session` call. `None` (default) means unlimited.
    max_execution_steps: Option<usize>,
}

impl Graph {
    /// Create an empty graph with the given id.
    ///
    /// Useful as a placeholder (e.g. in tests). Real graphs are built with
    /// [`GraphBuilder`].
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            tasks: HashMap::new(),
            edges: HashMap::new(),
            start_task_id: None,
            task_timeout: Duration::from_secs(300), // Default 5 minute timeout
            max_execution_steps: None,
        }
    }

    /// Execute the graph with session management.
    ///
    /// Runs the session's current task. If the task returns
    /// [`NextAction::ContinueAndExecute`], execution proceeds to the next task
    /// within the same call, repeating until a task pauses, waits for input,
    /// ends, or the configured `max_execution_steps` limit is hit.
    ///
    /// Note: the session is only persisted by the *caller* (e.g.
    /// [`crate::FlowRunner::run`]) after this method returns, so context
    /// updates made by intermediate `ContinueAndExecute` steps are not durable
    /// until the whole chain finishes.
    pub async fn execute_session(&self, session: &mut Session) -> Result<ExecutionResult> {
        tracing::info!(
            graph_id = %self.id,
            session_id = %session.id,
            current_task = %session.current_task_id,
            "Starting graph execution"
        );

        let mut steps = 0usize;

        loop {
            let result = self
                .execute_single_task(&session.current_task_id, session.context.clone())
                .await?;

            // Update session status message if provided
            session.status_message = result.status_message.clone();

            match &result.next_action {
                NextAction::Continue | NextAction::ContinueAndExecute => {
                    let Some(next_task_id) = self.find_next_task(&result.task_id, &session.context)
                    else {
                        // No outgoing edge found, stay at current task
                        session.current_task_id = result.task_id.clone();
                        return Ok(ExecutionResult {
                            response: result.response,
                            status: ExecutionStatus::Paused {
                                next_task_id: result.task_id.clone(),
                                reason: "No outgoing edge found from current task".to_string(),
                            },
                        });
                    };

                    session.current_task_id = next_task_id.clone();

                    if result.next_action == NextAction::ContinueAndExecute {
                        steps += 1;
                        if let Some(max) = self.max_execution_steps
                            && steps > max
                        {
                            return Err(GraphError::TaskExecutionFailed(format!(
                                "Aborted after {} chained ContinueAndExecute steps \
                                 (max_execution_steps = {}). Possible cycle in graph '{}'",
                                steps, max, self.id
                            )));
                        }
                        // Execute the next task immediately within this call
                        continue;
                    }

                    return Ok(ExecutionResult {
                        response: result.response,
                        status: ExecutionStatus::Paused {
                            next_task_id,
                            reason: "Task completed, continuing to next task".to_string(),
                        },
                    });
                }
                NextAction::WaitForInput => {
                    // Stay at the current task
                    session.current_task_id = result.task_id.clone();
                    return Ok(ExecutionResult {
                        response: result.response,
                        status: ExecutionStatus::WaitingForInput,
                    });
                }
                NextAction::End => {
                    session.current_task_id = result.task_id.clone();
                    return Ok(ExecutionResult {
                        response: result.response,
                        status: ExecutionStatus::Completed,
                    });
                }
                NextAction::GoTo(target_id) => {
                    if !self.tasks.contains_key(target_id) {
                        return Err(GraphError::TaskNotFound(target_id.clone()));
                    }
                    session.current_task_id = target_id.clone();
                    return Ok(ExecutionResult {
                        response: result.response,
                        status: ExecutionStatus::Paused {
                            next_task_id: target_id.clone(),
                            reason: "Task requested jump to specific task".to_string(),
                        },
                    });
                }
            }
        }
    }

    /// Execute a single task without following Continue actions
    async fn execute_single_task(&self, task_id: &str, context: Context) -> Result<TaskResult> {
        tracing::debug!(
            task_id = %task_id,
            "Executing single task"
        );

        let task = self
            .tasks
            .get(task_id)
            .ok_or_else(|| GraphError::TaskNotFound(task_id.to_string()))?
            .clone();

        // Execute task with timeout
        let mut result = match timeout(self.task_timeout, task.run(context)).await {
            Ok(Ok(result)) => result,
            Ok(Err(e)) => {
                return Err(GraphError::TaskExecutionFailed(format!(
                    "Task '{}' failed: {}",
                    task_id, e
                )));
            }
            Err(_) => {
                return Err(GraphError::TaskExecutionFailed(format!(
                    "Task '{}' timed out after {:?}",
                    task_id, self.task_timeout
                )));
            }
        };

        // Set the task_id in the result to track which task generated it
        result.task_id = task_id.to_string();

        Ok(result)
    }

    /// Find the next task based on edges and conditions
    pub fn find_next_task(&self, current_task_id: &str, context: &Context) -> Option<String> {
        let edges = self.edges.get(current_task_id)?;

        let mut fallback: Option<&Edge> = None;
        for edge in edges {
            match &edge.condition {
                Some(pred) if pred(context) => return Some(edge.to.clone()),
                None if fallback.is_none() => fallback = Some(edge),
                _ => {}
            }
        }
        fallback.map(|edge| edge.to.clone())
    }

    /// Get the start task ID
    pub fn start_task_id(&self) -> Option<&str> {
        self.start_task_id.as_deref()
    }

    /// Get a task by ID
    pub fn get_task(&self, task_id: &str) -> Option<Arc<dyn Task>> {
        self.tasks.get(task_id).cloned()
    }
}

/// Builder for creating graphs.
///
/// The builder owns all graph data until [`GraphBuilder::build`] validates it
/// and produces an immutable [`Graph`].
pub struct GraphBuilder {
    id: String,
    tasks: HashMap<String, Arc<dyn Task>>,
    edges: HashMap<String, Vec<Edge>>,
    /// First task added; used as the start task unless overridden.
    first_task_id: Option<String>,
    /// Explicit start task set via `set_start_task`.
    start_task_id: Option<String>,
    task_timeout: Duration,
    max_execution_steps: Option<usize>,
}

impl GraphBuilder {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            tasks: HashMap::new(),
            edges: HashMap::new(),
            first_task_id: None,
            start_task_id: None,
            task_timeout: Duration::from_secs(300),
            max_execution_steps: None,
        }
    }

    pub fn add_task(mut self, task: Arc<dyn Task>) -> Self {
        let task_id = task.id().to_string();
        if self.first_task_id.is_none() {
            self.first_task_id = Some(task_id.clone());
        }
        self.tasks.insert(task_id, task);
        self
    }

    pub fn add_edge(mut self, from: impl Into<String>, to: impl Into<String>) -> Self {
        let from = from.into();
        self.edges.entry(from.clone()).or_default().push(Edge {
            from,
            to: to.into(),
            condition: None,
        });
        self
    }

    /// Add a conditional edge with an explicit `else` branch.
    /// `yes` is taken when `condition(ctx)` returns `true`; otherwise `no` is chosen.
    pub fn add_conditional_edge<F>(
        mut self,
        from: impl Into<String>,
        condition: F,
        yes: impl Into<String>,
        no: impl Into<String>,
    ) -> Self
    where
        F: Fn(&Context) -> bool + Send + Sync + 'static,
    {
        let from = from.into();
        let bucket = self.edges.entry(from.clone()).or_default();

        // "yes" branch
        bucket.push(Edge {
            from: from.clone(),
            to: yes.into(),
            condition: Some(Arc::new(condition)),
        });

        // "else" branch (unconditional fallback)
        bucket.push(Edge {
            from,
            to: no.into(),
            condition: None,
        });

        self
    }

    /// Set the starting task. Validated in [`GraphBuilder::build`]; defaults
    /// to the first task added.
    pub fn set_start_task(mut self, task_id: impl Into<String>) -> Self {
        self.start_task_id = Some(task_id.into());
        self
    }

    /// Set the timeout applied to each task execution (default: 5 minutes).
    pub fn with_task_timeout(mut self, timeout: Duration) -> Self {
        self.task_timeout = timeout;
        self
    }

    /// Limit the number of chained `ContinueAndExecute` steps within a single
    /// `execute_session` call, guarding against infinite loops in cyclic
    /// graphs (default: unlimited).
    pub fn with_max_execution_steps(mut self, max: usize) -> Self {
        self.max_execution_steps = Some(max);
        self
    }

    /// Validate the graph and produce an immutable [`Graph`].
    ///
    /// # Errors
    ///
    /// - [`GraphError::InvalidEdge`] if an edge references a task that was
    ///   never added
    /// - [`GraphError::TaskNotFound`] if `set_start_task` names an unknown task
    pub fn build(self) -> Result<Graph> {
        if self.tasks.is_empty() {
            tracing::warn!("Building graph with no tasks");
        }

        // Every edge endpoint must be a known task
        let mut connected_tasks = HashSet::new();
        for edge in self.edges.values().flatten() {
            for endpoint in [&edge.from, &edge.to] {
                if !self.tasks.contains_key(endpoint) {
                    return Err(GraphError::InvalidEdge(format!(
                        "Edge '{}' -> '{}' references task '{}' which was not added to graph '{}'",
                        edge.from, edge.to, endpoint, self.id
                    )));
                }
                connected_tasks.insert(endpoint.clone());
            }
        }

        // An explicit start task must exist
        let start_task_id = match self.start_task_id {
            Some(id) => {
                if !self.tasks.contains_key(&id) {
                    return Err(GraphError::TaskNotFound(format!(
                        "Start task '{}' was not added to graph '{}'",
                        id, self.id
                    )));
                }
                Some(id)
            }
            None => self.first_task_id,
        };

        // Warn about orphaned tasks (no incoming or outgoing edges)
        if self.tasks.len() > 1 {
            for task_id in self.tasks.keys() {
                if !connected_tasks.contains(task_id) {
                    tracing::warn!(
                        task_id = %task_id,
                        "Task has no edges - it may be unreachable"
                    );
                }
            }
        }

        Ok(Graph {
            id: self.id,
            tasks: self.tasks,
            edges: self.edges,
            start_task_id,
            task_timeout: self.task_timeout,
            max_execution_steps: self.max_execution_steps,
        })
    }
}

/// Status of graph execution
#[derive(Debug, Clone)]
pub struct ExecutionResult {
    pub response: Option<String>,
    pub status: ExecutionStatus,
}

#[derive(Debug, Clone)]
pub enum ExecutionStatus {
    /// Paused, will continue automatically to the specified next task
    Paused {
        next_task_id: String,
        reason: String,
    },
    /// Waiting for user input to continue
    WaitingForInput,
    /// Workflow completed successfully
    Completed,
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;

    struct ActionTask {
        id: &'static str,
        action: NextAction,
    }

    #[async_trait]
    impl Task for ActionTask {
        fn id(&self) -> &str {
            self.id
        }

        async fn run(&self, _context: Context) -> Result<TaskResult> {
            Ok(TaskResult::new(None, self.action.clone()))
        }
    }

    #[tokio::test]
    async fn max_execution_steps_allows_the_configured_number_of_hops() {
        let start = Arc::new(ActionTask {
            id: "start",
            action: NextAction::ContinueAndExecute,
        });
        let end = Arc::new(ActionTask {
            id: "end",
            action: NextAction::End,
        });
        let graph = GraphBuilder::new("bounded")
            .add_task(start.clone())
            .add_task(end.clone())
            .add_edge(start.id(), end.id())
            .with_max_execution_steps(1)
            .build()
            .unwrap();
        let mut session = Session::new_from_task("session".to_string(), start.id());

        let result = graph.execute_session(&mut session).await.unwrap();

        assert!(matches!(result.status, ExecutionStatus::Completed));
        assert_eq!(session.current_task_id, "end");
    }

    #[tokio::test]
    async fn max_execution_steps_stops_chains_over_the_limit() {
        let start = Arc::new(ActionTask {
            id: "start",
            action: NextAction::ContinueAndExecute,
        });
        let middle = Arc::new(ActionTask {
            id: "middle",
            action: NextAction::ContinueAndExecute,
        });
        let end = Arc::new(ActionTask {
            id: "end",
            action: NextAction::End,
        });
        let graph = GraphBuilder::new("bounded")
            .add_task(start.clone())
            .add_task(middle.clone())
            .add_task(end.clone())
            .add_edge(start.id(), middle.id())
            .add_edge(middle.id(), end.id())
            .with_max_execution_steps(1)
            .build()
            .unwrap();
        let mut session = Session::new_from_task("session".to_string(), start.id());

        let error = graph.execute_session(&mut session).await.unwrap_err();

        assert!(matches!(
            error,
            GraphError::TaskExecutionFailed(message)
                if message.contains("Aborted after 2 chained ContinueAndExecute steps")
        ));
    }
}
