# PostgreSQL Session Storage for Graph Flow

This document describes the PostgreSQL-based session storage implementation for the graph-flow library.

## Overview

The `PostgresSessionStorage` provides persistent storage for session data using PostgreSQL as the backend database. It implements the `SessionStorage` trait and automatically creates the required database schema on first connection.

## Database Schema

The implementation creates a `sessions` table with the following structure:

```sql
CREATE TABLE IF NOT EXISTS sessions (
    id UUID PRIMARY KEY,
    graph_id TEXT NOT NULL,
    current_task_id TEXT NOT NULL,
    status_message TEXT,
    context JSONB NOT NULL,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);
```

### Schema Details

- `id`: UUID primary key identifying the session
- `graph_id`: The ID of the graph associated with this session
- `current_task_id`: The ID of the currently executing task
- `status_message`: Optional status message from the last executed task
- `context`: JSONB field storing the entire Context struct (includes both data and chat_history)
- `created_at`: Timestamp when the session was first created
- `updated_at`: Timestamp when the session was last modified

## Usage

### Setup

1. Add the required dependencies to your `Cargo.toml`:
```toml
sqlx = { version = "0.7", features = ["runtime-tokio-rustls", "postgres", "json", "macros", "uuid"] }
```

2. Set up your PostgreSQL database and connection string.

### Example Usage

```rust
use graph_flow::PostgresSessionStorage;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Connect to PostgreSQL - this will automatically create the schema
    let storage = PostgresSessionStorage::connect("postgresql://user:password@localhost/database").await?;
    
    // Use the storage with your session management
    let session = Session {
        id: "session-123".to_string(),
        graph_id: "my-graph".to_string(),
        current_task_id: "task-1".to_string(),
        status_message: Some("Processing...".to_string()),
        context: Context::new(),
    };
    
    // Save the session
    storage.save(session).await?;
    
    // Retrieve the session
    if let Some(retrieved) = storage.get("session-123").await? {
        println!("Found session: {}", retrieved.id);
    }
    
    // Delete the session
    storage.delete("session-123").await?;
    
    Ok(())
}
```

### Environment Variable Configuration

You can use environment variables to configure the database connection:

```bash
export DATABASE_URL="postgresql://user:password@localhost/database"
```

```rust
let database_url = std::env::var("DATABASE_URL")?;
let storage = PostgresSessionStorage::connect(&database_url).await?;
```

## Features

- **Automatic Schema Migration**: Creates the required table structure on first connection
- **UPSERT Operations**: Uses `ON CONFLICT` to handle both inserts and updates seamlessly
- **JSONB Storage**: Stores the entire Context (including chat history) as JSONB for efficient querying
- **Async/Await Support**: Fully async implementation using sqlx
- **Connection Pooling**: Uses sqlx connection pooling for efficient database connections
- **Error Handling**: Comprehensive error handling with detailed error messages

## Session Data Structure

The Context field stores the complete session context as JSONB, including:

- **data**: Key-value pairs for arbitrary session data
- **chat_history**: Complete chat conversation history with:
  - messages: Array of chat messages with role, content, and timestamp
  - max_messages: Optional limit for message history

Example of stored context JSON:
```json
{
  "data": {
    "claim_decision": {
      "approved": true,
      "decision_reason": "Auto-approved: claim amount under $1000 threshold",
      "timestamp": "2025-06-10T17:32:27.573057+00:00"
    },
    "insurance_type": "car",
    "user_input": "about $150"
  },
  "chat_history": {
    "messages": [
      {
        "role": "User",
        "content": "hello",
        "timestamp": "2025-06-10T17:30:48.727954Z"
      },
      {
        "role": "Assistant", 
        "content": "Hello! Welcome! I'm here to help you with your insurance claim.",
        "timestamp": "2025-06-10T17:30:48.727959Z"
      }
    ],
    "max_messages": null
  }
}
```

## Performance Considerations

- The implementation uses JSONB for flexible context storage while maintaining query performance
- Connection pooling is configured with a maximum of 5 connections by default
- Upsert operations minimize database round trips
- Automatic timestamp updates track session modifications

## Error Handling

The implementation provides detailed error messages for common failure scenarios:
- Database connection failures
- Schema migration failures  
- Context serialization/deserialization errors
- Session save/retrieve/delete failures

All errors are wrapped in `GraphError::StorageError` for consistent error handling across the library.