# Movie Recommendation Service

An Axum-based web service that provides AI-powered movie recommendations using a multi-step workflow with vector search, answer generation, and validation.

## Features

- **Query Refinement**: Optimizes user queries for vector search
- **Vector Search**: Performs semantic search on movie database using pgvector
- **Answer Generation**: Uses OpenRouter/GPT-4o-mini to generate recommendations
- **Validation**: Validates recommendations and retries if needed (up to 3 attempts)
- **Chat History**: Maintains conversation context for improved iterations
- **RESTful API**: Simple HTTP endpoints for easy integration

## Architecture

The service is built using:
- **Axum**: Web framework for HTTP server
- **graph-flow**: Workflow engine for task orchestration
- **PostgreSQL**: Session storage and vector database
- **OpenRouter**: LLM provider for text generation
- **fastembed**: Local embeddings generation

## Workflow

1. **Query Refinement**: Rewrites user query for better vector search
2. **Vector Search**: Finds similar movies using embeddings
3. **Answer Generation**: Creates recommendation using retrieved context
4. **Validation**: Evaluates answer quality
5. **Retry Logic**: Improves answer if validation fails (max 3 attempts)
6. **Delivery**: Returns final validated recommendation

## Environment Variables

```bash
# Required
DATABASE_URL=postgresql://user:password@localhost:5432/sessions_db
MOVIES_DATABASE_URL=postgresql://user:password@localhost:5432/movies_db
OPENROUTER_API_KEY=your_openrouter_api_key

# Optional
RUST_LOG=info
```

## Database Setup

The service requires two PostgreSQL databases:

1. **Sessions Database**: Stores workflow session state
2. **Movies Database**: Contains movie data with vector embeddings

### Movies Database Schema

```sql
CREATE TABLE movies_with_vectors (
    id SERIAL PRIMARY KEY,
    title TEXT NOT NULL,
    overview TEXT,
    vector vector(384)  -- Adjust dimension based on your embedding model
);

-- Add vector similarity search index
CREATE INDEX movies_vector_idx ON movies_with_vectors USING ivfflat (vector vector_cosine_ops);
```

## API Endpoints

### Health Check
```http
GET /health
```

Returns `OK` if service is running.

### Generate Recommendation
```http
POST /recommend?query=<your_query>
```

**Parameters:**
- `query` (required): The movie recommendation query

**Example:**
```bash
curl -X POST "http://localhost:3000/recommend?query=action%20movies%20with%20great%20fight%20scenes"
```

**Response:**
```json
{
  "session_id": "123e4567-e89b-12d3-a456-426614174000",
  "answer": "Based on your interest in action movies with great fight scenes, I highly recommend...",
  "status": "completed"
}
```

**Error Response:**
```json
{
  "error": "Error description"
}
```

## Running the Service

### Development
```bash
# Install dependencies
cargo build

# Set environment variables
export DATABASE_URL="postgresql://user:password@localhost:5432/sessions_db"
export MOVIES_DATABASE_URL="postgresql://user:password@localhost:5432/movies_db"
export OPENROUTER_API_KEY="your_api_key"

# Run the service
cargo run
```

### Production
```bash
# Build release binary
cargo build --release

# Run with production settings
RUST_LOG=info ./target/release/recommendation-service
```

The service runs on `http://0.0.0.0:3000` by default.

## Project Structure

```
recommendation-service/
├── src/
│   ├── main.rs              # Axum server and workflow setup
│   └── tasks/               # Task modules
│       ├── mod.rs           # Module exports
│       ├── types.rs         # Shared types and constants
│       ├── utils.rs         # Utility functions (LLM, embeddings)
│       ├── query_refinement.rs    # Query optimization task
│       ├── vector_search.rs       # Vector similarity search task
│       ├── answer_generation.rs   # LLM answer generation task
│       ├── validation.rs          # Answer validation task
│       └── delivery.rs            # Final answer delivery task
├── Cargo.toml
└── README.md
```

## Task Details

### QueryRefinementTask
- Optimizes user queries for better vector search results
- Uses LLM to rewrite queries with more semantic information

### VectorSearchTask
- Generates embeddings for refined queries
- Performs similarity search against movie database
- Returns top 25 most similar movies

### AnswerGenerationTask
- Uses retrieved context to generate recommendations
- Maintains chat history for iterative improvements
- Handles retry attempts with conversation context

### ValidationTask
- Evaluates recommendation quality using LLM
- Provides detailed feedback for improvements
- Triggers retries if validation fails

### DeliveryTask
- Returns final validated recommendation
- Logs completion statistics

## Configuration

The service supports the following configuration through environment variables:

- **MAX_RETRIES**: Maximum validation retry attempts (default: 3)
- **CHAT_HISTORY_LIMIT**: Maximum chat messages to retain (default: 50)
- **VECTOR_SEARCH_LIMIT**: Number of movies to retrieve (default: 25)

## Error Handling

The service includes comprehensive error handling:
- Database connection failures
- LLM API errors
- Validation failures
- Session management errors
- Workflow execution errors

All errors are logged and returned as structured JSON responses.

## Dependencies

Key dependencies include:
- `axum`: Web framework
- `graph-flow`: Workflow engine (local dependency)
- `sqlx`: PostgreSQL driver
- `rig-core`: LLM integration
- `fastembed`: Embedding generation
- `serde`: JSON serialization
- `tracing`: Structured logging
- `uuid`: Session ID generation

## License

This project is part of the rs-inter-task workspace. 