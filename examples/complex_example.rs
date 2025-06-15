use async_trait::async_trait;
use graph_flow::{
    Context, ExecutionStatus, GraphBuilder, GraphStorage, InMemoryGraphStorage,
    InMemorySessionStorage, NextAction, Session, SessionStorage, Task, TaskResult,
};
use rig::completion::Chat;
use rig::prelude::*;
use serde::Deserialize;
use std::sync::Arc;
use tracing::{Level, info};

// --- Sentiment analysis helpers -------------------------------------------------------------
#[derive(Deserialize)]
struct SentimentResponse {
    sentiment: String,
}

const SENTIMENT_PROMPT: &str = r#"You are a helpful sentiment analysis assistant.

ANALYZE THE USER INPUT AND RESPOND **ONLY** WITH ONE OF THE FOLLOWING JSON OBJECTS:
{ "sentiment": "positive" }
{ "sentiment": "negative" }

If you are not sure, ask a short clarifying question **instead** of returning JSON. Do not add any additional text around the JSON.
"#;

/// Very small wrapper around `rig` to obtain an agent that can answer our prompt.
fn get_llm_agent() -> anyhow::Result<rig::agent::Agent<rig::providers::openrouter::CompletionModel>>
{
    let api_key = std::env::var("OPENROUTER_API_KEY")
        .map_err(|_| anyhow::anyhow!("OPENROUTER_API_KEY not set"))?;
    let client = rig::providers::openrouter::Client::new(&api_key);
    Ok(client
        .agent("openai/gpt-4o-mini")
        .preamble(SENTIMENT_PROMPT)
        .build())
}

// --- Task A: run sentiment analysis ---------------------------------------------------------
struct SentimentAnalysisTask;

#[async_trait]
impl Task for SentimentAnalysisTask {
    fn id(&self) -> &str {
        std::any::type_name::<Self>()
    }

    async fn run(&self, context: Context) -> graph_flow::Result<TaskResult> {
        // Pull the user input we stored in the session context
        let user_input: String = context
            .get_sync("user_input")
            .unwrap_or_else(|| "".to_string());

        // Build the LLM agent
        let agent = match get_llm_agent() {
            Ok(a) => a,
            Err(e) => {
                // If the agent cannot be created (for example, the API key is missing) we fall back
                // to a dummy implementation so that this example can still be executed without an LLM.
                info!(error = %e, "Falling back to dummy sentiment detection (LLM not available)");
                return self.dummy_sentiment(context, user_input).await;
            }
        };

        // We are not using chat history here for simplicity, but rig expects a vector – supply an empty one.
        let response = agent
            .chat(&user_input, vec![])
            .await
            .map_err(|e| graph_flow::GraphError::TaskExecutionFailed(e.to_string()))?;

        // Try to parse the JSON response returned by the LLM
        if let Ok(parsed) = serde_json::from_str::<SentimentResponse>(response.trim()) {
            let sentiment = parsed.sentiment;
            info!(sentiment, "Sentiment detected – continuing");
            // Persist the sentiment in the context so that the conditional edge can read it.
            context.set("sentiment", sentiment.clone()).await;

            // We want to proceed straight to the next task and execute it immediately.
            return Ok(TaskResult::new(None, NextAction::ContinueAndExecute));
        }

        // If we are here the model did not return the expected JSON – treat its reply as a clarifying question.
        context.add_assistant_message(response.clone()).await;
        Ok(TaskResult::new(
            Some(response),
            NextAction::WaitForInput, // Wait for the user to answer the clarifying question.
        ))
    }
}

impl SentimentAnalysisTask {
    // Very small heuristic fallback in case an LLM is not available.
    async fn dummy_sentiment(
        &self,
        context: Context,
        user_input: String,
    ) -> graph_flow::Result<TaskResult> {
        let lowered = user_input.to_lowercase();
        let sentiment = if lowered.contains("good") || lowered.contains("love") {
            "positive"
        } else {
            "negative"
        };
        context.set("sentiment", sentiment.to_string()).await;
        Ok(TaskResult::new(None, NextAction::Continue))
    }
}

// --- Task B: positive branch ----------------------------------------------------------------
struct PositiveResponseTask;

#[async_trait]
impl Task for PositiveResponseTask {
    fn id(&self) -> &str {
        std::any::type_name::<Self>()
    }

    async fn run(&self, _context: Context) -> graph_flow::Result<TaskResult> {
        let reply = "That is awesome to hear! Keep up the good vibes.".to_string();
        Ok(TaskResult::new(Some(reply), NextAction::End))
    }
}

// --- Task C: negative branch ----------------------------------------------------------------
struct NegativeResponseTask;

#[async_trait]
impl Task for NegativeResponseTask {
    fn id(&self) -> &str {
        std::any::type_name::<Self>()
    }

    async fn run(&self, _context: Context) -> graph_flow::Result<TaskResult> {
        let reply = "I am sorry to hear that. Let me know if there is anything I can do to help."
            .to_string();
        Ok(TaskResult::new(Some(reply), NextAction::End))
    }
}

// --------------------------------------------------------------------------------------------
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // A little bit of logging so that the flow is easier to follow when the example is run.
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .compact()
        .init();

    // Capture the user input that we want to analyse. If none is supplied we fall back to a default.
    let args: Vec<String> = std::env::args().collect();
    let user_input = if args.len() > 1 {
        args[1..].join(" ")
    } else {
        "I feel good today".to_string()
    };

    info!(%user_input, "Starting complex example");

    // --- Storage ---------------------------------------------------------------------------------
    let session_storage: Arc<dyn SessionStorage> = Arc::new(InMemorySessionStorage::new());
    let graph_storage: Arc<dyn GraphStorage> = Arc::new(InMemoryGraphStorage::new());

    // --- Build graph -----------------------------------------------------------------------------
    let sentiment_task: Arc<dyn Task> = Arc::new(SentimentAnalysisTask);
    let positive_task: Arc<dyn Task> = Arc::new(PositiveResponseTask);
    let negative_task: Arc<dyn Task> = Arc::new(NegativeResponseTask);

    let sentiment_id = sentiment_task.id().to_string();
    let positive_id = positive_task.id().to_string();
    let negative_id = negative_task.id().to_string();

    let graph = Arc::new(
        GraphBuilder::new("sentiment_flow")
            .add_task(sentiment_task)
            .add_task(positive_task)
            .add_task(negative_task)
            // Conditional routing based on the sentiment detected in the first task
            .add_conditional_edge(
                sentiment_id.clone(),
                |context| {
                    context
                        .get_sync::<String>("sentiment")
                        .map(|s| s == "positive")
                        .unwrap_or(false)
                },
                positive_id.clone(),
                negative_id.clone(),
            )
            .build(),
    );

    graph_storage
        .save("sentiment_flow".to_string(), graph.clone())
        .await?;

    // --- Session ---------------------------------------------------------------------------------
    let session_id = "sentiment_session_001".to_string();
    let mut session = Session::new_from_task(session_id.clone(), &sentiment_id);

    // Seed the session context with the user input gathered on the command line
    session.context.set("user_input", user_input.clone()).await;

    // Persist the session before we start executing the graph
    session_storage.save(session.clone()).await?;

    // --- Execution ------------------------------------------------------------------------------
    loop {
        let mut current_session = session_storage
            .get(&session_id)
            .await?
            .ok_or("Session not found")?;

        info!(task = %current_session.current_task_id, "Executing task");
        let execution_result = graph.execute_session(&mut current_session).await?;
        session_storage.save(current_session.clone()).await?;

        if let Some(response) = execution_result.response {
            println!("Assistant: {}", response);
        }

        match execution_result.status {
            ExecutionStatus::Completed => {
                info!("Workflow completed");
                break;
            }
            ExecutionStatus::WaitingForInput => {
                // For brevity we end the example here. In a real application we'd collect more user input.
                println!("The workflow is waiting for additional input – exiting example.");
                break;
            }
            ExecutionStatus::Error(err) => {
                println!("Error: {}", err);
                break;
            }
        }
    }

    Ok(())
}
