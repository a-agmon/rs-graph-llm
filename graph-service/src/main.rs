mod chat_bridge;
mod tasks;

use crate::tasks::{
    InitialClaimQueryTask, InsuranceTypeClassifierTask, CarInsuranceDetailsTask,
    ApartmentInsuranceDetailsTask, SmartClaimValidatorTask, FinalSummaryTask,
};
use axum::{
    Router,
    extract::{Path, State},
    http::StatusCode,
    response::Json,
    routing::{get, post},
};
use graph_flow::{
    Graph, GraphBuilder, GraphStorage, InMemoryGraphStorage, InMemorySessionStorage, Session,
    SessionStorage, Task, PostgresSessionStorage,
};
use serde::{Deserialize, Serialize};
use std::any::type_name;
use std::sync::Arc;
use tasks::session_keys;
use tracing::{error, info};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use uuid::Uuid;

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

    // Check if API key is available
    // This is required for LLM-based tasks (CollectUserDetailsTask, AnswerUserRequestsTask)
    if std::env::var("OPENROUTER_API_KEY").is_err() {
        error!("OPENROUTER_API_KEY not set");
        std::process::exit(1);
    }
    
    // Create storage instances
    let graph_storage = Arc::new(InMemoryGraphStorage::new());
    
    // Check for DATABASE_URL and use PostgreSQL if available, otherwise use in-memory
    let session_storage: Arc<dyn SessionStorage> = if let Ok(database_url) = std::env::var("DATABASE_URL") {
        info!("Using PostgreSQL session storage");
        match PostgresSessionStorage::connect(&database_url).await {
            Ok(postgres_storage) => Arc::new(postgres_storage),
            Err(e) => {
                error!("Failed to connect to PostgreSQL: {}. Falling back to in-memory storage.", e);
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

    // Check if session_id was provided for validation
    let session_id_provided = request.session_id.is_some();

    // Get or create session id
    let session_id = request
        .session_id
        .unwrap_or_else(|| Uuid::new_v4().to_string());

    // Validate session ID format if provided
    if session_id_provided && Uuid::parse_str(&session_id).is_err() {
        error!("Invalid session ID format: {}", session_id);
        return Err(StatusCode::BAD_REQUEST);
    }

    // Get or create session
    let mut session = match state.session_storage.get(&session_id).await {
        Ok(Some(session)) => session,
        Ok(None) => {
            // Only create new session if session_id was not provided
            // If session_id was provided but not found, return error
            if session_id_provided {
                error!("Session not found: {}", session_id);
                return Err(StatusCode::NOT_FOUND);
            }
            Session::new_from_task(session_id.clone(), type_name::<InitialClaimQueryTask>())
        }
        Err(e) => {
            error!("Failed to get session: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    // set the current user input in the session
    session
        .context
        .set(session_keys::USER_INPUT, request.content)
        .await;

    // Get or create the relevant graph type id
    let graph = get_or_create_graph(state.graph_storage.clone()).await?;

    // Execute the the next task in the graph
    let result = match graph.execute_session(&mut session).await {
        Ok(result) => result,
        Err(e) => {
            error!("Failed to execute graph: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    // persist the session
    if let Err(e) = state.session_storage.save(session).await {
        error!("Failed to save session: {}", e);
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }

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
        car_details_id.clone(),
        |context| {
            context.get_sync::<String>(session_keys::INSURANCE_TYPE)
                .map(|t| t == "car")
                .unwrap_or(false)
        }
    );

    builder = builder.add_conditional_edge(
        classifier_id,
        apartment_details_id.clone(),
        |context| {
            context.get_sync::<String>(session_keys::INSURANCE_TYPE)
                .map(|t| t == "apartment")
                .unwrap_or(false)
        }
    );

    // Both details collectors flow to smart validator
    builder = builder
        .add_edge(car_details_id, smart_validator_id.clone())
        .add_edge(apartment_details_id, smart_validator_id.clone());

    // Smart validator flows to final summary
    builder = builder.add_edge(smart_validator_id, final_summary_id);

    builder.build()
}

async fn get_or_create_graph(
    graph_storage: Arc<dyn GraphStorage>,
) -> Result<Arc<Graph>, StatusCode> {
    let graphid = "default";
    // Get or create the relevant graph type id
    match graph_storage.get(graphid).await {
        Ok(Some(graph)) => Ok(graph),
        Ok(None) => {
            error!("Graph not found: {}", graphid);
            Err(StatusCode::NOT_FOUND)
        }
        Err(e) => {
            error!("Failed to get graph: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}
