use crate::models::MedicalDocument;
use crate::tasks::*;
use graph_flow::{FlowRunner, Graph, GraphBuilder, Session, SessionStorage, Task};
use std::sync::Arc;
use uuid::Uuid;

pub fn build_medical_workflow() -> Graph {
    let pdf_extract_task = Arc::new(PdfExtractTask);
    let pdf_extract_id = pdf_extract_task.id().to_string();

    let human_review_task = Arc::new(HumanReviewTask);
    let human_review_id = human_review_task.id().to_string();

    let summary_integration_task = Arc::new(SummaryIntegrationTask);
    let summary_integration_id = summary_integration_task.id().to_string();

    let research_search_task = Arc::new(ResearchSearchTask);
    let research_search_id = research_search_task.id().to_string();

    GraphBuilder::new("medical_workflow")
        .add_task(pdf_extract_task)
        .add_task(human_review_task)
        .add_task(summary_integration_task)
        .add_task(research_search_task)
        .add_edge(&pdf_extract_id, &human_review_id)
        .add_edge(&human_review_id, &summary_integration_id)
        .add_edge(&summary_integration_id, &research_search_id)
        .build()
}

pub async fn create_medical_analysis_session(pdf_path: String) -> Session {
    let document = MedicalDocument {
        id: Uuid::new_v4().to_string(),
        pdf_path,
        extracted_text: None,
        initial_summary: None,
        human_feedback: None,
        integrated_summary: None,
        research_keywords: None,
        research_articles: None,
        research_summary: None,
        final_report: None,
    };

    let session_id = Uuid::new_v4().to_string();
    let pdf_extract_task = Arc::new(PdfExtractTask);
    let pdf_extract_id = pdf_extract_task.id().to_string();

    let session = Session::new_from_task(session_id, &pdf_extract_id);
    session.context.set("document", document).await;

    session
}

pub fn create_flow_runner(session_storage: Arc<dyn SessionStorage>) -> FlowRunner {
    let graph = Arc::new(build_medical_workflow());
    FlowRunner::new(graph, session_storage)
}
