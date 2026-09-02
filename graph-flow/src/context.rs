//! Context and state management for workflows.
//!
//! This module provides thread-safe state management across workflow tasks,
//! including regular data storage and specialized chat history management.
//!
//! All `Context` methods are synchronous: the underlying storage is an
//! in-memory map guarded by short critical sections, so there is nothing to
//! await. Methods can be called freely from both async tasks and sync code
//! (e.g. edge-condition closures).
//!
//! # Examples
//!
//! ## Basic Context Usage
//!
//! ```rust
//! use graph_flow::Context;
//!
//! # fn main() -> graph_flow::Result<()> {
//! let context = Context::new();
//!
//! // Store different types of data
//! context.set("user_id", 12345)?;
//! context.set("name", "Alice".to_string())?;
//! context.set("active", true)?;
//!
//! // Retrieve data with type safety
//! let user_id: Option<i32> = context.get("user_id");
//! let name: Option<String> = context.get("name");
//! let active: Option<bool> = context.get("active");
//! # Ok(())
//! # }
//! ```
//!
//! ## Chat History Management
//!
//! ```rust
//! use graph_flow::Context;
//!
//! let context = Context::new();
//!
//! // Add messages to chat history
//! context.add_user_message("Hello, assistant!".to_string());
//! context.add_assistant_message("Hello! How can I help you?".to_string());
//! context.add_system_message("User session started".to_string());
//!
//! // Get chat history
//! let history = context.get_chat_history();
//! let all_messages = context.get_all_messages();
//! let last_5 = context.get_last_messages(5);
//!
//! // Check history status
//! let count = context.chat_history_len();
//! let is_empty = context.is_chat_history_empty();
//! ```
//!
//! ## Context with Message Limits
//!
//! ```rust
//! use graph_flow::Context;
//!
//! // Create context with maximum 100 messages
//! let context = Context::with_max_chat_messages(100);
//!
//! // Messages will be automatically pruned when limit is exceeded
//! for i in 0..150 {
//!     context.add_user_message(format!("Message {}", i));
//! }
//!
//! // Only the last 100 messages are kept
//! assert_eq!(context.chat_history_len(), 100);
//! ```
//!
//! ## LLM Integration (with `rig` feature)
//!
//! ```rust
//! # #[cfg(feature = "rig")]
//! # {
//! use graph_flow::Context;
//!
//! let context = Context::new();
//!
//! context.add_user_message("What is the capital of France?".to_string());
//! context.add_assistant_message("The capital of France is Paris.".to_string());
//!
//! // Get messages in rig format for LLM calls
//! let mut rig_messages = context.get_rig_messages();
//! let recent_messages = context.get_last_rig_messages(10);
//!
//! // Use with rig-agent's completion API, whose `chat` borrows the history
//! // mutably and appends the turn's committed messages to it:
//! // let response = agent.chat(&user_input, &mut rig_messages).await?;
//! # }
//! ```

use chrono::{DateTime, Utc};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::VecDeque;
use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};

use crate::error::GraphError;

#[cfg(feature = "rig")]
use rig_core::completion::Message;

/// Represents the role of a message in a conversation.
///
/// Used in chat history to distinguish between different types of messages.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MessageRole {
    /// Message from a user/human
    User,
    /// Message from an assistant/AI
    Assistant,
    /// System message (instructions, status updates, etc.)
    System,
}

/// A serializable message that can be converted to/from rig_core::completion::Message.
///
/// This struct provides a unified message format that can be stored, serialized,
/// and optionally converted to other formats like rig's Message type.
///
/// # Examples
///
/// ```rust
/// use graph_flow::{SerializableMessage, MessageRole};
///
/// // Create different types of messages
/// let user_msg = SerializableMessage::user("Hello!".to_string());
/// let assistant_msg = SerializableMessage::assistant("Hi there!".to_string());
/// let system_msg = SerializableMessage::system("Session started".to_string());
///
/// // Access message properties
/// assert_eq!(user_msg.role, MessageRole::User);
/// assert_eq!(user_msg.content, "Hello!");
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializableMessage {
    /// The role of the message sender
    pub role: MessageRole,
    /// The content of the message
    pub content: String,
    /// When the message was created
    pub timestamp: DateTime<Utc>,
}

impl SerializableMessage {
    /// Create a new message with the specified role and content.
    ///
    /// The timestamp is automatically set to the current UTC time.
    pub fn new(role: MessageRole, content: String) -> Self {
        Self {
            role,
            content,
            timestamp: Utc::now(),
        }
    }

    /// Create a new user message.
    pub fn user(content: String) -> Self {
        Self::new(MessageRole::User, content)
    }

    /// Create a new assistant message.
    pub fn assistant(content: String) -> Self {
        Self::new(MessageRole::Assistant, content)
    }

    /// Create a new system message.
    pub fn system(content: String) -> Self {
        Self::new(MessageRole::System, content)
    }
}

/// Container for managing chat history with serialization support.
///
/// Messages are stored in a ring buffer: when the configured limit is
/// exceeded, the oldest message is dropped in O(1).
///
/// # Examples
///
/// ```rust
/// use graph_flow::ChatHistory;
///
/// let mut history = ChatHistory::new();
/// history.add_user_message("Hello".to_string());
/// history.add_assistant_message("Hi there!".to_string());
///
/// assert_eq!(history.len(), 2);
/// assert!(!history.is_empty());
///
/// let contents: Vec<&str> = history.messages().map(|m| m.content.as_str()).collect();
/// assert_eq!(contents, ["Hello", "Hi there!"]);
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ChatHistory {
    messages: VecDeque<SerializableMessage>,
    max_messages: Option<usize>,
}

impl ChatHistory {
    /// Create a new empty chat history with a default limit of 1000 messages.
    pub fn new() -> Self {
        Self {
            messages: VecDeque::new(),
            max_messages: Some(1000), // Default limit to prevent unbounded growth
        }
    }

    /// Create a new chat history with a maximum message limit.
    ///
    /// When the limit is exceeded, older messages are automatically removed.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use graph_flow::ChatHistory;
    ///
    /// let mut history = ChatHistory::with_max_messages(10);
    ///
    /// for i in 0..15 {
    ///     history.add_user_message(format!("Message {}", i));
    /// }
    ///
    /// // Only the last 10 are kept
    /// assert_eq!(history.len(), 10);
    /// ```
    pub fn with_max_messages(max: usize) -> Self {
        Self {
            messages: VecDeque::new(),
            max_messages: Some(max),
        }
    }

    /// Add a user message to the chat history.
    pub fn add_user_message(&mut self, content: String) {
        self.add_message(SerializableMessage::user(content));
    }

    /// Add an assistant message to the chat history.
    pub fn add_assistant_message(&mut self, content: String) {
        self.add_message(SerializableMessage::assistant(content));
    }

    /// Add a system message to the chat history.
    pub fn add_system_message(&mut self, content: String) {
        self.add_message(SerializableMessage::system(content));
    }

    /// Add a message to the chat history, respecting max_messages limit.
    fn add_message(&mut self, message: SerializableMessage) {
        self.messages.push_back(message);

        if let Some(max) = self.max_messages {
            while self.messages.len() > max {
                self.messages.pop_front();
            }
        }
    }

    /// Clear all messages from the chat history.
    pub fn clear(&mut self) {
        self.messages.clear();
    }

    /// Get the number of messages in the chat history.
    pub fn len(&self) -> usize {
        self.messages.len()
    }

    /// Check if the chat history is empty.
    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }

    /// Iterate over all messages, oldest first.
    pub fn messages(&self) -> impl Iterator<Item = &SerializableMessage> + '_ {
        self.messages.iter()
    }

    /// Iterate over the last N messages, oldest first.
    ///
    /// If N is greater than the total number of messages, all messages are returned.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use graph_flow::ChatHistory;
    ///
    /// let mut history = ChatHistory::new();
    /// history.add_user_message("Message 1".to_string());
    /// history.add_user_message("Message 2".to_string());
    /// history.add_user_message("Message 3".to_string());
    ///
    /// let last_two: Vec<&str> = history.last_messages(2).map(|m| m.content.as_str()).collect();
    /// assert_eq!(last_two, ["Message 2", "Message 3"]);
    /// ```
    pub fn last_messages(&self, n: usize) -> impl Iterator<Item = &SerializableMessage> + '_ {
        self.messages
            .iter()
            .skip(self.messages.len().saturating_sub(n))
    }
}

/// Helper struct for serializing/deserializing Context
#[derive(Serialize, Deserialize)]
struct ContextData {
    data: std::collections::HashMap<String, Value>,
    chat_history: ChatHistory,
}

/// Context for sharing data between tasks in a graph execution.
///
/// Provides thread-safe storage for workflow state and dedicated chat history
/// management. The context is shared across all tasks in a workflow execution
/// (cloning a `Context` is cheap and yields a handle to the same state).
///
/// All methods are synchronous - see the [module docs](self) for rationale.
///
/// # Examples
///
/// ```rust
/// use graph_flow::Context;
///
/// # fn main() -> graph_flow::Result<()> {
/// let context = Context::new();
///
/// // Typed key-value state
/// context.set("user_id", 12345)?;
/// let user_id: Option<i32> = context.get("user_id");
///
/// // Chat history
/// context.add_user_message("Hello".to_string());
/// let history = context.get_chat_history();
/// # Ok(())
/// # }
/// ```
#[derive(Clone, Debug)]
pub struct Context {
    data: Arc<DashMap<String, Value>>,
    chat_history: Arc<RwLock<ChatHistory>>,
}

impl Context {
    /// Create a new empty context.
    pub fn new() -> Self {
        Self {
            data: Arc::new(DashMap::new()),
            chat_history: Arc::new(RwLock::new(ChatHistory::new())),
        }
    }

    /// Create a new context with a maximum chat history size.
    ///
    /// When the chat history exceeds this size, older messages are automatically removed.
    pub fn with_max_chat_messages(max: usize) -> Self {
        Self {
            data: Arc::new(DashMap::new()),
            chat_history: Arc::new(RwLock::new(ChatHistory::with_max_messages(max))),
        }
    }

    /// Acquire the chat history read lock, recovering from poisoning.
    ///
    /// A poisoned lock means a task panicked while holding it; the history
    /// itself is still valid (messages are appended atomically), so we recover
    /// the guard rather than silently returning empty data.
    fn history_read(&self) -> RwLockReadGuard<'_, ChatHistory> {
        self.chat_history
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Acquire the chat history write lock, recovering from poisoning (see
    /// [`Context::history_read`]).
    fn history_write(&self) -> RwLockWriteGuard<'_, ChatHistory> {
        self.chat_history
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    // Regular context methods

    /// Set a value in the context.
    ///
    /// Returns `Err(GraphError::ContextError)` if the value cannot be
    /// serialized to JSON (e.g. a map with non-string keys).
    ///
    /// # Examples
    ///
    /// ```rust
    /// use graph_flow::Context;
    /// use serde::{Serialize, Deserialize};
    ///
    /// #[derive(Serialize, Deserialize)]
    /// struct UserData {
    ///     id: u32,
    ///     name: String,
    /// }
    ///
    /// # fn main() -> graph_flow::Result<()> {
    /// let context = Context::new();
    ///
    /// // Store primitive types
    /// context.set("count", 42)?;
    /// context.set("name", "Alice".to_string())?;
    ///
    /// // Store complex types
    /// let user = UserData { id: 1, name: "Bob".to_string() };
    /// context.set("user", user)?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn set(&self, key: impl Into<String>, value: impl serde::Serialize) -> crate::error::Result<()> {
        let key = key.into();
        let value = serde_json::to_value(value).map_err(|e| {
            GraphError::ContextError(format!("Failed to serialize value for key '{key}': {e}"))
        })?;
        self.data.insert(key, value);
        Ok(())
    }

    /// Get a value from the context.
    ///
    /// Returns `None` if the key doesn't exist or if the stored value cannot
    /// be deserialized to the requested type (a warning is logged for the
    /// latter, since it usually indicates a type mismatch bug).
    ///
    /// # Examples
    ///
    /// ```rust
    /// use graph_flow::Context;
    ///
    /// # fn main() -> graph_flow::Result<()> {
    /// let context = Context::new();
    /// context.set("count", 42)?;
    ///
    /// let count: Option<i32> = context.get("count");
    /// assert_eq!(count, Some(42));
    ///
    /// let missing: Option<String> = context.get("missing");
    /// assert_eq!(missing, None);
    /// # Ok(())
    /// # }
    /// ```
    pub fn get<T: serde::de::DeserializeOwned>(&self, key: &str) -> Option<T> {
        let entry = self.data.get(key)?;
        // Deserialize straight from a reference to avoid cloning the stored
        // Value (which can be large, e.g. documents or chat transcripts).
        match T::deserialize(entry.value()) {
            Ok(value) => Some(value),
            Err(e) => {
                tracing::warn!(
                    key = %key,
                    error = %e,
                    "Context value exists but failed to deserialize to the requested type"
                );
                None
            }
        }
    }

    /// Remove a value from the context.
    ///
    /// Returns the removed value if it existed.
    pub fn remove(&self, key: &str) -> Option<Value> {
        self.data.remove(key).map(|(_, v)| v)
    }

    /// Clear all regular context data (does not affect chat history).
    pub fn clear(&self) {
        self.data.clear();
    }

    // Chat history methods

    /// Add a user message to the chat history.
    pub fn add_user_message(&self, content: String) {
        self.history_write().add_user_message(content);
    }

    /// Add an assistant message to the chat history.
    pub fn add_assistant_message(&self, content: String) {
        self.history_write().add_assistant_message(content);
    }

    /// Add a system message to the chat history.
    pub fn add_system_message(&self, content: String) {
        self.history_write().add_system_message(content);
    }

    /// Get a clone of the current chat history.
    pub fn get_chat_history(&self) -> ChatHistory {
        self.history_read().clone()
    }

    /// Clear the chat history.
    pub fn clear_chat_history(&self) {
        self.history_write().clear();
    }

    /// Get the number of messages in the chat history.
    pub fn chat_history_len(&self) -> usize {
        self.history_read().len()
    }

    /// Check if the chat history is empty.
    pub fn is_chat_history_empty(&self) -> bool {
        self.history_read().is_empty()
    }

    /// Get the last N messages from chat history, oldest first.
    pub fn get_last_messages(&self, n: usize) -> Vec<SerializableMessage> {
        self.history_read().last_messages(n).cloned().collect()
    }

    /// Get all messages from chat history, oldest first.
    pub fn get_all_messages(&self) -> Vec<SerializableMessage> {
        self.history_read().messages().cloned().collect()
    }

    // Rig integration methods (only available when rig feature is enabled)

    #[cfg(feature = "rig")]
    /// Get all chat history messages converted to rig_core::completion::Message format.
    ///
    /// This method is only available when the "rig" feature is enabled.
    pub fn get_rig_messages(&self) -> Vec<Message> {
        self.get_all_messages().into_iter().map(Message::from).collect()
    }

    #[cfg(feature = "rig")]
    /// Get the last N messages converted to rig_core::completion::Message format.
    ///
    /// This method is only available when the "rig" feature is enabled.
    pub fn get_last_rig_messages(&self, n: usize) -> Vec<Message> {
        self.get_last_messages(n).into_iter().map(Message::from).collect()
    }
}

#[cfg(feature = "rig")]
impl From<SerializableMessage> for Message {
    /// Convert a [`SerializableMessage`] into a `rig_core::completion::Message`,
    /// moving the content instead of cloning it.
    ///
    /// Only available when the "rig" feature is enabled.
    fn from(msg: SerializableMessage) -> Self {
        match msg.role {
            MessageRole::User => Message::user(msg.content),
            MessageRole::Assistant => Message::assistant(msg.content),
            MessageRole::System => Message::system(msg.content),
        }
    }
}

impl Default for Context {
    fn default() -> Self {
        Self::new()
    }
}

// Serialization support for Context
impl Serialize for Context {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        // Convert DashMap to HashMap for serialization
        let data: std::collections::HashMap<String, Value> = self
            .data
            .iter()
            .map(|entry| (entry.key().clone(), entry.value().clone()))
            .collect();

        let chat_history = self.history_read().clone();

        let context_data = ContextData { data, chat_history };
        context_data.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Context {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let context_data = ContextData::deserialize(deserializer)?;

        let data = Arc::new(DashMap::new());
        for (key, value) in context_data.data {
            data.insert(key, value);
        }

        let chat_history = Arc::new(RwLock::new(context_data.chat_history));

        Ok(Context { data, chat_history })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_context_operations() {
        let context = Context::new();

        context.set("key", "value").unwrap();
        let value: Option<String> = context.get("key");
        assert_eq!(value, Some("value".to_string()));
    }

    #[test]
    fn test_set_unserializable_value_errors() {
        use std::collections::HashMap;
        let context = Context::new();

        // Maps with non-string keys cannot be represented in JSON
        let bad: HashMap<Vec<u8>, String> = HashMap::from([(vec![1u8], "x".to_string())]);
        assert!(context.set("bad", bad).is_err());
    }

    #[test]
    fn test_chat_history_operations() {
        let context = Context::new();

        assert!(context.is_chat_history_empty());
        assert_eq!(context.chat_history_len(), 0);

        context.add_user_message("Hello".to_string());
        context.add_assistant_message("Hi there!".to_string());

        assert!(!context.is_chat_history_empty());
        assert_eq!(context.chat_history_len(), 2);

        let history = context.get_chat_history();
        assert_eq!(history.len(), 2);
        let messages: Vec<_> = history.messages().collect();
        assert_eq!(messages[0].content, "Hello");
        assert_eq!(messages[0].role, MessageRole::User);
        assert_eq!(messages[1].content, "Hi there!");
        assert_eq!(messages[1].role, MessageRole::Assistant);
    }

    #[test]
    fn test_chat_history_max_messages() {
        let context = Context::with_max_chat_messages(2);

        context.add_user_message("Message 1".to_string());
        context.add_assistant_message("Response 1".to_string());
        context.add_user_message("Message 2".to_string());

        let history = context.get_chat_history();
        assert_eq!(history.len(), 2);
        let messages: Vec<_> = history.messages().collect();
        assert_eq!(messages[0].content, "Response 1");
        assert_eq!(messages[1].content, "Message 2");
    }

    #[test]
    fn test_last_messages() {
        let context = Context::new();

        context.add_user_message("Message 1".to_string());
        context.add_assistant_message("Response 1".to_string());
        context.add_user_message("Message 2".to_string());
        context.add_assistant_message("Response 2".to_string());

        let last_two = context.get_last_messages(2);
        assert_eq!(last_two.len(), 2);
        assert_eq!(last_two[0].content, "Message 2");
        assert_eq!(last_two[1].content, "Response 2");
    }

    #[test]
    fn test_context_serialization() {
        let context = Context::new();
        context.set("key", "value").unwrap();
        context.add_user_message("test message".to_string());

        let serialized = serde_json::to_string(&context).unwrap();
        let deserialized: Context = serde_json::from_str(&serialized).unwrap();

        let value: Option<String> = deserialized.get("key");
        assert_eq!(value, Some("value".to_string()));

        assert_eq!(deserialized.chat_history_len(), 1);
        let history = deserialized.get_chat_history();
        let first = history.messages().next().unwrap();
        assert_eq!(first.content, "test message");
        assert_eq!(first.role, MessageRole::User);
    }

    #[test]
    fn test_serializable_message() {
        let msg = SerializableMessage::user("test content".to_string());
        assert_eq!(msg.role, MessageRole::User);
        assert_eq!(msg.content, "test content");

        let serialized = serde_json::to_string(&msg).unwrap();
        let deserialized: SerializableMessage = serde_json::from_str(&serialized).unwrap();

        assert_eq!(msg.role, deserialized.role);
        assert_eq!(msg.content, deserialized.content);
    }

    #[test]
    fn test_chat_history_serialization() {
        let mut history = ChatHistory::new();
        history.add_user_message("Hello".to_string());
        history.add_assistant_message("Hi!".to_string());

        let serialized = serde_json::to_string(&history).unwrap();
        let deserialized: ChatHistory = serde_json::from_str(&serialized).unwrap();

        assert_eq!(deserialized.len(), 2);
        let messages: Vec<_> = deserialized.messages().collect();
        assert_eq!(messages[0].content, "Hello");
        assert_eq!(messages[1].content, "Hi!");
    }

    #[test]
    fn test_chat_history_json_compat_with_0_5() {
        // 0.5 serialized `messages` as a JSON array from a Vec; the VecDeque
        // representation must remain interchangeable.
        let json = r#"{"messages":[{"role":"User","content":"hi","timestamp":"2026-01-01T00:00:00Z"}],"max_messages":1000}"#;
        let history: ChatHistory = serde_json::from_str(json).unwrap();
        assert_eq!(history.len(), 1);
    }

    #[cfg(feature = "rig")]
    #[test]
    fn test_rig_integration() {
        let context = Context::new();

        context.add_user_message("Hello".to_string());
        context.add_assistant_message("Hi there!".to_string());
        context.add_system_message("System message".to_string());

        let rig_messages = context.get_rig_messages();
        assert_eq!(rig_messages.len(), 3);

        // Verify each message maps to the correct rig variant.
        assert!(
            matches!(&rig_messages[0], Message::User { .. }),
            "Expected Message::User, got: {:?}",
            rig_messages[0]
        );
        assert!(
            matches!(&rig_messages[1], Message::Assistant { .. }),
            "Expected Message::Assistant, got: {:?}",
            rig_messages[1]
        );
        assert!(
            matches!(&rig_messages[2], Message::System { content } if content == "System message"),
            "Expected Message::System with correct content, got: {:?}",
            rig_messages[2]
        );

        // Verify get_last_rig_messages tail ordering when the tail includes a system message.
        let last_two = context.get_last_rig_messages(2);
        assert_eq!(last_two.len(), 2);
        assert!(
            matches!(&last_two[0], Message::Assistant { .. }),
            "Expected Message::Assistant as second-to-last, got: {:?}",
            last_two[0]
        );
        assert!(
            matches!(&last_two[1], Message::System { .. }),
            "Expected Message::System as last, got: {:?}",
            last_two[1]
        );
    }
}
