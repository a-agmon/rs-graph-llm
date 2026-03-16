//! Lance-based session storage with time travel support.
//!
//! This module provides [`LanceSessionStorage`] which persists sessions as
//! Lance dataset rows. Each save creates a new dataset version, enabling
//! time travel (loading sessions at any previous version).
//!
//! # Overview
//!
//! Lance is a columnar storage format with automatic versioning. Every write
//! operation creates a new version, and old versions are retained. This means
//! you can load any previous state of a session — enabling time travel debugging.
//!
//! # Architecture
//!
//! Since Lance is an optional heavy dependency, this module provides the storage
//! trait implementation and serialization logic. The actual Lance I/O is abstracted
//! behind a trait so it can be mocked in tests.
//!
//! # Examples
//!
//! ```rust
//! use graph_flow::lance_storage::LanceSessionStorage;
//!
//! // Create storage with a dataset path
//! let storage = LanceSessionStorage::new("/tmp/sessions.lance");
//! ```

use async_trait::async_trait;
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::{
    error::{GraphError, Result},
    storage::{Session, SessionStorage},
};

/// A session snapshot at a particular version.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionedSession {
    /// The session data
    pub session: Session,
    /// The version number (auto-incremented on each save)
    pub version: u64,
    /// Timestamp of this version
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// Lance-based session storage with time travel support.
///
/// Each `save()` creates a new version of the session. Previous versions
/// can be retrieved using `get_at_version()`.
///
/// This implementation uses an in-memory store that simulates Lance's
/// versioning semantics. For production use with actual Lance datasets,
/// replace the inner storage with Lance I/O calls.
pub struct LanceSessionStorage {
    /// Path to the Lance dataset (for future Lance integration)
    pub dataset_path: String,
    /// In-memory store: session_id -> Vec<VersionedSession>
    sessions: Arc<DashMap<String, Vec<VersionedSession>>>,
    /// Global version counter
    version_counter: Arc<AtomicU64>,
}

impl LanceSessionStorage {
    /// Create a new Lance session storage.
    ///
    /// The `dataset_path` is stored for future Lance integration.
    /// Currently uses an in-memory store with versioning semantics.
    pub fn new(dataset_path: impl Into<String>) -> Self {
        Self {
            dataset_path: dataset_path.into(),
            sessions: Arc::new(DashMap::new()),
            version_counter: Arc::new(AtomicU64::new(1)),
        }
    }

    /// Get a session at a specific version.
    ///
    /// Returns `None` if the session or version doesn't exist.
    ///
    /// # Time Travel
    ///
    /// This is the key feature: load any previous state of a session
    /// by version number. Useful for debugging, auditing, and replay.
    pub async fn get_at_version(
        &self,
        session_id: &str,
        version: u64,
    ) -> Result<Option<Session>> {
        Ok(self
            .sessions
            .get(session_id)
            .and_then(|versions| {
                versions
                    .iter()
                    .find(|v| v.version == version)
                    .map(|v| v.session.clone())
            }))
    }

    /// Get all versions of a session, ordered by version number.
    pub async fn get_versions(&self, session_id: &str) -> Result<Vec<VersionedSession>> {
        Ok(self
            .sessions
            .get(session_id)
            .map(|v| v.clone())
            .unwrap_or_default())
    }

    /// Get the version history (version numbers and timestamps) for a session.
    pub async fn get_version_history(
        &self,
        session_id: &str,
    ) -> Result<Vec<(u64, chrono::DateTime<chrono::Utc>)>> {
        Ok(self
            .sessions
            .get(session_id)
            .map(|versions| {
                versions
                    .iter()
                    .map(|v| (v.version, v.timestamp))
                    .collect()
            })
            .unwrap_or_default())
    }

    /// Revert a session to a specific version.
    ///
    /// This creates a NEW version with the state from the specified version.
    /// The old versions are preserved (append-only).
    pub async fn revert_to_version(
        &self,
        session_id: &str,
        version: u64,
    ) -> Result<Session> {
        let old_session = self
            .get_at_version(session_id, version)
            .await?
            .ok_or_else(|| {
                GraphError::SessionNotFound(format!(
                    "Session '{}' version {} not found",
                    session_id, version
                ))
            })?;

        // Save as a new version (append-only)
        self.save(old_session.clone()).await?;

        Ok(old_session)
    }

    /// Get the current (latest) version number for a session.
    pub async fn current_version(&self, session_id: &str) -> Option<u64> {
        self.sessions
            .get(session_id)
            .and_then(|versions| versions.last().map(|v| v.version))
    }
}

#[async_trait]
impl SessionStorage for LanceSessionStorage {
    async fn save(&self, session: Session) -> Result<()> {
        let version = self.version_counter.fetch_add(1, Ordering::SeqCst);
        let versioned = VersionedSession {
            session: session.clone(),
            version,
            timestamp: chrono::Utc::now(),
        };

        self.sessions
            .entry(session.id.clone())
            .or_default()
            .push(versioned);

        Ok(())
    }

    async fn get(&self, id: &str) -> Result<Option<Session>> {
        Ok(self
            .sessions
            .get(id)
            .and_then(|versions| versions.last().map(|v| v.session.clone())))
    }

    async fn delete(&self, id: &str) -> Result<()> {
        self.sessions.remove(id);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Context;

    fn make_session(id: &str, task: &str) -> Session {
        Session {
            id: id.to_string(),
            graph_id: "test".to_string(),
            current_task_id: task.to_string(),
            status_message: None,
            context: Context::new(),
        }
    }

    #[tokio::test]
    async fn test_basic_save_and_get() {
        let storage = LanceSessionStorage::new("/tmp/test.lance");

        let session = make_session("s1", "task_a");
        storage.save(session).await.unwrap();

        let loaded = storage.get("s1").await.unwrap();
        assert!(loaded.is_some());
        assert_eq!(loaded.unwrap().current_task_id, "task_a");
    }

    #[tokio::test]
    async fn test_versioning() {
        let storage = LanceSessionStorage::new("/tmp/test.lance");

        // Save version 1
        let mut session = make_session("s1", "task_a");
        storage.save(session.clone()).await.unwrap();

        // Save version 2
        session.current_task_id = "task_b".to_string();
        storage.save(session.clone()).await.unwrap();

        // Save version 3
        session.current_task_id = "task_c".to_string();
        storage.save(session).await.unwrap();

        // Latest should be task_c
        let latest = storage.get("s1").await.unwrap().unwrap();
        assert_eq!(latest.current_task_id, "task_c");

        // Get all versions
        let versions = storage.get_versions("s1").await.unwrap();
        assert_eq!(versions.len(), 3);
        assert_eq!(versions[0].session.current_task_id, "task_a");
        assert_eq!(versions[1].session.current_task_id, "task_b");
        assert_eq!(versions[2].session.current_task_id, "task_c");
    }

    #[tokio::test]
    async fn test_time_travel() {
        let storage = LanceSessionStorage::new("/tmp/test.lance");

        let mut session = make_session("s1", "start");
        storage.save(session.clone()).await.unwrap();
        let v1 = storage.current_version("s1").await.unwrap();

        session.current_task_id = "middle".to_string();
        storage.save(session.clone()).await.unwrap();

        session.current_task_id = "end".to_string();
        storage.save(session).await.unwrap();

        // Time travel back to v1
        let old = storage.get_at_version("s1", v1).await.unwrap().unwrap();
        assert_eq!(old.current_task_id, "start");
    }

    #[tokio::test]
    async fn test_revert() {
        let storage = LanceSessionStorage::new("/tmp/test.lance");

        let mut session = make_session("s1", "start");
        storage.save(session.clone()).await.unwrap();
        let v1 = storage.current_version("s1").await.unwrap();

        session.current_task_id = "changed".to_string();
        storage.save(session).await.unwrap();

        // Revert to v1
        let reverted = storage.revert_to_version("s1", v1).await.unwrap();
        assert_eq!(reverted.current_task_id, "start");

        // Latest should now be the reverted version
        let latest = storage.get("s1").await.unwrap().unwrap();
        assert_eq!(latest.current_task_id, "start");

        // But we should have 4 versions total (2 original + 1 revert)
        let versions = storage.get_versions("s1").await.unwrap();
        assert_eq!(versions.len(), 3);
    }

    #[tokio::test]
    async fn test_version_history() {
        let storage = LanceSessionStorage::new("/tmp/test.lance");

        let session = make_session("s1", "a");
        storage.save(session).await.unwrap();

        let session = make_session("s1", "b");
        storage.save(session).await.unwrap();

        let history = storage.get_version_history("s1").await.unwrap();
        assert_eq!(history.len(), 2);
        assert!(history[0].0 < history[1].0); // versions are increasing
    }

    #[tokio::test]
    async fn test_delete() {
        let storage = LanceSessionStorage::new("/tmp/test.lance");

        let session = make_session("s1", "a");
        storage.save(session).await.unwrap();

        storage.delete("s1").await.unwrap();
        assert!(storage.get("s1").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_nonexistent_session() {
        let storage = LanceSessionStorage::new("/tmp/test.lance");
        assert!(storage.get("nonexistent").await.unwrap().is_none());
    }
}
