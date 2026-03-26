use rig::{client::CompletionClient, providers::openrouter};

pub fn get_llm_agent(prompt: &str) -> anyhow::Result<impl rig::completion::Chat + rig::completion::Prompt> {
    let api_key = std::env::var("OPENROUTER_API_KEY")
        .map_err(|_| anyhow::anyhow!("OPENROUTER_API_KEY not set"))?;
    let client = openrouter::Client::new(&api_key)
        .map_err(|e| anyhow::anyhow!("Failed to create OpenRouter client: {}", e))?;
    let agent = client.agent("openai/gpt-4o").preamble(prompt).build();
    Ok(agent)
}
