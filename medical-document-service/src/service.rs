use axum::{
    Router,
    extract::{Path, State},
    http::StatusCode,
    response::Json,
    routing::{get, post},
};
use graph_flow::{ExecutionStatus, FlowRunner, PostgresSessionStorage, SessionStorage};
use serde_json::{Value, json};
use std::sync::Arc;
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing::{error, info};

use crate::{
    models::{AnalyzeDocumentRequest, HumanFeedbackRequest, MedicalDocument, SessionResponse},
    workflow::{create_flow_runner, create_medical_analysis_session},
};

type ApiResult<T> = Result<Json<T>, (StatusCode, Json<Value>)>;
type ApiError = (StatusCode, Json<Value>);

fn bad_request_error(message: &str) -> ApiError {
    (StatusCode::BAD_REQUEST, Json(json!({ "error": message })))
}

fn not_found_error(message: &str, id: &str) -> ApiError {
    (
        StatusCode::NOT_FOUND,
        Json(json!({
            "error": message,
            "session_id": id
        })),
    )
}

fn internal_error(message: &str, details: &str) -> ApiError {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({
            "error": message,
            "details": details
        })),
    )
}

#[derive(Clone)]
pub struct AppState {
    pub session_storage: Arc<dyn SessionStorage>,
    pub flow_runner: FlowRunner,
}

pub async fn create_app() -> Router {
    let app_state = create_app_state().await;
    build_router(app_state)
}

async fn create_app_state() -> AppState {
    let session_storage = create_session_storage().await;
    let flow_runner = create_flow_runner(session_storage.clone());

    AppState {
        session_storage,
        flow_runner,
    }
}

async fn create_session_storage() -> Arc<dyn SessionStorage> {
    let database_url =
        std::env::var("DATABASE_URL").expect("DATABASE_URL environment variable must be set");

    let pg_session_storage = PostgresSessionStorage::connect(&database_url)
        .await
        .unwrap_or_else(|e| {
            error!("Failed to connect to PostgreSQL: {}", e);
            std::process::exit(1);
        });

    Arc::new(pg_session_storage)
}

fn build_router(app_state: AppState) -> Router {
    Router::new()
        .route("/", get(root))
        .route("/health", get(health_check))
        .route("/medical/analyze", post(start_analysis))
        .route("/medical/{session_id}", get(get_session_status))
        .route("/medical/{session_id}/resume", post(provide_feedback))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(app_state)
}

async fn root() -> Json<Value> {
    Json(json!({
        "service": "Medical Document Analysis Service",
        "version": "1.0.0",
        "description": "AI-powered medical document analysis with human-in-the-loop review",
        "endpoints": {
            "POST /medical/analyze": "Start new document analysis",
            "GET /medical/{session_id}": "Get session status and results",
            "POST /medical/{session_id}/resume": "Provide human feedback to resume workflow",
            "GET /health": "Health check"
        }
    }))
}

async fn health_check() -> Json<Value> {
    Json(json!({
        "status": "healthy",
        "timestamp": chrono::Utc::now().to_rfc3339()
    }))
}

async fn start_analysis(
    State(state): State<AppState>,
    Json(request): Json<AnalyzeDocumentRequest>,
) -> ApiResult<Value> {
    info!(
        "Starting medical document analysis for: {}",
        request.pdf_path
    );

    validate_pdf_path(&request.pdf_path)?;

    let session = create_medical_analysis_session(request.pdf_path.clone()).await;
    let session_id = session.id.clone();

    save_session(&state, session).await?;
    start_workflow(&state, &session_id).await
}

fn validate_pdf_path(pdf_path: &str) -> Result<(), ApiError> {
    if pdf_path.trim().is_empty() {
        return Err(bad_request_error("PDF path is required"));
    }
    Ok(())
}

async fn save_session(state: &AppState, session: graph_flow::Session) -> Result<(), ApiError> {
    state.session_storage.save(session).await.map_err(|e| {
        error!("Failed to create session: {}", e);
        internal_error("Failed to create analysis session", &e.to_string())
    })
}

async fn start_workflow(state: &AppState, session_id: &str) -> ApiResult<Value> {
    info!("Session {} created successfully", session_id);

    match state.flow_runner.run(session_id).await {
        Ok(result) => {
            info!(
                "Workflow execution started for session {}: {:?}",
                session_id, result.status
            );

            // If workflow completed immediately, update session to reflect completion
            if matches!(result.status, ExecutionStatus::Completed) {
                if let Ok(Some(mut session)) = state.session_storage.get(session_id).await {
                    session.context.set("workflow_completed", true).await;
                    session.current_task_id = "completed".to_string();
                    if let Err(e) = state.session_storage.save(session).await {
                        error!(
                            "Failed to save completion status for session {}: {}",
                            session_id, e
                        );
                    }
                }
            }

            Ok(Json(json!({
                "session_id": session_id,
                "status": "started",
                "message": "Medical document analysis started successfully"
            })))
        }
        Err(e) => {
            error!("Failed to start workflow for session {}: {}", session_id, e);
            Err(internal_error(
                "Failed to start analysis workflow",
                &e.to_string(),
            ))
        }
    }
}

async fn get_session_status(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> ApiResult<SessionResponse> {
    info!("Getting status for session: {}", session_id);

    match state.session_storage.get(&session_id).await {
        Ok(Some(session)) => {
            let context_map = build_context_map(&session).await;
            let waiting_for_feedback = session
                .context
                .get("waiting_for_human_feedback")
                .await
                .unwrap_or(false);

            let workflow_completed = session
                .context
                .get("workflow_completed")
                .await
                .unwrap_or(false);

            // Determine the actual status based on workflow state
            let status = if workflow_completed {
                "completed".to_string()
            } else if waiting_for_feedback {
                "waiting_for_input".to_string()
            } else {
                "active".to_string()
            };

            let response = SessionResponse {
                session_id: session.id.clone(),
                status,
                current_task: Some(session.current_task_id.clone()),
                status_message: session.status_message.clone(),
                context: context_map,
                waiting_for_input: waiting_for_feedback,
            };

            Ok(Json(response))
        }
        Ok(None) => Err(not_found_error("Session not found", &session_id)),
        Err(e) => {
            error!("Failed to load session {}: {}", session_id, e);
            Err(internal_error("Failed to load session", &e.to_string()))
        }
    }
}

async fn build_context_map(
    session: &graph_flow::Session,
) -> std::collections::HashMap<String, serde_json::Value> {
    let mut context_map = std::collections::HashMap::new();

    if let Some(document) = session.context.get::<MedicalDocument>("document").await {
        context_map.insert(
            "document".to_string(),
            serde_json::to_value(&document).unwrap_or(serde_json::Value::Null),
        );

        add_document_fields_to_context(&document, &mut context_map);
    }

    context_map
}

fn add_document_fields_to_context(
    document: &MedicalDocument,
    context_map: &mut std::collections::HashMap<String, serde_json::Value>,
) {
    if let Some(summary) = &document.initial_summary {
        context_map.insert("initial_summary".to_string(), json!(summary));
    }
    if let Some(integrated) = &document.integrated_summary {
        context_map.insert("integrated_summary".to_string(), json!(integrated));
    }
    if let Some(research) = &document.research_summary {
        context_map.insert("research_summary".to_string(), json!(research));
    }
    if let Some(final_report) = &document.final_report {
        context_map.insert("final_report".to_string(), json!(final_report));
    }
    if let Some(keywords) = &document.research_keywords {
        context_map.insert(
            "research_keywords".to_string(),
            serde_json::to_value(keywords).unwrap_or(serde_json::Value::Null),
        );
    }
}

async fn provide_feedback(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
    Json(request): Json<HumanFeedbackRequest>,
) -> ApiResult<Value> {
    info!("Providing feedback for session: {}", session_id);

    validate_feedback(&request.feedback)?;

    match state.session_storage.get(&session_id).await {
        Ok(Some(session)) => {
            update_session_with_feedback(&session, &request.feedback).await;
            save_session_after_feedback(&state, session).await?;
            resume_workflow_with_feedback(&state, &session_id).await
        }
        Ok(None) => Err(not_found_error("Session not found", &session_id)),
        Err(e) => {
            error!("Failed to load session {}: {}", session_id, e);
            Err(internal_error("Failed to load session", &e.to_string()))
        }
    }
}

fn validate_feedback(feedback: &str) -> Result<(), ApiError> {
    if feedback.trim().is_empty() {
        return Err(bad_request_error("Feedback cannot be empty"));
    }
    Ok(())
}

async fn update_session_with_feedback(session: &graph_flow::Session, feedback: &str) {
    session
        .context
        .set("human_feedback", feedback.to_string())
        .await;

    // Clear the waiting flag since feedback has been provided
    session
        .context
        .set("waiting_for_human_feedback", false)
        .await;

    if let Some(mut document) = session.context.get::<MedicalDocument>("document").await {
        document.human_feedback = Some(feedback.to_string());
        session.context.set("document", document).await;
    }
}

async fn save_session_after_feedback(
    state: &AppState,
    session: graph_flow::Session,
) -> Result<(), ApiError> {
    state.session_storage.save(session).await.map_err(|e| {
        error!("Failed to save session with feedback: {}", e);
        internal_error("Failed to save feedback", &e.to_string())
    })
}

async fn resume_workflow_with_feedback(state: &AppState, session_id: &str) -> ApiResult<Value> {
    match state.flow_runner.run(session_id).await {
        Ok(result) => {
            info!(
                "Workflow resumed for session {}: {:?}",
                session_id, result.status
            );

            // If workflow completed, update session to reflect completion
            if matches!(result.status, ExecutionStatus::Completed) {
                if let Ok(Some(mut session)) = state.session_storage.get(session_id).await {
                    session.context.set("workflow_completed", true).await;
                    session.current_task_id = "completed".to_string();
                    if let Err(e) = state.session_storage.save(session).await {
                        error!(
                            "Failed to save completion status for session {}: {}",
                            session_id, e
                        );
                    }
                }
            }

            Ok(Json(build_feedback_response(session_id, result)))
        }
        Err(e) => {
            error!(
                "Failed to resume workflow for session {}: {}",
                session_id, e
            );
            Err(internal_error(
                "Failed to resume workflow after feedback",
                &e.to_string(),
            ))
        }
    }
}

fn build_feedback_response(session_id: &str, result: graph_flow::ExecutionResult) -> Value {
    let mut response = json!({
        "session_id": session_id,
        "status": "resumed",
        "message": "Feedback received and workflow resumed",
        "execution_status": format!("{:?}", result.status)
    });

    if matches!(result.status, ExecutionStatus::Completed) {
        if let Some(research_summary) = result.response {
            response["research_summary"] = json!(research_summary);
            response["message"] = json!("Medical document analysis completed successfully");
        }
    }

    response
}
