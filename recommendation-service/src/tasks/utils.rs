use anyhow::Result;
use rig::prelude::*;
use tracing::info;

/// Create an LLM agent using OpenRouter
pub fn get_llm_agent() -> Result<rig::agent::Agent<rig::providers::openrouter::CompletionModel>> {
    let api_key = std::env::var("OPENROUTER_API_KEY")
        .map_err(|_| anyhow::anyhow!("OPENROUTER_API_KEY not set"))?;
    let client = rig::providers::openrouter::Client::new(&api_key);
    Ok(client.agent("openai/gpt-4.1-mini").build())
}

/// Generate embedding for text using fastembed
pub async fn embed_query(text: &str) -> Result<Vec<f32>> {
    let input = text.to_owned();
    info!("Generating embedding for text: {}", text);

    // Off-load the potentially expensive ONNX inference to a blocking thread so
    // we don't obstruct Tokio's async scheduler.
    let embedding = tokio::task::spawn_blocking(move || {
        use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};

        let model = TextEmbedding::try_new(
            InitOptions::new(EmbeddingModel::AllMiniLML6V2).with_show_download_progress(true),
        )?;
        let embeddings = model.embed(vec![input], None)?;
        Ok::<Vec<f32>, anyhow::Error>(embeddings.into_iter().next().unwrap())
    })
    .await??;

    info!(
        "Query embedded successfully. Embedding size: {}",
        embedding.len()
    );
    Ok(embedding)
} 