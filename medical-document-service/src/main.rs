use medical_document_service::create_app;
use tokio::net::TcpListener;
use tracing::{Level, info};
use tracing_subscriber::{EnvFilter, FmtSubscriber};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    let subscriber = FmtSubscriber::builder()
        .with_env_filter(EnvFilter::from_default_env().add_directive(Level::INFO.into()))
        .finish();
    tracing::subscriber::set_global_default(subscriber)?;

    // Check required environment variables
    if std::env::var("OPENROUTER_API_KEY").is_err() {
        eprintln!("Error: OPENROUTER_API_KEY environment variable is required");
        std::process::exit(1);
    }

    let port = std::env::var("PORT")
        .unwrap_or_else(|_| "3000".to_string())
        .parse::<u16>()
        .unwrap_or(3000);

    let app = create_app().await;
    let listener = TcpListener::bind(format!("0.0.0.0:{}", port)).await?;
    let addr = listener.local_addr()?;

    info!("Medical Document Analysis Service starting on {}", addr);
    info!("API Documentation available at http://{}/", addr);
    info!("ealth check endpoint: http://{}/health", addr);
    info!("Analysis endpoint: POST http://{}/medical/analyze", addr);

    axum::serve(listener, app).await?;

    Ok(())
}
