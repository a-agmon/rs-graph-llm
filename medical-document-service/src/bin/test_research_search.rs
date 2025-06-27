use graph_flow::{Context, Task};
use medical_document_service::models::MedicalDocument;
use medical_document_service::tasks::research_search::ResearchSearchTask;
use std::env;
use tracing::{error, info};
use tracing_subscriber;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt().with_env_filter("info").init();

    info!("Starting research search test");

    // Check for API key
    if env::var("OPENROUTER_API_KEY").is_err() {
        error!("OPENROUTER_API_KEY environment variable not set");
        error!("Please set your OpenRouter API key to test the research search functionality");
        return Ok(());
    }

    // Create a sample medical document with integrated summary
    let sample_document = MedicalDocument {
        id: "test-doc-001".to_string(),
        pdf_path: "/path/to/test.pdf".to_string(),
        extracted_text: Some("Sample medical report text...".to_string()),
        initial_summary: Some("Initial medical summary...".to_string()),
        human_feedback: None,
        integrated_summary: Some(
            "Patient presents with acute myocardial infarction (heart attack) with ST-elevation on ECG. \
             Laboratory results show elevated troponin levels and CK-MB. Patient has history of hypertension \
             and hyperlipidemia. Current symptoms include chest pain, shortness of breath, and diaphoresis. \
             Requires immediate cardiac catheterization and possible percutaneous coronary intervention (PCI). \
             Risk factors include smoking history and family history of coronary artery disease.".to_string()
        ),
        research_keywords: None,
        research_articles: None,
        research_summary: None,
        final_report: None,
    };

    info!("Created sample medical document with integrated summary");
    info!(
        "Summary: {}",
        sample_document.integrated_summary.as_ref().unwrap()
    );

    // Create context and add the document
    let context = Context::new();
    context.set("document", sample_document).await;

    info!("Context prepared, running research search task...");

    // Create and run the research search task
    let research_task = ResearchSearchTask;

    match research_task.run(context.clone()).await {
        Ok(result) => {
            info!("Research search task completed successfully!");

            if let Some(status_message) = &result.status_message {
                info!("Status: {}", status_message);
            }

            // Retrieve the updated document from context
            if let Some(updated_document) = context.get::<MedicalDocument>("document").await {
                info!("\n=== RESEARCH SEARCH RESULTS ===");

                if let Some(keywords) = &updated_document.research_keywords {
                    info!("\nSearch Keywords Generated:");
                    for (i, keyword) in keywords.iter().enumerate() {
                        info!("  {}: {}", i + 1, keyword);
                    }
                }

                if let Some(articles) = &updated_document.research_articles {
                    info!("\nResearch Articles Found: {}", articles.len());
                    for (i, article) in articles.iter().enumerate() {
                        info!("\nArticle {}:", i + 1);
                        info!("  PMID: {}", article.pmid);
                        info!("  Title: {}", article.title);
                        if let Some(journal) = &article.journal {
                            info!("  Journal: {}", journal);
                        }
                        info!(
                            "  Abstract: {}...",
                            article.abstract_text.chars().take(200).collect::<String>()
                        );
                    }
                }

                if let Some(summary) = &updated_document.research_summary {
                    info!("\nResearch Summary:");
                    info!("{}", summary);
                }
            } else {
                error!("Could not retrieve updated document from context");
            }
        }
        Err(e) => {
            error!("Research search task failed: {}", e);
        }
    }

    info!("Test completed");
    Ok(())
}
