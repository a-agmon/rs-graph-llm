# Medical Document Analysis Service

A graph-flow based medical document analysis service that processes PDFs and generates comprehensive medical summaries with human-in-the-loop review.

## LLM Vision OCR Workflow

The core `PdfExtractTask` implements a complete LLM vision-based OCR workflow:

1. **PDF → Images**: Convert pages to high-resolution images using `pdfium-render`
2. **Images → LLM Vision**: Process images with GPT-4V for OCR via OpenRouter API
3. **OCR Text → Summary**: Generate structured medical summary from extracted text

## Quick Test

Test the complete PDF → LLM OCR → Summary workflow:

```bash
# Set your API key
export OPENROUTER_API_KEY="your_key_here"

# Method 1: Using test binary (recommended)
cargo build --bin test_pdf_ocr
./target/debug/test_pdf_ocr /path/to/your/document.pdf

# Method 2: Using test script
./test_pdf_ocr.sh /path/to/your/document.pdf

# Method 3: Using cargo test
export PDF_TEST_PATH="/path/to/your/document.pdf"
cargo test test_pdf_llm_ocr_workflow -- --nocapture
```

See [TEST_GUIDE.md](TEST_GUIDE.md) for detailed testing instructions and troubleshooting.

## Running the Service

```bash
export OPENROUTER_API_KEY="your_key_here"
cargo run --bin medical-document-service
```

## API Usage

```bash
# Start analysis
curl -X POST http://localhost:3000/medical/analyze \
  -H "Content-Type: application/json" \
  -d '{"pdf_path": "/path/to/document.pdf"}'

# Check status
curl http://localhost:3000/medical/{session_id}

# Provide human feedback
curl -X POST http://localhost:3000/medical/{session_id}/resume \
  -H "Content-Type: application/json" \
  -d '{"feedback": "Please add more detail about the treatment plan"}'
```

## Architecture

The service follows the graph-flow pattern with 5 tasks:

1. **PDF Extract** - Extract text and generate initial summary
2. **Human Review** - Pause for human feedback (human-in-the-loop)
3. **Summary Integration** - Combine human feedback with AI analysis
4. **Research Search** - Search PubMed for relevant medical literature
5. **Final Report** - Generate comprehensive medical analysis report

## Dependencies

- **PDF Rendering**: Uses `pdfium-render` for high-quality PDF to image conversion
- **LLM Vision**: Integrates with GPT-4V via OpenRouter API for OCR processing
- **Pure Rust**: No external system dependencies required
- **High Resolution**: Renders PDF pages at 2000px width for optimal OCR accuracy

This implementation works with all PDF types including scanned documents, complex layouts, and image-heavy medical files that traditional text extraction cannot handle.