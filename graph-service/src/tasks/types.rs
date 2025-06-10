use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UserDetails {
    pub username: Option<String>,
    pub bank_number: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountDetails {
    pub username: String,
    pub bank_number: String,
    pub account_balance: f64,
    pub account_type: String,
    pub last_transaction: String,
}

// create  a mod with all the types instead of strings
pub mod session_keys {
    pub const USER_INPUT: &str = "user_input";
    pub const USER_DETAILS: &str = "user_details";
    pub const ACCOUNT_DETAILS: &str = "account_details";
}
