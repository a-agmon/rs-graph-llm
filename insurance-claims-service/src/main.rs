mod tasks;

use crate::tasks::{
    ApartmentInsuranceDetailsTask, CarInsuranceDetailsTask, FinalSummaryTask,
    InitialClaimQueryTask, InsuranceTypeClassifierTask, SmartClaimValidatorTask,
};
use axum::{
    Router,
    extract::{Path, State},
    http::{HeaderValue, Request, StatusCode},
    middleware::{Next, from_fn},
    response::Json,
    routing::{get, post},
};
use graph_flow::{
    FlowRunner, Graph, GraphBuilder, GraphStorage, InMemoryGraphStorage, InMemorySessionStorage,
    PostgresSessionStorage, Session, SessionStorage, Task,
};
use serde::{Deserialize, Serialize};
use std::any::type_name;
use std::sync::Arc;
use tasks::session_keys;
use tower_http::cors::CorsLayer;
use tracing::{Instrument, error, info};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use uuid::Uuid;

#[derive(Clone)]
struct AppState {
    session_storage: Arc<dyn SessionStorage>,
    flow_runner: FlowRunner,
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

/// Initialize structured JSON tracing based on environment variables
fn init_tracing() {
    let log_format = std::env::var("LOG_FORMAT").unwrap_or_else(|_| "json".to_string());
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        "insurance_claims_service=debug,graph_flow=debug,tower_http=debug".into()
    });

    match log_format.as_str() {
        "pretty" => {
            // Human-readable logging for development
            tracing_subscriber::registry()
                .with(env_filter)
                .with(tracing_subscriber::fmt::layer().pretty())
                .init();
        }
        _ => {
            // Structured JSON logging for production
            tracing_subscriber::registry()
                .with(env_filter)
                .with(
                    tracing_subscriber::fmt::layer()
                        .json()
                        .with_target(true)
                        .with_level(true),
                )
                .init();
        }
    }
}

/// Create permissive CORS layer for development/testing
fn create_cors_layer() -> CorsLayer {
    CorsLayer::permissive()
}

/// Middleware to add correlation ID to all requests
async fn correlation_id_middleware(
    mut request: Request<axum::body::Body>,
    next: Next,
) -> axum::response::Response {
    // Generate a correlation ID for this request
    let correlation_id = Uuid::new_v4().to_string();

    // Add correlation ID to request headers for downstream use
    request.headers_mut().insert(
        "x-correlation-id",
        HeaderValue::from_str(&correlation_id).unwrap(),
    );

    // Create a tracing span for this request with correlation ID
    let span = tracing::info_span!("http_request", correlation_id = %correlation_id);

    // Execute the request within the span
    next.run(request).instrument(span).await
}

#[tokio::main]
async fn main() {
    // Initialize structured JSON tracing
    init_tracing();

    // Check if API key is available
    // This is required for LLM-based tasks (CollectUserDetailsTask, AnswerUserRequestsTask)
    if std::env::var("OPENROUTER_API_KEY").is_err() {
        error!("OPENROUTER_API_KEY not set");
        std::process::exit(1);
    }

    // Create storage instances
    let graph_storage = Arc::new(InMemoryGraphStorage::new());

    // Check for DATABASE_URL and use PostgreSQL if available, otherwise use in-memory
    let session_storage: Arc<dyn SessionStorage> =
        if let Ok(database_url) = std::env::var("DATABASE_URL") {
            info!("Using PostgreSQL session storage");
            match PostgresSessionStorage::connect(&database_url).await {
                Ok(postgres_storage) => Arc::new(postgres_storage),
                Err(e) => {
                    error!(
                        "Failed to connect to PostgreSQL: {}. Falling back to in-memory storage.",
                        e
                    );
                    Arc::new(InMemorySessionStorage::new())
                }
            }
        } else {
            info!("Using in-memory session storage (set DATABASE_URL to use PostgreSQL)");
            Arc::new(InMemorySessionStorage::new())
        };

    // Create and store a default graph
    let default_graph = create_default_graph();
    graph_storage
        .save("default".to_string(), Arc::new(default_graph))
        .await
        .expect("Failed to save default graph");

    // Get the graph for FlowRunner creation
    let graph = graph_storage
        .get("default")
        .await
        .expect("Failed to get graph")
        .expect("Graph not found");

    // Create FlowRunner once, share across all requests for maximum efficiency
    let flow_runner = FlowRunner::new(graph, session_storage.clone());

    let app_state = AppState {
        session_storage,
        flow_runner,
    };

    // Build the router with CORS and correlation ID middleware
    let app = Router::new()
        .route("/health", get(health_check))
        .route("/execute", post(execute_graph))
        .route("/session/{id}", get(get_session))
        .layer(create_cors_layer())
        .layer(from_fn(correlation_id_middleware))
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
    let correlation_id = tracing::Span::current()
        .field("correlation_id")
        .map(|f| f.to_string())
        .unwrap_or_else(|| "unknown".to_string());

    info!(
        correlation_id = %correlation_id,
        session_id = ?request.session_id,
        content_length = %request.content.len(),
        "Processing execute request"
    );

    // Use provided session ID or generate a new one
    let (session_id, is_new) = match request.session_id {
        Some(id) => (id, false),
        None => (Uuid::new_v4().to_string(), true),
    };

    // Get or create session
    let session = match state.session_storage.get(&session_id).await {
        Ok(Some(session)) => session,
        Ok(None) if is_new => {
            info!(
                correlation_id = %correlation_id,
                session_id = %session_id,
                "Creating new session"
            );
            Session::new_from_task(session_id.clone(), type_name::<InitialClaimQueryTask>())
        }
        Ok(None) => {
            error!(
                correlation_id = %correlation_id,
                session_id = %session_id,
                "Session not found"
            );
            return Err(StatusCode::NOT_FOUND);
        }
        Err(e) => {
            error!(
                correlation_id = %correlation_id,
                session_id = %session_id,
                error = %e,
                "Failed to get session"
            );
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    // set the current user input and session ID in the session context
    session
        .context
        .set(session_keys::USER_INPUT, request.content)
        .await;

    session.context.set("session_id", session_id.clone()).await;

    // Save the session with updated context before execution
    if let Err(e) = state.session_storage.save(session).await {
        error!(
            correlation_id = %correlation_id,
            session_id = %session_id,
            error = %e,
            "Failed to save session before execution"
        );
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }

    // Execute the workflow using FlowRunner (handles load → execute → save automatically)
    let result = match state.flow_runner.run(&session_id).await {
        Ok(result) => result,
        Err(e) => {
            error!(
                correlation_id = %correlation_id,
                session_id = %session_id,
                error = %e,
                "Failed to execute workflow"
            );
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    info!(
        correlation_id = %correlation_id,
        session_id = %session_id,
        status = ?result.status,
        "Request completed successfully"
    );

    Ok(Json(ExecuteResponse {
        session_id,
        response: result.response,
        status: format!("{:?}", result.status),
    }))
}

async fn get_session(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> Result<Json<Session>, StatusCode> {
    let correlation_id = tracing::Span::current()
        .field("correlation_id")
        .map(|f| f.to_string())
        .unwrap_or_else(|| "unknown".to_string());

    info!(
        correlation_id = %correlation_id,
        session_id = %session_id,
        "Getting session"
    );

    match state.session_storage.get(&session_id).await {
        Ok(Some(session)) => {
            info!(
                correlation_id = %correlation_id,
                session_id = %session_id,
                "Session found"
            );
            Ok(Json(session))
        }
        Ok(None) => {
            info!(
                correlation_id = %correlation_id,
                session_id = %session_id,
                "Session not found"
            );
            Err(StatusCode::NOT_FOUND)
        }
        Err(e) => {
            error!(
                correlation_id = %correlation_id,
                session_id = %session_id,
                error = %e,
                "Failed to get session"
            );
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

fn create_default_graph() -> Graph {
    use crate::tasks::session_keys;

    let mut builder = GraphBuilder::new("simplified_insurance_claims");

    // Create simplified task instances
    let initial_claim_query = Arc::new(InitialClaimQueryTask);
    let insurance_type_classifier = Arc::new(InsuranceTypeClassifierTask);
    let car_insurance_details = Arc::new(CarInsuranceDetailsTask);
    let apartment_insurance_details = Arc::new(ApartmentInsuranceDetailsTask);
    let smart_claim_validator = Arc::new(SmartClaimValidatorTask);
    let final_summary = Arc::new(FinalSummaryTask);

    // Get task IDs
    let initial_id = initial_claim_query.id().to_string();
    let classifier_id = insurance_type_classifier.id().to_string();
    let car_details_id = car_insurance_details.id().to_string();
    let apartment_details_id = apartment_insurance_details.id().to_string();
    let smart_validator_id = smart_claim_validator.id().to_string();
    let final_summary_id = final_summary.id().to_string();

    // Add all tasks to the simplified graph
    builder = builder
        .add_task(initial_claim_query)
        .add_task(insurance_type_classifier)
        .add_task(car_insurance_details)
        .add_task(apartment_insurance_details)
        .add_task(smart_claim_validator)
        .add_task(final_summary);

    // Linear flow from initial query to classifier
    builder = builder.add_edge(initial_id, classifier_id.clone());

    // Conditional routing from classifier to specific details collectors
    builder = builder.add_conditional_edge(
        classifier_id.clone(),
        |context| {
            context
                .get_sync::<String>(session_keys::INSURANCE_TYPE)
                .map(|t| t == "car")
                .unwrap_or(false)
        },
        car_details_id.clone(),       // yes – car branch
        apartment_details_id.clone(), // else – apartment branch
    );

    // Both details collectors flow to smart validator
    builder = builder
        .add_edge(car_details_id, smart_validator_id.clone())
        .add_edge(apartment_details_id, smart_validator_id.clone());

    // Smart validator flows to final summary
    builder = builder.add_edge(smart_validator_id, final_summary_id);

    builder.build()
}
