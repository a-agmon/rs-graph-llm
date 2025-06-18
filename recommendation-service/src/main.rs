mod tasks;

use axum::{
    extract::Query,
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use graph_flow::{
    Context, ExecutionStatus, GraphBuilder, GraphStorage, InMemoryGraphStorage,
    PostgresSessionStorage, Session, SessionStorage, Task,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tower::ServiceBuilder;
use tower_http::trace::TraceLayer;
use tracing::{error, info, Level};
use uuid::Uuid;

use tasks::{
    AnswerGenerationTask, DeliveryTask, QueryRefinementTask, ValidationTask, VectorSearchTask,
};

#[derive(Debug, Deserialize)]
struct RecommendationRequest {
    query: String,
}

#[derive(Debug, Serialize)]
struct RecommendationResponse {
    session_id: String,
    answer: String,
    status: String,
}

#[derive(Debug, Serialize)]
struct ErrorResponse {
    error: String,
}

#[derive(Clone)]
struct AppState {
    session_storage: Arc<dyn SessionStorage>,
    graph_storage: Arc<dyn GraphStorage>,
}

async fn health_check() -> &'static str {
    "OK"
}

async fn recommend(
    Query(params): Query<RecommendationRequest>,
    axum::extract::State(state): axum::extract::State<AppState>,
) -> Result<Json<RecommendationResponse>, (StatusCode, Json<ErrorResponse>)> {
    info!("Received recommendation request: {}", params.query);

    // Get the graph
    let graph = state
        .graph_storage
        .get("recommendation_flow")
        .await
        .map_err(|e| {
            error!("Failed to get graph: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Failed to get workflow graph".to_string(),
                }),
            )
        })?
        .ok_or_else(|| {
            error!("Graph not found");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Workflow graph not found".to_string(),
                }),
            )
        })?;

    // Create new session
    let session_id = Uuid::new_v4().to_string();
    let refine_task_id = std::any::type_name::<QueryRefinementTask>();

    // Set up context with chat history limit
    let context = Context::with_max_chat_messages(50);
    context.set("user_query", params.query.clone()).await;

    let session = Session {
        id: session_id.clone(),
        graph_id: "recommendation_flow".to_string(),
        current_task_id: refine_task_id.to_string(),
        status_message: None,
        context,
    };

    state
        .session_storage
        .save(session.clone())
        .await
        .map_err(|e| {
            error!("Failed to save session: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Failed to save session".to_string(),
                }),
            )
        })?;

    info!("Session created with ID: {}", session_id);

    // Execute workflow until completion
    let mut final_answer = String::new();
    loop {
        let mut current_session = state
            .session_storage
            .get(&session_id)
            .await
            .map_err(|e| {
                error!("Failed to get session: {}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorResponse {
                        error: "Session not found".to_string(),
                    }),
                )
            })?
            .ok_or_else(|| {
                error!("Session not found");
                (
                    StatusCode::NOT_FOUND,
                    Json(ErrorResponse {
                        error: "Session not found".to_string(),
                    }),
                )
            })?;

        info!("Executing task: {}", current_session.current_task_id);

        let execution = graph
            .execute_session(&mut current_session)
            .await
            .map_err(|e| {
                error!("Failed to execute session: {}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorResponse {
                        error: format!("Workflow execution failed: {}", e),
                    }),
                )
            })?;

        state
            .session_storage
            .save(current_session)
            .await
            .map_err(|e| {
                error!("Failed to save session: {}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorResponse {
                        error: "Failed to save session".to_string(),
                    }),
                )
            })?;

        if let Some(response) = execution.response {
            final_answer = response;
        }

        match execution.status {
            ExecutionStatus::Completed => {
                info!("Workflow completed successfully");
                break;
            }
            ExecutionStatus::WaitingForInput => {
                info!("Workflow waiting for input, continuing...");
                continue;
            }
            ExecutionStatus::Error(e) => {
                error!("Workflow error: {}", e);
                return Err((
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorResponse {
                        error: format!("Workflow failed: {}", e),
                    }),
                ));
            }
        }
    }

    Ok(Json(RecommendationResponse {
        session_id,
        answer: final_answer,
        status: "completed".to_string(),
    }))
}

async fn setup_graph(
    graph_storage: Arc<dyn GraphStorage>,
) -> Result<(), Box<dyn std::error::Error>> {
    info!("Setting up recommendation workflow graph");

    // Create tasks
    let refine_task: Arc<dyn Task> = Arc::new(QueryRefinementTask);
    let search_task: Arc<dyn Task> = Arc::new(VectorSearchTask::new().await?);
    let answer_task: Arc<dyn Task> = Arc::new(AnswerGenerationTask);
    let validate_task: Arc<dyn Task> = Arc::new(ValidationTask);
    let deliver_task: Arc<dyn Task> = Arc::new(DeliveryTask);

    let refine_id = refine_task.id().to_string();
    let search_id = search_task.id().to_string();
    let answer_id = answer_task.id().to_string();
    let validate_id = validate_task.id().to_string();
    let deliver_id = deliver_task.id().to_string();

    // Build graph
    let graph = Arc::new(
        GraphBuilder::new("recommendation_flow")
            .add_task(refine_task)
            .add_task(search_task)
            .add_task(answer_task)
            .add_task(validate_task)
            .add_task(deliver_task)
            .add_edge(refine_id.clone(), search_id.clone())
            .add_edge(search_id.clone(), answer_id.clone())
            .add_edge(answer_id.clone(), validate_id.clone())
            // Conditional routing: if validation passes go to delivery, else back to answer generation
            .add_conditional_edge(
                validate_id.clone(),
                |ctx| ctx.get_sync::<bool>("validation_passed").unwrap_or(false),
                deliver_id.clone(),
                answer_id.clone(), // Back to answer generation for retry
            )
            .build(),
    );

    graph_storage
        .save("recommendation_flow".to_string(), graph)
        .await?;

    info!("Graph built and saved successfully");
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .compact()
        .init();

    info!("Starting recommendation service");

    // Setup storage
    let database_url =
        std::env::var("DATABASE_URL").map_err(|_| "DATABASE_URL environment variable not set")?;

    let session_storage: Arc<dyn SessionStorage> =
        Arc::new(PostgresSessionStorage::connect(&database_url).await?);
    let graph_storage: Arc<dyn GraphStorage> = Arc::new(InMemoryGraphStorage::new());

    // Setup graph
    setup_graph(graph_storage.clone()).await?;

    // Create app state
    let state = AppState {
        session_storage,
        graph_storage,
    };

    // Build router
    let app = Router::new()
        .route("/health", get(health_check))
        .route("/recommend", post(recommend))
        .with_state(state)
        .layer(ServiceBuilder::new().layer(TraceLayer::new_for_http()));

    // Start server
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await?;
    info!("Server running on http://0.0.0.0:3000");

    info!("Available endpoints:");
    info!("  GET  /health     - Health check");
    info!("  POST /recommend  - Generate movie recommendation");
    info!("    Example: POST /recommend?query=action%20movies%20with%20great%20fight%20scenes");

    axum::serve(listener, app).await?;

    Ok(())
}
