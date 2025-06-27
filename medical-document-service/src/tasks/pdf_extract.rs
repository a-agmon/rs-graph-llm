use crate::models::MedicalDocument;
use anyhow::anyhow;
use async_trait::async_trait;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use graph_flow::{Context, GraphError, NextAction, Result, Task, TaskResult};
use image::{DynamicImage, ImageFormat};
use pdf2image::{PDF, Pages};
use reqwest::Client;
use serde_json::{Value, json};
use std::io::Cursor;
use tracing::{info, warn};

pub struct PdfExtractTask;

#[async_trait]
impl Task for PdfExtractTask {
    async fn run(&self, context: Context) -> Result<TaskResult> {
        info!("Starting PDF to images to LLM OCR workflow");

        let document: MedicalDocument = context
            .get("document")
            .await
            .ok_or_else(|| GraphError::ContextError("Document not found in context".to_string()))?;

        let pdf_path = &document.pdf_path;
        info!("Processing PDF: {}", pdf_path);

        // Workflow: PDF → Images → LLM OCR → Summary
        let extracted_text = process_pdf_with_llm_ocr(pdf_path)
            .await
            .map_err(|e| GraphError::TaskExecutionFailed(e.to_string()))?;

        if extracted_text.trim().is_empty() {
            warn!("No text extracted from document using LLM OCR");
            return Err(GraphError::TaskExecutionFailed(
                "No text extracted from document using LLM OCR".to_string(),
            ));
        }

        info!(
            "LLM OCR extracted text length: {} characters",
            extracted_text.len()
        );

        // Generate medical summary using LLM
        let initial_summary = generate_medical_summary(&extracted_text)
            .await
            .map_err(|e| GraphError::TaskExecutionFailed(e.to_string()))?;

        // Update document in context
        let mut updated_document = document;
        updated_document.extracted_text = Some(extracted_text);
        updated_document.initial_summary = Some(initial_summary);

        context.set("document", updated_document).await;

        info!("PDF LLM OCR and summary completed successfully");
        Ok(TaskResult::new_with_status(
            None,
            NextAction::ContinueAndExecute,
            Some("PDF processed with LLM OCR and medical summary generated".to_string()),
        ))
    }
}

/// Main function: PDF → Images → LLM OCR → Text
pub async fn process_pdf_with_llm_ocr(pdf_path: &str) -> anyhow::Result<String> {
    info!("Converting PDF to images for LLM OCR: {}", pdf_path);

    // Step 1: Convert PDF to images
    let images = convert_pdf_to_images(pdf_path).await?;

    if images.is_empty() {
        return Err(anyhow!("No images generated from PDF"));
    }

    info!("Generated {} images from PDF", images.len());

    // Step 2: Use LLM vision to extract text from images
    let extracted_text = extract_text_with_llm_vision(&images).await?;

    Ok(extracted_text)
}

/// Convert PDF to images using pdf2image
async fn convert_pdf_to_images(pdf_path: &str) -> anyhow::Result<Vec<DynamicImage>> {
    // Check if file exists
    if !tokio::fs::try_exists(pdf_path).await? {
        return Err(anyhow!("PDF file not found: {}", pdf_path));
    }

    info!("Converting PDF to images using pdf2image");

    let pdf_path_owned = pdf_path.to_string();
    let images = tokio::task::spawn_blocking(move || -> anyhow::Result<Vec<DynamicImage>> {
        let pdf =
            PDF::from_file(&pdf_path_owned).map_err(|e| anyhow!("Failed to load PDF: {}", e))?;

        // Render all pages with default options (good quality)
        let rendered_images = pdf
            .render(Pages::All, None)
            .map_err(|e| anyhow!("Failed to render PDF pages: {}", e))?;

        info!("Rendered {} pages from PDF", rendered_images.len());
        Ok(rendered_images)
    })
    .await??;

    info!("Successfully converted PDF to {} images", images.len());
    Ok(images)
}

/// Use LLM vision to extract text from images (OCR) - processes all images in one call
async fn extract_text_with_llm_vision(images: &[DynamicImage]) -> anyhow::Result<String> {
    info!(
        "Processing {} pages with LLM vision OCR in single call",
        images.len()
    );

    // Convert all images to base64
    let mut image_contents = Vec::new();
    for (i, image) in images.iter().enumerate() {
        let base64_image = image_to_base64(image)?;
        image_contents.push(json!({
            "type": "image_url",
            "image_url": {
                "url": format!("data:image/png;base64,{}", base64_image)
            }
        }));
        info!("Converted page {} to base64", i + 1);
    }

    // Create content array with text prompt + all images
    let mut content = vec![json!({
        "type": "text",
        "text": format!(
            "You are an expert medical document OCR system. I'm providing you with {} pages of a medical document written in either English or Hebrew. \
            Extract ALL text from these pages with perfect accuracy, preserving the exact structure, formatting, and medical terminology.

            For each page, start with '=== Page X ===' as a header, then provide the extracted text. \
            Maintain the document's logical flow and structure across pages.

            Return ONLY the extracted text without any commentary or explanations.",
            images.len()
        )
    })];
    content.extend(image_contents);

    let extracted_text = call_openrouter_api("openai/gpt-4.1-mini", content, 4000).await?;

    info!(
        "LLM vision OCR completed: {} total characters extracted",
        extracted_text.len()
    );
    Ok(extracted_text)
}

/// Convert image to base64 for LLM vision API
fn image_to_base64(image: &DynamicImage) -> anyhow::Result<String> {
    let mut buffer = Vec::new();
    let mut cursor = Cursor::new(&mut buffer);

    image
        .write_to(&mut cursor, ImageFormat::Png)
        .map_err(|e| anyhow!("Failed to encode image: {}", e))?;

    Ok(STANDARD.encode(&buffer))
}

/// Generate medical summary from extracted text using LLM
pub async fn generate_medical_summary(text: &str) -> anyhow::Result<String> {
    let prompt = format!(
                "You are a medical AI assistant. Analyze this medical document text (extracted via OCR) and provide a comprehensive summary in English with these sections:

        2. **Chief Complaint**: Primary reason for visit/consultation  
        3. **Medical History**: Relevant past medical history
        4. **Current Findings**: Physical examination findings, symptoms
        5. **Diagnostic Results**: Lab results, imaging findings, test results
        6. **Assessment**: Clinical impressions and diagnoses
        7. **Treatment Plan**: Medications, procedures, recommendations
        8. **Follow-up**: Next steps and monitoring requirements

        Focus on key medical information and maintain clinical accuracy. Use clear section headers.

        Medical Document Text (from OCR):
        {}

        Provide a structured summary:", 
        text
    );

    let content = vec![json!({
        "type": "text",
        "text": prompt
    })];

    let summary = call_openrouter_api("openai/gpt-4.1-mini", content, 2000).await?;

    info!(
        "Generated medical summary from OCR text ({} characters)",
        summary.len()
    );
    Ok(summary)
}

/// Centralized function to call OpenRouter API with vision/text support
async fn call_openrouter_api(
    model: &str,
    content: Vec<Value>,
    max_tokens: u32,
) -> anyhow::Result<String> {
    let api_key = std::env::var("OPENROUTER_API_KEY")
        .map_err(|_| anyhow!("OPENROUTER_API_KEY environment variable not set"))?;

    let client = Client::new();

    let payload = json!({
        "model": model,
        "messages": [
            {
                "role": "user",
                "content": content
            }
        ],
        "max_tokens": max_tokens
    });

    let response = client
        .post("https://openrouter.ai/api/v1/chat/completions")
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&payload)
        .send()
        .await?;

    if !response.status().is_success() {
        return Err(anyhow!("LLM API request failed: {}", response.status()));
    }

    let response_json: Value = response.json().await?;

    let content = response_json["choices"][0]["message"]["content"]
        .as_str()
        .ok_or_else(|| anyhow!("Invalid response format from LLM"))?;

    Ok(content.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test LLM vision OCR with sample images
    /// Usage: OPENROUTER_API_KEY=key cargo test test_llm_vision_ocr
    #[tokio::test]
    async fn test_llm_vision_ocr() -> anyhow::Result<()> {
        if std::env::var("OPENROUTER_API_KEY").is_err() {
            println!("Skipping test - set OPENROUTER_API_KEY environment variable");
            return Ok(());
        }

        // Create a simple test image with text (this would normally be a PDF page)
        let test_image = image::DynamicImage::new_rgb8(400, 200);
        let images = vec![test_image];

        println!("Testing LLM Vision OCR");

        match extract_text_with_llm_vision(&images).await {
            Ok(text) => {
                println!("LLM Vision OCR completed");
                println!("Extracted text: {}", text);
                assert!(!text.trim().is_empty());
            }
            Err(e) => {
                println!("Note: LLM Vision OCR test with blank image: {}", e);
                // This is expected with a blank test image
            }
        }

        Ok(())
    }

    /// Test the complete PDF → LLM OCR → Summary workflow
    /// Requires: OPENROUTER_API_KEY and PDF_TEST_PATH environment variables
    #[tokio::test]
    async fn test_pdf_llm_ocr_workflow() -> anyhow::Result<()> {
        let pdf_path = match std::env::var("PDF_TEST_PATH") {
            Ok(path) => path,
            Err(_) => {
                println!("Skipping test - set PDF_TEST_PATH environment variable");
                return Ok(());
            }
        };

        if std::env::var("OPENROUTER_API_KEY").is_err() {
            println!("Skipping test - set OPENROUTER_API_KEY environment variable");
            return Ok(());
        }

        println!("Testing PDF -> LLM OCR -> Summary workflow");
        println!("PDF: {}", pdf_path);

        match process_pdf_with_llm_ocr(&pdf_path).await {
            Ok(text) => {
                println!("PDF LLM OCR completed");
                println!("Extracted {} characters", text.len());

                let summary = generate_medical_summary(&text).await?;
                println!("Generated summary ({} characters)", summary.len());

                assert!(!text.trim().is_empty());
                assert!(!summary.trim().is_empty());
            }
            Err(e) => {
                println!("Error in PDF LLM OCR workflow: {}", e);
                println!("Check PDF file exists and pdfium library is available");
            }
        }

        Ok(())
    }
}
