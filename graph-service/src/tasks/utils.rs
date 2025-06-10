use rig::{agent::Agent, providers::openrouter};

pub fn get_llm_agent(prompt: &str) -> anyhow::Result<Agent<openrouter::CompletionModel>> {
    let api_key = std::env::var("OPENROUTER_API_KEY")
        .map_err(|_| anyhow::anyhow!("OPENROUTER_API_KEY not set"))?;
    let client = openrouter::Client::new(&api_key);
    let agent = client.agent("openai/gpt-4o-mini").preamble(prompt).build();
    Ok(agent)
}

/// Extract cost amount from text using simple parsing
pub fn extract_cost_from_text(text: &str) -> Option<f64> {
    // Look for patterns like $1000, $1,000.00, 1000, etc.
    let re = regex::Regex::new(r"[\$]?([0-9,]+\.?[0-9]*)")
        .expect("Invalid regex");
    
    if let Some(caps) = re.captures(text) {
        if let Some(amount_str) = caps.get(1) {
            let cleaned = amount_str.as_str().replace(",", "");
            return cleaned.parse::<f64>().ok();
        }
    }
    None
}
