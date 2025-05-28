use async_trait::async_trait;
use graph_flow::{Context, GraphError, NextAction, Result, Task, TaskResult};
use rig::{
    agent::Agent,
    completion::{Chat, Message, Prompt},
    providers::openrouter,
};
use serde::{Deserialize, Serialize};
use tracing::{error, info};

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

/// Task that collects user details (username and bank number)
/// May require multiple interactions if user provides incomplete information
pub struct CollectUserDetailsTask {
    id: String,
}

impl CollectUserDetailsTask {
    pub fn new() -> Self {
        Self {
            id: "collect_user_details".to_string(),
        }
    }
}

#[async_trait]
impl Task for CollectUserDetailsTask {
    fn id(&self) -> &str {
        &self.id
    }

    async fn run(&self, context: Context) -> Result<TaskResult> {
        info!("running task: {}", self.id);
        let user_input: String = context
            .get("user_query")
            .await
            .ok_or_else(|| GraphError::ContextError("user_query not found".to_string()))?;

        info!("Collecting user details from input: {}", user_input);

        // Get message history from context (if any)
        let mut chat_history: Vec<Message> = context.get("chat_history").await.unwrap_or_default();

        // Check if API key is available
        if std::env::var("OPENROUTER_API_KEY").is_err() {
            error!("OPENROUTER_API_KEY not set, using fallback response");
            return Ok(TaskResult {
                response: Some(
                    "Hello! To help you with your banking needs, I need your username and bank number. Please provide both details.".to_string()
                ),
                next_action: NextAction::WaitForInput,
            });
        }

        // Create agent with collection prompt
        let agent = get_llm_agent(COLLECT_USER_DETAILS_PROMPT)?;

        // Use chat to get response with history
        let response = agent
            .chat(&user_input, chat_history.clone())
            .await
            .map_err(|e| GraphError::TaskExecutionFailed(e.to_string()))?;

        // Add user message and assistant response to history
        chat_history.push(Message::user(user_input));
        chat_history.push(Message::assistant(response.clone()));

        // Store updated chat history
        context.set("chat_history", chat_history).await;

        // Try to parse JSON from response to check if we have complete details
        if let Ok(user_details) = serde_json::from_str::<UserDetails>(&response) {
            if user_details.username.is_some() && user_details.bank_number.is_some() {
                // We have complete details, store them and continue
                context.set("user_details", user_details.clone()).await;
                info!(
                    "All user details collected: {:?} - {:?}",
                    user_details.username, user_details.bank_number
                );

                return Ok(TaskResult {
                    response: Some(format!(
                        "Thank you! I have your username ({}) and bank number ({}). Let me fetch your account details.",
                        user_details.username.unwrap(),
                        user_details.bank_number.unwrap()
                    )),
                    next_action: NextAction::Continue,
                });
            }
        }

        // If we don't have complete details or couldn't parse JSON,
        // the response should be a guiding question
        Ok(TaskResult {
            response: Some(response),
            next_action: NextAction::WaitForInput,
        })
    }
}

/// Task that fetches account details using the collected user information
pub struct FetchAccountDetailsTask {
    id: String,
}

impl FetchAccountDetailsTask {
    pub fn new() -> Self {
        Self {
            id: "fetch_account_details".to_string(),
        }
    }
}

#[async_trait]
impl Task for FetchAccountDetailsTask {
    fn id(&self) -> &str {
        &self.id
    }

    async fn run(&self, context: Context) -> Result<TaskResult> {
        info!("running task: {}", self.id);
        let user_details: UserDetails = context
            .get("user_details")
            .await
            .ok_or_else(|| GraphError::ContextError("user_details not found".to_string()))?;

        let username = user_details.username.ok_or_else(|| {
            GraphError::ContextError("username not found in user_details".to_string())
        })?;
        let bank_number = user_details.bank_number.ok_or_else(|| {
            GraphError::ContextError("bank_number not found in user_details".to_string())
        })?;

        info!(
            "Fetching account details for: {} - {}",
            username, bank_number
        );

        // Simulate fetching account details from a banking API
        let account_details = fetch_account_details(&username, &bank_number)
            .await
            .map_err(|e| GraphError::TaskExecutionFailed(e.to_string()))?;

        // Store account details in context
        context
            .set("account_details", account_details.clone())
            .await;

        Ok(TaskResult {
            response: Some(format!(
                "Account details retrieved successfully! Your {} account ending in {} has a balance of ${:.2}. How can I help you today?",
                account_details.account_type,
                &bank_number[bank_number.len() - 4..],
                account_details.account_balance
            )),
            next_action: NextAction::WaitForInput,
        })
    }
}

/// Task that answers user requests about their account
pub struct AnswerUserRequestsTask {
    id: String,
}

impl AnswerUserRequestsTask {
    pub fn new() -> Self {
        Self {
            id: "answer_user_requests".to_string(),
        }
    }
}

#[async_trait]
impl Task for AnswerUserRequestsTask {
    fn id(&self) -> &str {
        &self.id
    }

    async fn run(&self, context: Context) -> Result<TaskResult> {
        info!("running task: {}", self.id);
        let user_query: String = context
            .get("user_query")
            .await
            .ok_or_else(|| GraphError::ContextError("user_query not found".to_string()))?;

        let account_details: AccountDetails = context
            .get("account_details")
            .await
            .ok_or_else(|| GraphError::ContextError("account_details not found".to_string()))?;

        info!("Answering user request: {}", user_query);

        // Use LLM to answer the user's question about their account
        let response = answer_user_request(&user_query, &account_details)
            .await
            .map_err(|e| GraphError::TaskExecutionFailed(e.to_string()))?;

        Ok(TaskResult {
            response: Some(response),
            next_action: NextAction::WaitForInput, // Keep the conversation going
        })
    }
}

// Helper functions

const COLLECT_USER_DETAILS_PROMPT: &str = r#"You are a banking assistant collecting username and bank number.

ANALYZE THE CONVERSATION HISTORY AND EXTRACT:
- Username: any name the user provides
- Bank number: any number sequence (8-15 digits) the user provides

WHEN USER SAYS:
- "My username is john" → username = "john"
- "My number 1234567891" → bank_number = "1234567891"
- "My bank number is 9876543210" → bank_number = "9876543210"
- "The number is 1122334455" → bank_number = "1122334455"

IF YOU HAVE BOTH username AND bank_number, respond with ONLY this JSON:
{
  "username": "extracted_username",
  "bank_number": "extracted_number"
}

IF MISSING INFO, ask for what's needed.

IMPORTANT: In banking context, when users say "my number" they mean their bank/account number, NOT phone number. Always extract numbers as bank numbers."#;

const ANSWER_REQUEST_PROMPT: &str = r#"You are a helpful banking assistant. Answer the user's question about their account using the provided account details.
Be friendly, professional, and provide accurate information based on the account data.
If the user asks about something not available in the account details, politely explain what information you have access to."#;

pub fn get_llm_agent(prompt: &str) -> anyhow::Result<Agent<openrouter::CompletionModel>> {
    let api_key = std::env::var("OPENROUTER_API_KEY")
        .map_err(|_| anyhow::anyhow!("OPENROUTER_API_KEY not set"))?;
    let client = openrouter::Client::new(&api_key);
    let agent = client.agent("openai/gpt-4o-mini").preamble(prompt).build();
    Ok(agent)
}

async fn fetch_account_details(
    username: &str,
    bank_number: &str,
) -> anyhow::Result<AccountDetails> {
    // Simulate API call to banking system
    // In a real implementation, this would call an actual banking API

    info!(
        "Simulating account fetch for {} - {}",
        username, bank_number
    );

    // Simulate some processing time
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    // Return mock account details
    Ok(AccountDetails {
        username: username.to_string(),
        bank_number: bank_number.to_string(),
        account_balance: 2547.83,
        account_type: "Checking".to_string(),
        last_transaction: "Grocery Store - $45.67 on 2024-01-15".to_string(),
    })
}

async fn answer_user_request(
    user_query: &str,
    account_details: &AccountDetails,
) -> anyhow::Result<String> {
    // Check if API key is available
    if std::env::var("OPENROUTER_API_KEY").is_err() {
        // Fallback: provide basic account information without LLM
        return Ok(format!(
            "I can help you with your account information. Here's what I have:
- Account Type: {}
- Current Balance: ${:.2}
- Last Transaction: {}

For your question '{}', I'd recommend contacting customer service for detailed assistance since I don't have access to the AI assistant right now.",
            account_details.account_type,
            account_details.account_balance,
            account_details.last_transaction,
            user_query
        ));
    }

    let agent = get_llm_agent(ANSWER_REQUEST_PROMPT)?;

    let context = format!(
        "Account Details:
- Username: {}
- Account Type: {}
- Balance: ${:.2}
- Last Transaction: {}

User Question: {}",
        account_details.username,
        account_details.account_type,
        account_details.account_balance,
        account_details.last_transaction,
        user_query
    );

    let response = agent.prompt(&context).await?;
    Ok(response)
}
