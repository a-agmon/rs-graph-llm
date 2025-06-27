use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MedicalDocument {
    pub id: String,
    pub pdf_path: String,
    pub extracted_text: Option<String>,
    pub initial_summary: Option<String>,
    pub human_feedback: Option<String>,
    pub integrated_summary: Option<String>,
    pub research_keywords: Option<Vec<String>>,
    pub research_articles: Option<Vec<ResearchArticle>>,
    pub research_summary: Option<String>,
    pub final_report: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResearchArticle {
    pub pmid: String,
    pub title: String,
    pub abstract_text: String,
    pub authors: Option<String>,
    pub journal: Option<String>,
    pub publication_date: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AnalyzeDocumentRequest {
    pub pdf_path: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HumanFeedbackRequest {
    pub feedback: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SessionResponse {
    pub session_id: String,
    pub status: String,
    pub current_task: Option<String>,
    pub status_message: Option<String>,
    pub context: HashMap<String, serde_json::Value>,
    pub waiting_for_input: bool,
}