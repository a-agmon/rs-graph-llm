use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ClaimDetails {
    pub insurance_type: Option<String>, // "car" | "apartment"
    pub description: Option<String>,
    pub estimated_cost: Option<f64>,
    pub additional_info: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaimDecision {
    pub approved: bool,
    pub decision_reason: String,
    pub timestamp: String,
}

// Session keys for the insurance claims workflow
pub mod session_keys {
    pub const USER_INPUT: &str = "user_input";
    pub const CLAIM_DETAILS: &str = "claim_details";
    pub const CLAIM_DECISION: &str = "claim_decision";
    pub const INSURANCE_TYPE: &str = "insurance_type";
    pub const APPROVAL_STATE: &str = "approval_state";
}
