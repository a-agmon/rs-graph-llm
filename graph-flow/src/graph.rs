use dashmap::DashMap;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
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

/// Type alias for N-way path functions that return a routing key string.
pub type PathFunction = Arc<dyn Fn(&Context) -> String + Send + Sync>;

/// An N-way conditional edge: calls `path_fn` and looks up the result in `path_map`.
#[derive(Clone)]
pub struct ConditionalEdgeSet {
    pub from: String,
    pub path_fn: PathFunction,
    /// Maps path function return values to target task IDs.
    pub path_map: HashMap<String, String>,
}

/// Edge between tasks in the graph
#[derive(Clone)]
pub struct Edge {
    pub from: String,
    pub to: String,
    pub condition: Option<EdgeCondition>,
}

/// A graph of tasks that can be executed
pub struct Graph {
    pub id: String,
    tasks: DashMap<String, Arc<dyn Task>>,
    edges: Mutex<Vec<Edge>>,
    conditional_edge_sets: Mutex<Vec<ConditionalEdgeSet>>,
    start_task_id: Mutex<Option<String>>,
    task_timeout: Duration,
}

impl Graph {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            tasks: DashMap::new(),
            edges: Mutex::new(Vec::new()),
            conditional_edge_sets: Mutex::new(Vec::new()),
            start_task_id: Mutex::new(None),
            task_timeout: Duration::from_secs(300), // Default 5 minute timeout
        }
    }
    
    /// Set the timeout duration for task execution
    pub fn set_task_timeout(&mut self, timeout: Duration) {
        self.task_timeout = timeout;
    }

    /// Add a task to the graph
    pub fn add_task(&self, task: Arc<dyn Task>) -> &Self {
        let task_id = task.id().to_string();
        let is_first = self.tasks.is_empty();
        self.tasks.insert(task_id.clone(), task);

        // Set as start task if it's the first one
        if is_first {
            *self.start_task_id.lock().unwrap() = Some(task_id);
        }

        self
    }

    /// Set the starting task
    pub fn set_start_task(&self, task_id: impl Into<String>) -> &Self {
        let task_id = task_id.into();
        if self.tasks.contains_key(&task_id) {
            *self.start_task_id.lock().unwrap() = Some(task_id);
        }
        self
    }

    /// Add an edge between tasks
    pub fn add_edge(&self, from: impl Into<String>, to: impl Into<String>) -> &Self {
        self.edges.lock().unwrap().push(Edge {
            from: from.into(),
            to: to.into(),
            condition: None,
        });
        self
    }

    /// Add a conditional edge with an explicit `else` branch.
    /// `yes` is taken when `condition(ctx)` returns `true`; otherwise `no` is chosen.
    pub fn add_conditional_edge<F>(
        &self,
        from: impl Into<String>,
        condition: F,
        yes: impl Into<String>,
        no: impl Into<String>,
    ) -> &Self
    where
        F: Fn(&Context) -> bool + Send + Sync + 'static,
    {
        let from = from.into();
        let yes_to = yes.into();
        let no_to = no.into();

        let predicate: EdgeCondition = Arc::new(condition);

        let mut edges = self.edges.lock().unwrap();

        // "yes" branch
        edges.push(Edge {
            from: from.clone(),
            to: yes_to,
            condition: Some(predicate),
        });

        // "else" branch (unconditional fallback)
        edges.push(Edge {
            from,
            to: no_to,
            condition: None,
        });

        self
    }

    /// Add N-way conditional edges based on a path function and a path map.
    ///
    /// The `path_fn` is called with the context and returns a routing key string.
    /// The routing key is looked up in `path_map` to determine the target task.
    ///
    /// This is the equivalent of LangGraph's `add_conditional_edges(source, path_fn, path_map)`.
    pub fn add_conditional_edges<F>(
        &self,
        from: impl Into<String>,
        path_fn: F,
        path_map: HashMap<String, String>,
    ) -> &Self
    where
        F: Fn(&Context) -> String + Send + Sync + 'static,
    {
        self.conditional_edge_sets.lock().unwrap().push(ConditionalEdgeSet {
            from: from.into(),
            path_fn: Arc::new(path_fn),
            path_map,
        });
        self
    }

    /// Execute the graph with session management
    /// This method manages the session state and returns a simple status
    pub async fn execute_session(&self, session: &mut Session) -> Result<ExecutionResult> {
        tracing::info!(
            graph_id = %self.id,
            session_id = %session.id,
            current_task = %session.current_task_id,
            "Starting graph execution"
        );
        
        // Execute ONLY the current task (not the full recursive chain)
        let result = self
            .execute_single_task(&session.current_task_id, session.context.clone())
            .await?;

        // Handle next action at the session level
        match &result.next_action {
            NextAction::Continue => {
                // Update session status message if provided
                session.status_message = result.status_message.clone();

                // Find the next task but don't execute it
                if let Some(next_task_id) = self.find_next_task(&result.task_id, &session.context) {
                    session.advance_to(next_task_id.clone());
                    Ok(ExecutionResult {
                        response: result.response,
                        status: ExecutionStatus::Paused {
                            next_task_id,
                            reason: "Task completed, continuing to next task".to_string(),
                        },
                    })
                } else {
                    // No next task found, stay at current task
                    session.current_task_id = result.task_id.clone();
                    Ok(ExecutionResult {
                        response: result.response,
                        status: ExecutionStatus::Paused {
                            next_task_id: result.task_id.clone(),
                            reason: "No outgoing edge found from current task".to_string(),
                        },
                    })
                }
            }
            NextAction::ContinueAndExecute => {
                // Update session status message if provided
                session.status_message = result.status_message.clone();

                // Find the next task and execute it immediately (recursive behavior)
                if let Some(next_task_id) = self.find_next_task(&result.task_id, &session.context) {
                    session.advance_to(next_task_id);

                    // Recursively call execute_session to maintain proper context sharing
                    return Box::pin(self.execute_session(session)).await;
                } else {
                    // No next task found, stay at current task
                    session.current_task_id = result.task_id.clone();
                    Ok(ExecutionResult {
                        response: result.response,
                        status: ExecutionStatus::Paused {
                            next_task_id: result.task_id.clone(),
                            reason: "No outgoing edge found from current task".to_string(),
                        },
                    })
                }
            }
            NextAction::WaitForInput => {
                // Update session status message if provided
                session.status_message = result.status_message.clone();
                // Stay at the current task
                session.current_task_id = result.task_id.clone();
                Ok(ExecutionResult {
                    response: result.response,
                    status: ExecutionStatus::WaitingForInput,
                })
            }
            NextAction::End => {
                // Update session status message if provided
                session.status_message = result.status_message.clone();
                session.current_task_id = result.task_id.clone();
                Ok(ExecutionResult {
                    response: result.response,
                    status: ExecutionStatus::Completed,
                })
            }
            NextAction::GoTo(target_id) => {
                // Update session status message if provided
                session.status_message = result.status_message.clone();
                if self.tasks.contains_key(target_id) {
                    session.advance_to(target_id.clone());
                    Ok(ExecutionResult {
                        response: result.response,
                        status: ExecutionStatus::Paused {
                            next_task_id: target_id.clone(),
                            reason: "Task requested jump to specific task".to_string(),
                        },
                    })
                } else {
                    Err(GraphError::TaskNotFound(target_id.clone()))
                }
            }
            NextAction::GoBack => {
                session.status_message = result.status_message.clone();
                if let Some(prev_task_id) = session.go_back() {
                    Ok(ExecutionResult {
                        response: result.response,
                        status: ExecutionStatus::Paused {
                            next_task_id: prev_task_id,
                            reason: "Navigated back to previous task".to_string(),
                        },
                    })
                } else {
                    // No history to go back to — stay at current task
                    session.current_task_id = result.task_id.clone();
                    Ok(ExecutionResult {
                        response: result.response,
                        status: ExecutionStatus::WaitingForInput,
                    })
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
            .ok_or_else(|| GraphError::TaskNotFound(task_id.to_string()))?;

        // Execute task with timeout
        let task_future = task.run(context);
        let mut result = match timeout(self.task_timeout, task_future).await {
            Ok(Ok(result)) => result,
            Ok(Err(e)) => return Err(GraphError::TaskExecutionFailed(
                format!("Task '{}' failed: {}", task_id, e)
            )),
            Err(_) => return Err(GraphError::TaskExecutionFailed(
                format!("Task '{}' timed out after {:?}", task_id, self.task_timeout)
            )),
        };

        // Set the task_id in the result to track which task generated it
        result.task_id = task_id.to_string();

        Ok(result)
    }

    /// Execute the graph starting from a specific task
    pub async fn execute(&self, task_id: &str, context: Context) -> Result<TaskResult> {
        let task = self
            .tasks
            .get(task_id)
            .ok_or_else(|| GraphError::TaskNotFound(task_id.to_string()))?;

        let mut result = task.run(context.clone()).await?;

        // Set the task_id in the result to track which task generated it
        result.task_id = task_id.to_string();

        // Handle next action
        match &result.next_action {
            NextAction::Continue => {
                // If this task has a response, stop here and don't continue to next task
                // This allows the response to be returned to the user
                if result.response.is_some() {
                    Ok(result)
                } else {
                    // Find the next task based on edges
                    if let Some(next_task_id) = self.find_next_task(task_id, &context) {
                        Box::pin(self.execute(&next_task_id, context)).await
                    } else {
                        Ok(result)
                    }
                }
            }
            NextAction::GoTo(target_id) => {
                if self.tasks.contains_key(target_id) {
                    Box::pin(self.execute(target_id, context)).await
                } else {
                    Err(GraphError::TaskNotFound(target_id.clone()))
                }
            }
            _ => Ok(result),
        }
    }

    /// Find the next task based on edges and conditions.
    ///
    /// Priority order:
    /// 1. N-way conditional edge sets (path function + path map)
    /// 2. Binary conditional edges (predicate-based)
    /// 3. Unconditional edges (fallback)
    pub fn find_next_task(&self, current_task_id: &str, context: &Context) -> Option<String> {
        // 1. Check N-way conditional edge sets first
        {
            let sets = self.conditional_edge_sets.lock().unwrap();
            for set in sets.iter().filter(|s| s.from == current_task_id) {
                let key = (set.path_fn)(context);
                if let Some(target) = set.path_map.get(&key) {
                    return Some(target.clone());
                }
            }
        }

        // 2. Check binary conditional edges and unconditional edges
        let edges = self.edges.lock().unwrap();
        let mut fallback: Option<String> = None;
        for edge in edges.iter().filter(|e| e.from == current_task_id) {
            match &edge.condition {
                Some(pred) if pred(context) => return Some(edge.to.clone()),
                None if fallback.is_none() => fallback = Some(edge.to.clone()),
                _ => {}
            }
        }
        fallback
    }

    /// Get the start task ID
    pub fn start_task_id(&self) -> Option<String> {
        self.start_task_id.lock().unwrap().clone()
    }

    /// Get a task by ID
    pub fn get_task(&self, task_id: &str) -> Option<Arc<dyn Task>> {
        self.tasks.get(task_id).map(|entry| entry.clone())
    }
}

/// Builder for creating graphs
pub struct GraphBuilder {
    graph: Graph,
}

impl GraphBuilder {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            graph: Graph::new(id),
        }
    }

    pub fn add_task(self, task: Arc<dyn Task>) -> Self {
        self.graph.add_task(task);
        self
    }

    pub fn add_edge(self, from: impl Into<String>, to: impl Into<String>) -> Self {
        self.graph.add_edge(from, to);
        self
    }

    pub fn add_conditional_edge<F>(
        self,
        from: impl Into<String>,
        condition: F,
        yes: impl Into<String>,
        no: impl Into<String>,
    ) -> Self
    where
        F: Fn(&Context) -> bool + Send + Sync + 'static,
    {
        self.graph.add_conditional_edge(from, condition, yes, no);
        self
    }

    /// Add N-way conditional edges using a path function and path map.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use graph_flow::{GraphBuilder, Context, Task, TaskResult, NextAction};
    /// # use async_trait::async_trait;
    /// # use std::sync::Arc;
    /// # use std::collections::HashMap;
    /// # struct T; #[async_trait] impl Task for T { fn id(&self) -> &str { "t" } async fn run(&self, _: Context) -> graph_flow::Result<TaskResult> { Ok(TaskResult::new(None, NextAction::End)) } }
    /// let graph = GraphBuilder::new("test")
    ///     .add_task(Arc::new(T) as Arc<dyn Task>)
    ///     .add_conditional_edges(
    ///         "classify",
    ///         |ctx| ctx.get_sync::<String>("category").unwrap_or_default(),
    ///         HashMap::from([
    ///             ("bug".to_string(), "bug_handler".to_string()),
    ///             ("feature".to_string(), "feature_handler".to_string()),
    ///         ]),
    ///     )
    ///     .build();
    /// ```
    pub fn add_conditional_edges<F>(
        self,
        from: impl Into<String>,
        path_fn: F,
        path_map: HashMap<String, String>,
    ) -> Self
    where
        F: Fn(&Context) -> String + Send + Sync + 'static,
    {
        self.graph.add_conditional_edges(from, path_fn, path_map);
        self
    }

    pub fn set_start_task(self, task_id: impl Into<String>) -> Self {
        self.graph.set_start_task(task_id);
        self
    }

    pub fn build(self) -> Graph {
        // Validate the graph before returning
        if self.graph.tasks.is_empty() {
            tracing::warn!("Building graph with no tasks");
        }
        
        // Check for orphaned tasks (tasks with no incoming or outgoing edges)
        let task_count = self.graph.tasks.len();
        if task_count > 1 {
            // Collect task IDs first
            let all_task_ids: Vec<String> = self.graph.tasks.iter()
                .map(|t| t.key().clone())
                .collect();
            
            // Then check edges
            let edges = self.graph.edges.lock().unwrap();
            let mut connected_tasks = std::collections::HashSet::new();
            
            for edge in edges.iter() {
                connected_tasks.insert(edge.from.clone());
                connected_tasks.insert(edge.to.clone());
            }
            drop(edges); // Explicitly drop the lock
            
            // Now check for orphaned tasks
            for task_id in all_task_ids {
                if !connected_tasks.contains(&task_id) {
                    tracing::warn!(
                        task_id = %task_id,
                        "Task has no edges - it may be unreachable"
                    );
                }
            }
        }
        
        self.graph
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
    /// Error occurred during execution
    Error(String),
}
