use async_trait::async_trait;
use graph_flow::GraphError::TaskExecutionFailed;
use graph_flow::{
    Context, ExecutionStatus, FlowRunner, GraphBuilder, GraphStorage, InMemoryGraphStorage,
    NextAction, PostgresSessionStorage, Session, SessionStorage, Task, TaskResult,
};
use rig::completion::Chat;
use rig::prelude::*;
use serde::Deserialize;
use sqlx::postgres::PgPoolOptions;
use std::sync::Arc;
use tracing::{Level, error, info};
use uuid::Uuid;

// Maximum number of retries for answer generation
const MAX_RETRIES: u32 = 3;

// -----------------------------------------------------------------------------
// A wrapper around `rig` so the example compiles only when OPENROUTER_API_KEY is set
// -----------------------------------------------------------------------------
fn get_llm_agent() -> anyhow::Result<rig::agent::Agent<rig::providers::openrouter::CompletionModel>>
{
    let api_key = std::env::var("OPENROUTER_API_KEY")
        .map_err(|_| anyhow::anyhow!("OPENROUTER_API_KEY not set"))?;
    let client = rig::providers::openrouter::Client::new(&api_key);
    Ok(client.agent("openai/gpt-4.1-mini").build())
}

// Helper: obtain an embedding for the refined query using the `fastembed` crate.
async fn embed_query(text: &str) -> anyhow::Result<Vec<f32>> {
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

// -----------------------------------------------------------------------------
// Task 1 – rewrite the user query so that it is more suitable for retrieval
// -----------------------------------------------------------------------------
struct QueryRefinementTask;

#[async_trait]
impl Task for QueryRefinementTask {
    async fn run(&self, context: Context) -> graph_flow::Result<TaskResult> {
        info!("Starting query refinement task");

        let user_query: String = context
            .get_sync("user_query")
            .ok_or_else(|| TaskExecutionFailed("user_query not found in context".into()))?;

        info!("Original user query: {}", user_query);

        let agent = get_llm_agent()
            .map_err(|e| TaskExecutionFailed(format!("Failed to initialize LLM agent: {}", e)))?;

        let refined = agent
            .chat(
                &format!(
                    r#"
                    You are a helpful assistant that rewrites user queries for vector search.
                    Rewrite the following user query so that it is optimised for vector search. Only return the rewritten query.
                    Query: {user_query}"#
                ),
                vec![],
            )
            .await
            .map_err(|e| TaskExecutionFailed(format!("LLM chat failed: {}", e)))?
            .trim()
            .to_string();

        info!("Refined query: {}", refined);
        context.set("refined_query", refined.clone()).await;

        // Initialize retry count
        context.set("retry_count", 0u32).await;

        Ok(TaskResult::new(None, NextAction::Continue))
    }
}

// -----------------------------------------------------------------------------
// Task 2 – run a pgvector similarity search based on the refined query
// -----------------------------------------------------------------------------
struct VectorSearchTask {
    pool: sqlx::PgPool,
}

impl VectorSearchTask {
    async fn new() -> anyhow::Result<Self> {
        let movies_db_url = std::env::var("MOVIES_DATABASE_URL").unwrap();

        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect(&movies_db_url)
            .await?;

        Ok(Self { pool })
    }
}

#[async_trait]
impl Task for VectorSearchTask {
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

// -----------------------------------------------------------------------------
// Task 3 – generate an answer using the retrieved context
// -----------------------------------------------------------------------------
struct AnswerGenerationTask;

#[async_trait]
impl Task for AnswerGenerationTask {
    async fn run(&self, context: Context) -> graph_flow::Result<TaskResult> {
        info!("Starting answer generation task");

        let user_query: String = context
            .get_sync("user_query")
            .ok_or_else(|| TaskExecutionFailed("user_query not found in context".into()))?;

        let ctx: String = context
            .get_sync("retrieved_context")
            .ok_or_else(|| TaskExecutionFailed("retrieved_context not found in context".into()))?;

        let retry_count: u32 = context
            .get_sync("retry_count")
            .ok_or_else(|| TaskExecutionFailed("retry_count not found in context".into()))?;

        info!(
            "Generating answer (attempt {} of {})",
            retry_count + 1,
            MAX_RETRIES + 1
        );

        // Get the full chat history for conversational memory
        let history = context.get_rig_messages().await;

        let agent = get_llm_agent()
            .map_err(|e| TaskExecutionFailed(format!("Failed to initialize LLM agent: {}", e)))?;

        let prompt = if history.is_empty() {
            format!(
                r#"
            You are a movie recommendation assistant.
            Use the following information to answer the user request for a movie recommendation.
            If the information is not sufficient, answer as best you can.
            Information:
            {ctx}
            Question: {user_query}"#
            )

        // if we are running a retry attempt, we only use the context
        } else {
            info!(retry_count = %retry_count, "running a retry attempt");
            format!(
                r#"
            You are a movie recommendation assistant.
            The user asked: "{user_query}"
            
            Based on the validation feedback in our conversation above, and the context above, provide an improved movie recommendation.
            Focus on the specific issues mentioned in the feedback.
            Provide a complete recommendation without referring to previous attempts.
            "#
            )
        };

        let answer = agent
            .chat(&prompt, history)
            .await
            .map_err(|e| TaskExecutionFailed(format!("LLM chat failed: {}", e)))?;

        info!("Answer generated: {}", answer);

        // Add the current answer attempt to chat history
        context.add_user_message(prompt).await;
        context
            .add_assistant_message(format!("Attempt {}: {}", retry_count + 1, answer))
            .await;
        context.set("answer", answer.clone()).await;

        Ok(TaskResult::new(Some(answer), NextAction::Continue))
    }
}

// -----------------------------------------------------------------------------
// Task 4 – validate the generated answer
// -----------------------------------------------------------------------------
#[derive(Deserialize)]
struct ValidationResult {
    passed: bool,
    comment: Option<String>,
}

struct ValidationTask;

#[async_trait]
impl Task for ValidationTask {
    async fn run(&self, context: Context) -> graph_flow::Result<TaskResult> {
        info!("Starting validation task");

        let answer: String = context
            .get_sync("answer")
            .ok_or_else(|| TaskExecutionFailed("answer not found in context".into()))?;

        let user_query: String = context
            .get_sync("user_query")
            .ok_or_else(|| TaskExecutionFailed("user_query not found in context".into()))?;

        let retry_count: u32 = context
            .get_sync("retry_count")
            .ok_or_else(|| TaskExecutionFailed("retry_count not found in context".into()))?;

        info!(
            "Validating answer (attempt {} of {})",
            retry_count + 1,
            MAX_RETRIES + 1
        );

        let prompt = format!(
            r#"
            You are a movie recommendation evaluator.
            Evaluate the following recommendation against the user query.
            Guidelines:
            1 - A good recommendation is relevant to the user query.
            2 - A good recommendation is reasoned.
            3 - A good recommendation includes what the user asked for, and excludes what the user did not ask for.
            4 - If the recommendation is not good, explain why it is not good.
            5 - If the recommendation is good, explain why it is good.
            Respond **only** with JSON of the form \n{{ \"passed\": true/false, \"comment\": \"...\" }}.\n\n
            Query: {user_query}
            Answer: {answer}"#
        );

        let agent = get_llm_agent()
            .map_err(|e| TaskExecutionFailed(format!("Failed to initialize LLM agent: {}", e)))?;

        let raw = agent
            .chat(&prompt, vec![])
            .await
            .map_err(|e| TaskExecutionFailed(format!("LLM chat failed: {}", e)))?;

        // Clean JSON response (remove code blocks if present)
        let cleaned_raw = raw
            .trim()
            .strip_prefix("```json")
            .unwrap_or(&raw)
            .strip_suffix("```")
            .unwrap_or(&raw)
            .trim();

        let validation_result =
            serde_json::from_str::<ValidationResult>(cleaned_raw).map_err(|e| {
                TaskExecutionFailed(format!(
                    "Could not parse validator response: {}. Raw response: {}",
                    e, raw
                ))
            })?;

        context
            .set("validation_passed", &validation_result.passed)
            .await;
        if validation_result.passed {
            info!("Validation passed");
            return Ok(TaskResult::new(None, NextAction::Continue));
        }
        // if we are here, the validation failed - first we get the comment
        if validation_result.comment.is_none() {
            // something went wrong, we should not continue
            return Err(TaskExecutionFailed("No validation comment".into()));
        }
        let comment = validation_result.comment.clone().unwrap();
        info!(comment = %comment, "Validation failed");
        // first we check if we are above the max retries
        if retry_count >= MAX_RETRIES {
            error!(
                "Maximum retry attempts ({}) exceeded. Failing the workflow.",
                MAX_RETRIES
            );
            return Err(TaskExecutionFailed(format!(
                "Maximum retry attempts ({}) exceeded. Last validation comment: {:?}",
                MAX_RETRIES, &validation_result.comment
            )));
        }
        // we still have another chance to try
        // add the comment to the chat history with a explanation of what went wrong
        let validation_message = format!("The answer is not good enough. Reason: {}", comment);
        context.add_user_message(validation_message).await;

        // Increment retry count for the next attempt
        context.set("retry_count", retry_count + 1).await;
        Ok(TaskResult::new(None, NextAction::Continue))
    }
}

// -----------------------------------------------------------------------------
// Task 5 – deliver the final answer (after validation success)
// -----------------------------------------------------------------------------
struct DeliveryTask;

#[async_trait]
impl Task for DeliveryTask {
    async fn run(&self, context: Context) -> graph_flow::Result<TaskResult> {
        info!("Starting delivery task");

        let answer: String = context
            .get_sync("answer")
            .ok_or_else(|| TaskExecutionFailed("answer not found in context".into()))?;

        let retry_count: u32 = context
            .get_sync("retry_count")
            .ok_or_else(|| TaskExecutionFailed("retry_count not found in context".into()))?;

        info!("Delivering final answer after {} retries", retry_count);

        Ok(TaskResult::new(Some(answer), NextAction::End))
    }
}

// -----------------------------------------------------------------------------
// main – wire everything together
// -----------------------------------------------------------------------------
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .compact()
        .init();

    // Accept user question from CLI
    let args: Vec<String> = std::env::args().collect();
    let user_query = if args.len() > 1 {
        args[1..].join(" ")
    } else {
        return Err("Please provide a user query as command line argument".into());
    };

    info!(
        "Starting recommendation flow example with query: {}",
        user_query
    );

    // --- Storage ---------------------------------------------------------------------------

    let database_url = std::env::var("DATABASE_URL").unwrap();
    let session_storage: Arc<dyn SessionStorage> =
        Arc::new(PostgresSessionStorage::connect(&database_url).await?);
    let graph_storage: Arc<dyn GraphStorage> = Arc::new(InMemoryGraphStorage::new());

    // --- Build graph -----------------------------------------------------------------------
    let refine_task: Arc<dyn Task> = Arc::new(QueryRefinementTask);
    let search_task: Arc<dyn Task> = Arc::new(VectorSearchTask::new().await?);
    let answer_task: Arc<dyn Task> = Arc::new(AnswerGenerationTask);
    let validate_task: Arc<dyn Task> = Arc::new(ValidationTask);
    let deliver_task: Arc<dyn Task> = Arc::new(DeliveryTask);

    let refine_id = refine_task.id().to_string();
    let search_id = search_task.id().to_string();
    let answer_id = answer_task.id().to_string();
    let validate_id = validate_task.id().to_string();
    let deliver_id = deliver_task.id().to_string();

    info!("Building workflow graph");
    let graph = Arc::new(
        GraphBuilder::new("recommendation_flow")
            .add_task(refine_task)
            .add_task(search_task)
            .add_task(answer_task)
            .add_task(validate_task)
            .add_task(deliver_task)
            .add_edge(refine_id.clone(), search_id.clone())
            .add_edge(search_id.clone(), answer_id.clone())
            .add_edge(answer_id.clone(), validate_id.clone())
            // Conditional routing: if validation passes go to delivery, else back to answer generation
            .add_conditional_edge(
                validate_id.clone(),
                |ctx| ctx.get_sync::<bool>("validation_passed").unwrap_or(false),
                deliver_id.clone(),
                answer_id.clone(), // Changed from search_id to answer_id for proper feedback loop
            )
            .build(),
    );

    graph_storage
        .save("recommendation_flow".to_string(), graph.clone())
        .await?;

    info!("Graph built and saved successfully");

    // --- Session --------------------------------------------------------------------------
    let session_id = Uuid::new_v4().to_string();
    let session = Session::new_from_task(session_id.clone(), &refine_id);
    session.context.set("user_query", user_query.clone()).await;
    session_storage.save(session.clone()).await?;

    info!(
        session_id = %session_id,
        "Session created"
    );

    // --- Execute --------------------------------------------------------------------------
    let flow_runner = FlowRunner::new(graph.clone(), Arc::clone(&session_storage));

    loop {
        let execution = flow_runner.run(&session_id).await?;

        match execution.status {
            ExecutionStatus::Completed => {
                info!("Workflow completed successfully");
                break;
            }
            ExecutionStatus::Paused { next_task_id } => {
                info!("Workflow paused, will continue to task: {}", next_task_id);
                continue;
            }
            ExecutionStatus::WaitingForInput => {
                info!("Workflow waiting for user input, continuing...");
                continue;
            }
            ExecutionStatus::Error(e) => {
                error!("Workflow error: {}", e);
                return Err(e.into());
            }
        }
    }

    Ok(())
}
