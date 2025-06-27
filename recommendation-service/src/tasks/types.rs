use serde::{Deserialize, Serialize};

/// Maximum number of retries for answer generation
pub const MAX_RETRIES: u32 = 3;

/// Result of answer validation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    pub passed: bool,
    pub comment: Option<String>,
}

/// Movie data structure for search results
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Movie {
    pub id: i32,
    pub title: String,
    pub overview: String,
}

/// Configuration for the recommendation service
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ServiceConfig {
    pub database_url: String,
    pub movies_database_url: String,
    pub openrouter_api_key: String,
}
