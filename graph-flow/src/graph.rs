use dashmap::DashMap;
use std::sync::{Arc, Mutex};

use crate::{
    context::Context,
    error::{GraphError, Result},
    task::{NextAction, Task, TaskResult},
};

/// Edge between tasks in the graph
#[derive(Clone)]
pub struct Edge {
    pub from: String,
    pub to: String,
    pub condition: Option<Arc<dyn Fn(&Context) -> bool + Send + Sync>>,
}

/// A graph of tasks that can be executed
pub struct Graph {
    pub id: String,
    tasks: DashMap<String, Arc<dyn Task>>,
    edges: Mutex<Vec<Edge>>,
    start_task_id: Mutex<Option<String>>,
}

impl Graph {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            tasks: DashMap::new(),
            edges: Mutex::new(Vec::new()),
            start_task_id: Mutex::new(None),
        }
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

    /// Add a conditional edge between tasks
    pub fn add_conditional_edge<F>(
        &self,
        from: impl Into<String>,
        to: impl Into<String>,
        condition: F,
    ) -> &Self
    where
        F: Fn(&Context) -> bool + Send + Sync + 'static,
    {
        self.edges.lock().unwrap().push(Edge {
            from: from.into(),
            to: to.into(),
            condition: Some(Arc::new(condition)),
        });
        self
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
                // Find the next task based on edges
                if let Some(next_task_id) = self.find_next_task(task_id, &context) {
                    Box::pin(self.execute(&next_task_id, context)).await
                } else {
                    Ok(result)
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

    /// Find the next task based on edges and conditions
    pub fn find_next_task(&self, current_task_id: &str, context: &Context) -> Option<String> {
        let edges = self.edges.lock().unwrap();

        // First, check conditional edges
        for edge in edges.iter() {
            if edge.from == current_task_id {
                if let Some(condition) = &edge.condition {
                    if condition(context) {
                        return Some(edge.to.clone());
                    }
                } else {
                    // Default edge without condition
                    return Some(edge.to.clone());
                }
            }
        }
        None
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
        to: impl Into<String>,
        condition: F,
    ) -> Self
    where
        F: Fn(&Context) -> bool + Send + Sync + 'static,
    {
        self.graph.add_conditional_edge(from, to, condition);
        self
    }

    pub fn set_start_task(self, task_id: impl Into<String>) -> Self {
        self.graph.set_start_task(task_id);
        self
    }

    pub fn build(self) -> Graph {
        self.graph
    }
}
