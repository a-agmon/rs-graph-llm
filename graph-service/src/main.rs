mod tasks;

use axum::{
    Router,
    extract::{Path, State},
    http::StatusCode,
    response::Json,
    routing::{get, post},
};
use graph_flow::{
    Context, Graph, GraphBuilder, GraphStorage, InMemoryGraphStorage, InMemorySessionStorage,
    Session, SessionStorage,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use uuid::Uuid;

use crate::tasks::{AnswerUserRequestsTask, CollectUserDetailsTask, FetchAccountDetailsTask};

#[derive(Clone)]
struct AppState {
    graph_storage: Arc<dyn GraphStorage>,
    session_storage: Arc<dyn SessionStorage>,
}

#[derive(Debug, Deserialize)]
struct ExecuteRequest {
    session_id: Option<String>,
    content: String,
}

#[derive(Debug, Serialize)]
struct ExecuteResponse {
    session_id: String,
    response: Option<String>,
    status: String,
}

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "graph_service=debug,graph_flow=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Create storage instances
    let graph_storage = Arc::new(InMemoryGraphStorage::new());
    let session_storage = Arc::new(InMemorySessionStorage::new());

    // Create and store a default graph
    let default_graph = create_default_graph();
    graph_storage
        .save("default".to_string(), Arc::new(default_graph))
        .await
        .expect("Failed to save default graph");

    let app_state = AppState {
        graph_storage,
        session_storage,
    };

    // Build the router
    let app = Router::new()
        .route("/health", get(health_check))
        .route("/execute", post(execute_graph))
        .route("/session/{id}", get(get_session))
        // .layer(TraceLayer::new_for_http())
        .with_state(app_state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();

    info!("Server running on http://0.0.0.0:3000");

    axum::serve(listener, app).await.unwrap();
}

async fn health_check() -> &'static str {
    "OK"
}

async fn execute_graph(
    State(state): State<AppState>,
    Json(request): Json<ExecuteRequest>,
) -> Result<Json<ExecuteResponse>, StatusCode> {
    info!("Execute request: {:?}", request);

    // Get or create session
    let session_id = request
        .session_id
        .unwrap_or_else(|| Uuid::new_v4().to_string());

    let mut session = match state.session_storage.get(&session_id).await {
        Ok(Some(session)) => session,
        Ok(None) => {
            // Create new session with default graph
            let context = Context::new();
            Session {
                id: session_id.clone(),
                graph_id: "default".to_string(),
                current_task_id: "collect_user_details".to_string(),
                context,
            }
        }
        Err(e) => {
            error!("Failed to get session: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    // Get the graph
    let graph = match state.graph_storage.get(&session.graph_id).await {
        Ok(Some(graph)) => graph,
        Ok(None) => {
            error!("Graph not found: {}", session.graph_id);
            return Err(StatusCode::NOT_FOUND);
        }
        Err(e) => {
            error!("Failed to get graph: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    // Set the user input in context
    session.context.set("user_query", request.content).await;

    // Execute the graph
    let result = match graph
        .execute(&session.current_task_id, session.context.clone())
        .await
    {
        Ok(result) => result,
        Err(e) => {
            error!("Failed to execute graph: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    // Update session based on result
    match &result.next_action {
        graph_flow::NextAction::WaitForInput => {
            // The task that returned WaitForInput is the one we should continue from
            session.current_task_id = result.task_id.clone();
        }
        graph_flow::NextAction::GoTo(task_id) => {
            // The graph execution engine already handled the transition,
            // but we need to update our session to reflect where we ended up
            session.current_task_id = task_id.clone();
        }
        graph_flow::NextAction::End => {
            // Session completed, could clean up if needed
        }
        graph_flow::NextAction::Continue => {
            // This shouldn't happen in our current setup, because Continue should
            // cause the graph to keep executing until it hits WaitForInput or End.
            // If we get here, it means the graph execution completed all tasks.
            // We'll assume we're at the end of the workflow.
        }
        _ => {
            // For other actions (like GoBack), don't change the current_task_id
        }
    }

    // Save updated session
    if let Err(e) = state.session_storage.save(session).await {
        error!("Failed to save session: {}", e);
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }

    Ok(Json(ExecuteResponse {
        session_id,
        response: result.response,
        status: format!("{:?}", result.next_action),
    }))
}

async fn get_session(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> Result<Json<Session>, StatusCode> {
    match state.session_storage.get(&session_id).await {
        Ok(Some(session)) => Ok(Json(session)),
        Ok(None) => Err(StatusCode::NOT_FOUND),
        Err(e) => {
            error!("Failed to get session: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

fn create_default_graph() -> Graph {
    let mut builder = GraphBuilder::new("default");

    // Add tasks
    builder = builder
        .add_task(Arc::new(CollectUserDetailsTask::new()))
        .add_task(Arc::new(FetchAccountDetailsTask::new()))
        .add_task(Arc::new(AnswerUserRequestsTask::new()));

    // Add edges
    builder = builder
        .add_edge("collect_user_details", "fetch_account_details")
        .add_edge("fetch_account_details", "answer_user_requests");

    builder.build()
}
