use async_trait::async_trait;
use serde_json;
use sqlx::{postgres::PgPoolOptions, Pool, Postgres};
use std::sync::Arc;

use crate::{Session, error::{Result, GraphError}, storage::SessionStorage};

pub struct PostgresSessionStorage {
    pool: Arc<Pool<Postgres>>,
}

impl PostgresSessionStorage {
    pub async fn connect(database_url: &str) -> Result<Self> {
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect(database_url)
            .await
            .map_err(|e| GraphError::StorageError(format!("Failed to connect to Postgres: {e}")))?;

        Self::migrate(&pool).await?;
        Ok(Self { pool: Arc::new(pool) })
    }

    async fn migrate(pool: &Pool<Postgres>) -> Result<()> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS sessions (
                id UUID PRIMARY KEY,
                graph_id TEXT NOT NULL,
                current_task_id TEXT NOT NULL,
                status_message TEXT,
                context JSONB NOT NULL,
                created_at TIMESTAMPTZ DEFAULT NOW(),
                updated_at TIMESTAMPTZ DEFAULT NOW()
            );
            "#,
        )
        .execute(pool)
        .await
        .map_err(|e| GraphError::StorageError(format!("Migration failed: {e}")))?;
        Ok(())
    }
}

#[async_trait]
impl SessionStorage for PostgresSessionStorage {
    async fn save(&self, session: Session) -> Result<()> {
        let context_json = serde_json::to_value(&session.context)
            .map_err(|e| GraphError::StorageError(format!("Context serialization failed: {e}")))?;

        // Use a transaction to ensure atomicity
        let mut tx = self.pool.begin().await
            .map_err(|e| GraphError::StorageError(format!("Failed to start transaction: {e}")))?;

        sqlx::query(
            r#"
            INSERT INTO sessions (id, graph_id, current_task_id, status_message, context, updated_at)
            VALUES ($1::uuid, $2, $3, $4, $5, NOW())
            ON CONFLICT (id) DO UPDATE
            SET graph_id = EXCLUDED.graph_id,
                current_task_id = EXCLUDED.current_task_id,
                status_message = EXCLUDED.status_message,
                context = EXCLUDED.context,
                updated_at = NOW()
            WHERE sessions.updated_at <= EXCLUDED.updated_at  -- Prevent overwriting newer data
            "#,
        )
        .bind(&session.id)
        .bind(&session.graph_id)
        .bind(&session.current_task_id)
        .bind(&session.status_message)
        .bind(&context_json)
        .execute(&mut *tx)
        .await
        .map_err(|e| GraphError::StorageError(format!("Failed to save session: {e}")))?;
        
        tx.commit().await
            .map_err(|e| GraphError::StorageError(format!("Failed to commit transaction: {e}")))?;
        
        Ok(())
    }

    async fn get(&self, id: &str) -> Result<Option<Session>> {
        let row = sqlx::query_as::<_, (String, String, String, Option<String>, serde_json::Value)>(
            r#"
            SELECT id::text, graph_id, current_task_id, status_message, context
            FROM sessions
            WHERE id = $1::uuid
            "#,
        )
        .bind(id)
        .fetch_optional(&*self.pool)
        .await
        .map_err(|e| GraphError::StorageError(format!("Failed to fetch session: {e}")))?;

        if let Some((session_id, graph_id, current_task_id, status_message, context_json)) = row {
            let context: crate::Context = serde_json::from_value(context_json)
                .map_err(|e| GraphError::StorageError(format!("Context deserialization failed: {e}")))?;
            Ok(Some(Session {
                id: session_id,
                graph_id,
                current_task_id,
                status_message,
                context,
            }))
        } else {
            Ok(None)
        }
    }

    async fn delete(&self, id: &str) -> Result<()> {
        sqlx::query(
            r#"
            DELETE FROM sessions WHERE id = $1::uuid
            "#,
        )
        .bind(id)
        .execute(&*self.pool)
        .await
        .map_err(|e| GraphError::StorageError(format!("Failed to delete session: {e}")))?;
        Ok(())
    }
}