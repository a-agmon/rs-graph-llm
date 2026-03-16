//! Typed state support for graph-flow.
//!
//! LangGraph uses TypedDict for state. graph-flow uses `Context` (HashMap<String, Value>).
//! This module adds an optional typed state layer via generics, while remaining
//! backward compatible with the existing `Context` API.
//!
//! # Examples
//!
//! ```rust
//! use graph_flow::typed_context::{TypedContext, State};
//! use serde::{Serialize, Deserialize};
//!
//! #[derive(Debug, Clone, Default, Serialize, Deserialize)]
//! struct AgentState {
//!     query: String,
//!     results: Vec<String>,
//!     iteration: usize,
//! }
//!
//! impl State for AgentState {}
//!
//! # #[tokio::main]
//! # async fn main() {
//! let ctx = TypedContext::new(AgentState {
//!     query: "rust langgraph".to_string(),
//!     results: vec![],
//!     iteration: 0,
//! });
//!
//! // Access typed state
//! {
//!     let state = ctx.state();
//!     assert_eq!(state.query, "rust langgraph");
//! }
//!
//! // Mutate typed state
//! ctx.update_state(|s| {
//!     s.iteration += 1;
//!     s.results.push("result1".to_string());
//! });
//!
//! {
//!     let state = ctx.state();
//!     assert_eq!(state.iteration, 1);
//!     assert_eq!(state.results.len(), 1);
//! }
//!
//! // Still access the underlying Context for untyped data
//! ctx.context().set_sync("extra_key", "extra_value".to_string());
//! let val: Option<String> = ctx.context().get_sync("extra_key");
//! assert_eq!(val, Some("extra_value".to_string()));
//! # }
//! ```

use serde::{de::DeserializeOwned, Serialize};
use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};

use crate::context::Context;

/// Marker trait for typed state structs.
///
/// Any struct that is Send + Sync + Serialize + DeserializeOwned + Clone can be used
/// as typed state in a `TypedContext`.
pub trait State: Send + Sync + Serialize + DeserializeOwned + Clone + 'static {}

/// A context wrapper that provides both typed state and untyped key-value storage.
///
/// `TypedContext<S>` wraps the standard `Context` and adds a typed state `S`.
/// The typed state provides compile-time guarantees about the shape of your
/// workflow state, while the underlying `Context` is still available for
/// dynamic data.
///
/// # Examples
///
/// ```rust
/// use graph_flow::typed_context::{TypedContext, State};
/// use serde::{Serialize, Deserialize};
///
/// #[derive(Debug, Clone, Default, Serialize, Deserialize)]
/// struct MyState {
///     counter: i32,
///     message: String,
/// }
///
/// impl State for MyState {}
///
/// let ctx = TypedContext::new(MyState {
///     counter: 0,
///     message: "hello".to_string(),
/// });
///
/// // Read state
/// assert_eq!(ctx.state().counter, 0);
///
/// // Update state
/// ctx.update_state(|s| s.counter += 1);
/// assert_eq!(ctx.state().counter, 1);
/// ```
#[derive(Clone)]
pub struct TypedContext<S: State> {
    inner: Context,
    state: Arc<RwLock<S>>,
}

impl<S: State> TypedContext<S> {
    /// Create a new TypedContext with initial state.
    pub fn new(initial_state: S) -> Self {
        Self {
            inner: Context::new(),
            state: Arc::new(RwLock::new(initial_state)),
        }
    }

    /// Create a new TypedContext with initial state and an existing Context.
    pub fn with_context(initial_state: S, context: Context) -> Self {
        Self {
            inner: context,
            state: Arc::new(RwLock::new(initial_state)),
        }
    }

    /// Get read access to the typed state.
    pub fn state(&self) -> RwLockReadGuard<'_, S> {
        self.state.read().expect("state lock poisoned")
    }

    /// Get write access to the typed state.
    pub fn state_mut(&self) -> RwLockWriteGuard<'_, S> {
        self.state.write().expect("state lock poisoned")
    }

    /// Update the typed state with a closure.
    pub fn update_state<F>(&self, f: F)
    where
        F: FnOnce(&mut S),
    {
        let mut state = self.state.write().expect("state lock poisoned");
        f(&mut state);
    }

    /// Get a reference to the underlying Context.
    pub fn context(&self) -> &Context {
        &self.inner
    }

    /// Get a clone of the typed state.
    pub fn snapshot_state(&self) -> S {
        self.state.read().expect("state lock poisoned").clone()
    }

    /// Replace the entire typed state.
    pub fn replace_state(&self, new_state: S) {
        let mut state = self.state.write().expect("state lock poisoned");
        *state = new_state;
    }

    /// Serialize the typed state to the Context under a given key.
    ///
    /// This is useful for persisting the typed state alongside the Context.
    pub async fn sync_state_to_context(&self, key: &str) {
        let state = self.state.read().expect("state lock poisoned").clone();
        self.inner.set(key, state).await;
    }

    /// Deserialize the typed state from the Context under a given key.
    ///
    /// Returns true if successful, false if the key doesn't exist or deserialization fails.
    pub async fn sync_state_from_context(&self, key: &str) -> bool {
        if let Some(state) = self.inner.get::<S>(key).await {
            let mut current = self.state.write().expect("state lock poisoned");
            *current = state;
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
    struct TestState {
        counter: i32,
        items: Vec<String>,
    }

    impl State for TestState {}

    #[test]
    fn test_typed_context_basic() {
        let ctx = TypedContext::new(TestState {
            counter: 0,
            items: vec![],
        });

        assert_eq!(ctx.state().counter, 0);
        assert!(ctx.state().items.is_empty());

        ctx.update_state(|s| {
            s.counter = 42;
            s.items.push("hello".to_string());
        });

        assert_eq!(ctx.state().counter, 42);
        assert_eq!(ctx.state().items, vec!["hello".to_string()]);
    }

    #[test]
    fn test_typed_context_with_context() {
        let context = Context::new();
        context.set_sync("key", "value".to_string());

        let ctx = TypedContext::with_context(
            TestState::default(),
            context,
        );

        let val: Option<String> = ctx.context().get_sync("key");
        assert_eq!(val, Some("value".to_string()));
    }

    #[test]
    fn test_snapshot_and_replace() {
        let ctx = TypedContext::new(TestState {
            counter: 10,
            items: vec!["a".to_string()],
        });

        let snap = ctx.snapshot_state();
        assert_eq!(snap.counter, 10);

        ctx.replace_state(TestState {
            counter: 99,
            items: vec![],
        });
        assert_eq!(ctx.state().counter, 99);

        // snapshot is independent
        assert_eq!(snap.counter, 10);
    }

    #[tokio::test]
    async fn test_sync_state_to_context() {
        let ctx = TypedContext::new(TestState {
            counter: 5,
            items: vec!["x".to_string()],
        });

        ctx.sync_state_to_context("typed_state").await;

        let loaded: Option<TestState> = ctx.context().get("typed_state").await;
        assert_eq!(
            loaded,
            Some(TestState {
                counter: 5,
                items: vec!["x".to_string()],
            })
        );
    }

    #[tokio::test]
    async fn test_sync_state_from_context() {
        let ctx = TypedContext::new(TestState::default());

        let target = TestState {
            counter: 77,
            items: vec!["loaded".to_string()],
        };
        ctx.context().set("state_key", target.clone()).await;

        assert!(ctx.sync_state_from_context("state_key").await);
        assert_eq!(ctx.state().counter, 77);
        assert_eq!(ctx.state().items, vec!["loaded".to_string()]);
    }

    #[tokio::test]
    async fn test_sync_state_from_context_missing() {
        let ctx = TypedContext::new(TestState::default());
        assert!(!ctx.sync_state_from_context("nonexistent").await);
        assert_eq!(ctx.state().counter, 0); // unchanged
    }

    #[test]
    fn test_clone() {
        let ctx = TypedContext::new(TestState {
            counter: 1,
            items: vec![],
        });

        let cloned = ctx.clone();
        ctx.update_state(|s| s.counter = 100);

        // Cloned shares the same Arc, so it sees the update
        assert_eq!(cloned.state().counter, 100);
    }
}
