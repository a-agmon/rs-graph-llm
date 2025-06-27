use anyhow::Result;
use medical_document_service::tasks::pdf_extract::{
    generate_medical_summary, process_pdf_with_llm_ocr,
};
use std::env;
use tracing_subscriber;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt().with_env_filter("info").init();

    println!("Medical Document PDF -> LLM OCR -> Summary Test");
    println!("===============================================");

    // Get PDF path from command line argument
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage: {} <pdf_file_path>", args[0]);
        eprintln!("Example: {} /path/to/medical/document.pdf", args[0]);
        std::process::exit(1);
    }

    let pdf_path = &args[1];

    // Check if API key is set
    if env::var("OPENROUTER_API_KEY").is_err() {
        eprintln!("Please set OPENROUTER_API_KEY environment variable");
        eprintln!("Example: export OPENROUTER_API_KEY='your_key_here'");
        std::process::exit(1);
    }

    println!("Processing PDF: {}", pdf_path);
    println!(
        "API Key: {}...",
        env::var("OPENROUTER_API_KEY").unwrap()[..10].to_string()
    );
    println!();

    // Step 1: PDF -> Images -> LLM OCR -> Text
    println!("Step 1: PDF -> Images -> LLM OCR");
    println!("   Converting PDF to images...");
    println!("   Processing images with GPT-4V...");

    match process_pdf_with_llm_ocr(pdf_path).await {
        Ok(extracted_text) => {
            println!(
                "OCR completed: {} characters extracted",
                extracted_text.len()
            );
            println!();

            // Show first 500 characters of extracted text
            println!("Extracted Text (first 500 chars):");
            println!("────────────────────────────────────");
            let preview = if extracted_text.chars().count() > 500 {
                let truncated: String = extracted_text.chars().take(500).collect();
                format!("{}...", truncated)
            } else {
                extracted_text.clone()
            };
            println!("{}", preview);
            println!();

            // Step 2: Generate medical summary
            println!("Step 2: Generating Medical Summary");
            println!("   Processing with medical AI...");

            match generate_medical_summary(&extracted_text).await {
                Ok(summary) => {
                    println!("Summary generated: {} characters", summary.len());
                    println!();

                    println!("Medical Summary:");
                    println!("──────────────────");
                    println!("{}", summary);
                    println!();

                    println!("Complete workflow successful!");
                    println!();
                    println!("Summary:");
                    println!("   - PDF processed successfully");
                    println!("   - OCR extracted {} characters", extracted_text.len());
                    println!("   - Medical summary: {} characters", summary.len());
                }
                Err(e) => {
                    eprintln!("Failed to generate medical summary: {}", e);
                    std::process::exit(1);
                }
            }
        }
        Err(e) => {
            eprintln!("Failed to process PDF with LLM OCR: {}", e);
            eprintln!();
            eprintln!("Troubleshooting:");
            eprintln!("   - Check PDF file exists and is readable");
            eprintln!("   - Ensure OPENROUTER_API_KEY is valid");
            eprintln!("   - Verify pdfium library is available");
            std::process::exit(1);
        }
    }

    Ok(())
}
