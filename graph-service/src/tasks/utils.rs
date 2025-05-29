use rig::{agent::Agent, providers::openrouter};
use tracing::info;

use super::types::AccountDetails;

pub fn get_llm_agent(prompt: &str) -> anyhow::Result<Agent<openrouter::CompletionModel>> {
    let api_key = std::env::var("OPENROUTER_API_KEY")
        .map_err(|_| anyhow::anyhow!("OPENROUTER_API_KEY not set"))?;
    let client = openrouter::Client::new(&api_key);
    let agent = client.agent("openai/gpt-4o-mini").preamble(prompt).build();
    Ok(agent)
}

pub async fn fetch_account_details(
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
