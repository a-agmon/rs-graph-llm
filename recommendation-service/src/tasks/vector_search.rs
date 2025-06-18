use async_trait::async_trait;
use graph_flow::GraphError::TaskExecutionFailed;
use graph_flow::{Context, NextAction, Task, TaskResult};
use sqlx::postgres::PgPoolOptions;
use tracing::info;

use super::utils::embed_query;

/// Task to perform vector search on movie database
pub struct VectorSearchTask {
    pool: sqlx::PgPool,
}

impl VectorSearchTask {
    pub async fn new() -> anyhow::Result<Self> {
        let movies_db_url = std::env::var("MOVIES_DATABASE_URL")
            .map_err(|_| anyhow::anyhow!("MOVIES_DATABASE_URL not set"))?;

        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect(&movies_db_url)
            .await?;

        Ok(Self { pool })
    }
}

#[async_trait]
impl Task for VectorSearchTask {
    fn id(&self) -> &str {
        std::any::type_name::<Self>()
    }

    async fn run(&self, context: Context) -> graph_flow::Result<TaskResult> {
        info!("Starting vector search task");

        let refined_query: String = context
            .get_sync("refined_query")
            .ok_or_else(|| TaskExecutionFailed("refined_query not found in context".into()))?;

        info!("Searching for: {}", refined_query);

        let embedding = embed_query(&refined_query)
            .await
            .map_err(|e| TaskExecutionFailed(format!("Embedding generation failed: {}", e)))?;

        // Build a literal vector representation suitable for pgvector.
        let vector_literal = embedding
            .iter()
            .map(|f| f.to_string())
            .collect::<Vec<_>>()
            .join(",");
        let sql = format!(
            "SELECT id, title, overview                                   \
             FROM movies_with_vectors                                      \
             ORDER BY vector <-> ARRAY[{}]::vector                        \
             LIMIT 25",
            vector_literal
        );

        let rows = sqlx::query_as::<_, (i32, String, String)>(&sql)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| TaskExecutionFailed(format!("Database query failed: {}", e)))?;

        info!("Retrieved {} results from vector search", rows.len());

        // Concatenate the retrieved documents into a single context string.
        let context_block = rows
            .iter()
            .map(|(_, title, overview)| {
                info!(%title, "Retrieved movie");
                format!("Title: {title} Overview: {overview} \n")
            })
            .collect::<Vec<_>>()
            .join("\n---\n");

        context
            .set("retrieved_context", context_block.clone())
            .await;
        info!("Vector search completed successfully");

        Ok(TaskResult::new(None, NextAction::Continue))
    }
} 