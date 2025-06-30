//! Context and state management for workflows.
//!
//! This module provides thread-safe state management across workflow tasks,
//! including regular data storage and specialized chat history management.
//!
//! # Examples
//!
//! ## Basic Context Usage
//!
//! ```rust
//! use graph_flow::Context;
//!
//! # #[tokio::main]
//! # async fn main() {
//! let context = Context::new();
//!
//! // Store different types of data
//! context.set("user_id", 12345).await;
//! context.set("name", "Alice".to_string()).await;
//! context.set("active", true).await;
//!
//! // Retrieve data with type safety
//! let user_id: Option<i32> = context.get("user_id").await;
//! let name: Option<String> = context.get("name").await;
//! let active: Option<bool> = context.get("active").await;
//!
//! // Synchronous access (useful in edge conditions)
//! let name_sync: Option<String> = context.get_sync("name");
//! # }
//! ```
//!
//! ## Chat History Management
//!
//! ```rust
//! use graph_flow::Context;
//!
//! # #[tokio::main]
//! # async fn main() {
//! let context = Context::new();
//!
//! // Add messages to chat history
//! context.add_user_message("Hello, assistant!".to_string()).await;
//! context.add_assistant_message("Hello! How can I help you?".to_string()).await;
//! context.add_system_message("User session started".to_string()).await;
//!
//! // Get chat history
//! let history = context.get_chat_history().await;
//! let all_messages = context.get_all_messages().await;
//! let last_5 = context.get_last_messages(5).await;
//!
//! // Check history status
//! let count = context.chat_history_len().await;
//! let is_empty = context.is_chat_history_empty().await;
//! # }
//! ```
//!
//! ## Context with Message Limits
//!
//! ```rust
//! use graph_flow::Context;
//!
//! # #[tokio::main]
//! # async fn main() {
//! // Create context with maximum 100 messages
//! let context = Context::with_max_chat_messages(100);
//!
//! // Messages will be automatically pruned when limit is exceeded
//! for i in 0..150 {
//!     context.add_user_message(format!("Message {}", i)).await;
//! }
//!
//! // Only the last 100 messages are kept
//! assert_eq!(context.chat_history_len().await, 100);
//! # }
//! ```
//!
//! ## LLM Integration (with `rig` feature)
//!
//! ```rust
//! # #[cfg(feature = "rig")]
//! # {
//! use graph_flow::Context;
//!
//! # #[tokio::main]
//! # async fn main() {
//! let context = Context::new();
//!
//! context.add_user_message("What is the capital of France?".to_string()).await;
//! context.add_assistant_message("The capital of France is Paris.".to_string()).await;
//!
//! // Get messages in rig format for LLM calls
//! let rig_messages = context.get_rig_messages().await;
//! let recent_messages = context.get_last_rig_messages(10).await;
//!
//! // Use with rig's completion API
//! // let response = agent.completion(&rig_messages).await?;
//! # }
//! # }
//! ```

use chrono::{DateTime, Utc};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::{Arc, RwLock};

#[cfg(feature = "rig")]
use rig::completion::Message;

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

/// A serializable message that can be converted to/from rig::completion::Message.
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
    ///
    /// # Examples
    ///
    /// ```rust
    /// use graph_flow::SerializableMessage;
    ///
    /// let msg = SerializableMessage::user("Hello, world!".to_string());
    /// ```
    pub fn user(content: String) -> Self {
        Self::new(MessageRole::User, content)
    }

    /// Create a new assistant message.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use graph_flow::SerializableMessage;
    ///
    /// let msg = SerializableMessage::assistant("Hello! How can I help?".to_string());
    /// ```
    pub fn assistant(content: String) -> Self {
        Self::new(MessageRole::Assistant, content)
    }

    /// Create a new system message.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use graph_flow::SerializableMessage;
    ///
    /// let msg = SerializableMessage::system("User logged in".to_string());
    /// ```
    pub fn system(content: String) -> Self {
        Self::new(MessageRole::System, content)
    }
}

/// Container for managing chat history with serialization support.
///
/// Provides automatic message limit management and convenient methods
/// for adding and retrieving messages.
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
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ChatHistory {
    messages: Vec<SerializableMessage>,
    max_messages: Option<usize>,
}

impl ChatHistory {
    /// Create a new empty chat history with a default limit of 1000 messages.
    pub fn new() -> Self {
        Self {
            messages: Vec::new(),
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
    /// // Add 15 messages
    /// for i in 0..15 {
    ///     history.add_user_message(format!("Message {}", i));
    /// }
    ///
    /// // Only the last 10 are kept
    /// assert_eq!(history.len(), 10);
    /// ```
    pub fn with_max_messages(max: usize) -> Self {
        Self {
            messages: Vec::new(),
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
        self.messages.push(message);

        if let Some(max) = self.max_messages {
            if self.messages.len() > max {
                self.messages.drain(0..(self.messages.len() - max));
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

    /// Get a reference to all messages.
    pub fn messages(&self) -> &[SerializableMessage] {
        &self.messages
    }

    /// Get the last N messages.
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
    /// let last_two = history.last_messages(2);
    /// assert_eq!(last_two.len(), 2);
    /// assert_eq!(last_two[0].content, "Message 2");
    /// assert_eq!(last_two[1].content, "Message 3");
    /// ```
    pub fn last_messages(&self, n: usize) -> &[SerializableMessage] {
        let start = if self.messages.len() > n {
            self.messages.len() - n
        } else {
            0
        };
        &self.messages[start..]
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
/// management. The context is shared across all tasks in a workflow execution.
///
/// # Examples
///
/// ## Basic Usage
///
/// ```rust
/// use graph_flow::Context;
///
/// # #[tokio::main]
/// # async fn main() {
/// let context = Context::new();
///
/// // Store different types of data
/// context.set("user_id", 12345).await;
/// context.set("name", "Alice".to_string()).await;
/// context.set("settings", vec!["opt1", "opt2"]).await;
///
/// // Retrieve data
/// let user_id: Option<i32> = context.get("user_id").await;
/// let name: Option<String> = context.get("name").await;
/// let settings: Option<Vec<String>> = context.get("settings").await;
/// # }
/// ```
///
/// ## Chat History
///
/// ```rust
/// use graph_flow::Context;
///
/// # #[tokio::main]
/// # async fn main() {
/// let context = Context::new();
///
/// // Add messages
/// context.add_user_message("Hello".to_string()).await;
/// context.add_assistant_message("Hi there!".to_string()).await;
///
/// // Get message history
/// let history = context.get_chat_history().await;
/// let last_5 = context.get_last_messages(5).await;
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
    ///
    /// # Examples
    ///
    /// ```rust
    /// use graph_flow::Context;
    ///
    /// # #[tokio::main]
    /// # async fn main() {
    /// let context = Context::with_max_chat_messages(50);
    ///
    /// // Chat history will be limited to 50 messages
    /// for i in 0..100 {
    ///     context.add_user_message(format!("Message {}", i)).await;
    /// }
    ///
    /// assert_eq!(context.chat_history_len().await, 50);
    /// # }
    /// ```
    pub fn with_max_chat_messages(max: usize) -> Self {
        Self {
            data: Arc::new(DashMap::new()),
            chat_history: Arc::new(RwLock::new(ChatHistory::with_max_messages(max))),
        }
    }

    // Regular context methods (unchanged API)

    /// Set a value in the context.
    ///
    /// The value must be serializable. Most common Rust types are supported.
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
    /// # #[tokio::main]
    /// # async fn main() {
    /// let context = Context::new();
    ///
    /// // Store primitive types
    /// context.set("count", 42).await;
    /// context.set("name", "Alice".to_string()).await;
    /// context.set("active", true).await;
    ///
    /// // Store complex types
    /// let user = UserData { id: 1, name: "Bob".to_string() };
    /// context.set("user", user).await;
    /// # }
    /// ```
    pub async fn set(&self, key: impl Into<String>, value: impl serde::Serialize) {
        let value = serde_json::to_value(value).expect("Failed to serialize value");
        self.data.insert(key.into(), value);
    }

    /// Get a value from the context.
    ///
    /// Returns `None` if the key doesn't exist or if deserialization fails.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use graph_flow::Context;
    ///
    /// # #[tokio::main]
    /// # async fn main() {
    /// let context = Context::new();
    /// context.set("count", 42).await;
    ///
    /// let count: Option<i32> = context.get("count").await;
    /// assert_eq!(count, Some(42));
    ///
    /// let missing: Option<String> = context.get("missing").await;
    /// assert_eq!(missing, None);
    /// # }
    /// ```
    pub async fn get<T: serde::de::DeserializeOwned>(&self, key: &str) -> Option<T> {
        self.data
            .get(key)
            .and_then(|v| serde_json::from_value(v.clone()).ok())
    }

    /// Remove a value from the context.
    ///
    /// Returns the removed value if it existed.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use graph_flow::Context;
    ///
    /// # #[tokio::main]
    /// # async fn main() {
    /// let context = Context::new();
    /// context.set("temp", "value".to_string()).await;
    ///
    /// let removed = context.remove("temp").await;
    /// assert!(removed.is_some());
    ///
    /// let value: Option<String> = context.get("temp").await;
    /// assert_eq!(value, None);
    /// # }
    /// ```
    pub async fn remove(&self, key: &str) -> Option<Value> {
        self.data.remove(key).map(|(_, v)| v)
    }

    /// Clear all regular context data (does not affect chat history).
    ///
    /// # Examples
    ///
    /// ```rust
    /// use graph_flow::Context;
    ///
    /// # #[tokio::main]
    /// # async fn main() {
    /// let context = Context::new();
    /// context.set("key1", "value1".to_string()).await;
    /// context.set("key2", "value2".to_string()).await;
    /// context.add_user_message("Hello".to_string()).await;
    ///
    /// context.clear().await;
    ///
    /// // Regular data is cleared
    /// let value: Option<String> = context.get("key1").await;
    /// assert_eq!(value, None);
    ///
    /// // Chat history is preserved
    /// assert_eq!(context.chat_history_len().await, 1);
    /// # }
    /// ```
    pub async fn clear(&self) {
        self.data.clear();
    }

    /// Synchronous version of get for use in edge conditions.
    ///
    /// This method should only be used when you're certain the data exists
    /// and when async is not available (e.g., in edge condition closures).
    ///
    /// # Examples
    ///
    /// ```rust
    /// use graph_flow::{Context, GraphBuilder};
    ///
    /// # #[tokio::main]
    /// # async fn main() {
    /// let context = Context::new();
    /// context.set("condition", true).await;
    ///
    /// // Used in edge conditions
    /// let graph = GraphBuilder::new("test")
    ///     .add_conditional_edge(
    ///         "task1",
    ///         |ctx| ctx.get_sync::<bool>("condition").unwrap_or(false),
    ///         "task2",
    ///         "task3"
    ///     );
    /// # }
    /// ```
    pub fn get_sync<T: serde::de::DeserializeOwned>(&self, key: &str) -> Option<T> {
        self.data
            .get(key)
            .and_then(|v| serde_json::from_value(v.clone()).ok())
    }

    /// Synchronous version of set for use when async is not available.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use graph_flow::Context;
    ///
    /// let context = Context::new();
    /// context.set_sync("key", "value".to_string());
    ///
    /// let value: Option<String> = context.get_sync("key");
    /// assert_eq!(value, Some("value".to_string()));
    /// ```
    pub fn set_sync(&self, key: impl Into<String>, value: impl serde::Serialize) {
        let value = serde_json::to_value(value).expect("Failed to serialize value");
        self.data.insert(key.into(), value);
    }

    // Chat history methods

    /// Add a user message to the chat history.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use graph_flow::Context;
    ///
    /// # #[tokio::main]
    /// # async fn main() {
    /// let context = Context::new();
    /// context.add_user_message("Hello, assistant!".to_string()).await;
    /// # }
    /// ```
    pub async fn add_user_message(&self, content: String) {
        if let Ok(mut history) = self.chat_history.write() {
            history.add_user_message(content);
        }
    }

    /// Add an assistant message to the chat history.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use graph_flow::Context;
    ///
    /// # #[tokio::main]
    /// # async fn main() {
    /// let context = Context::new();
    /// context.add_assistant_message("Hello! How can I help you?".to_string()).await;
    /// # }
    /// ```
    pub async fn add_assistant_message(&self, content: String) {
        if let Ok(mut history) = self.chat_history.write() {
            history.add_assistant_message(content);
        }
    }

    /// Add a system message to the chat history.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use graph_flow::Context;
    ///
    /// # #[tokio::main]
    /// # async fn main() {
    /// let context = Context::new();
    /// context.add_system_message("Session started".to_string()).await;
    /// # }
    /// ```
    pub async fn add_system_message(&self, content: String) {
        if let Ok(mut history) = self.chat_history.write() {
            history.add_system_message(content);
        }
    }

    /// Get a clone of the current chat history.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use graph_flow::Context;
    ///
    /// # #[tokio::main]
    /// # async fn main() {
    /// let context = Context::new();
    /// context.add_user_message("Hello".to_string()).await;
    ///
    /// let history = context.get_chat_history().await;
    /// assert_eq!(history.len(), 1);
    /// # }
    /// ```
    pub async fn get_chat_history(&self) -> ChatHistory {
        if let Ok(history) = self.chat_history.read() {
            history.clone()
        } else {
            ChatHistory::new()
        }
    }

    /// Clear the chat history.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use graph_flow::Context;
    ///
    /// # #[tokio::main]
    /// # async fn main() {
    /// let context = Context::new();
    /// context.add_user_message("Hello".to_string()).await;
    /// assert_eq!(context.chat_history_len().await, 1);
    ///
    /// context.clear_chat_history().await;
    /// assert_eq!(context.chat_history_len().await, 0);
    /// # }
    /// ```
    pub async fn clear_chat_history(&self) {
        if let Ok(mut history) = self.chat_history.write() {
            history.clear();
        }
    }

    /// Get the number of messages in the chat history.
    pub async fn chat_history_len(&self) -> usize {
        if let Ok(history) = self.chat_history.read() {
            history.len()
        } else {
            0
        }
    }

    /// Check if the chat history is empty.
    pub async fn is_chat_history_empty(&self) -> bool {
        if let Ok(history) = self.chat_history.read() {
            history.is_empty()
        } else {
            true
        }
    }

    /// Get the last N messages from chat history.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use graph_flow::Context;
    ///
    /// # #[tokio::main]
    /// # async fn main() {
    /// let context = Context::new();
    /// context.add_user_message("Message 1".to_string()).await;
    /// context.add_user_message("Message 2".to_string()).await;
    /// context.add_user_message("Message 3".to_string()).await;
    ///
    /// let last_two = context.get_last_messages(2).await;
    /// assert_eq!(last_two.len(), 2);
    /// assert_eq!(last_two[0].content, "Message 2");
    /// assert_eq!(last_two[1].content, "Message 3");
    /// # }
    /// ```
    pub async fn get_last_messages(&self, n: usize) -> Vec<SerializableMessage> {
        if let Ok(history) = self.chat_history.read() {
            history.last_messages(n).to_vec()
        } else {
            Vec::new()
        }
    }

    /// Get all messages from chat history as SerializableMessage.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use graph_flow::Context;
    ///
    /// # #[tokio::main]
    /// # async fn main() {
    /// let context = Context::new();
    /// context.add_user_message("Hello".to_string()).await;
    /// context.add_assistant_message("Hi there!".to_string()).await;
    ///
    /// let all_messages = context.get_all_messages().await;
    /// assert_eq!(all_messages.len(), 2);
    /// # }
    /// ```
    pub async fn get_all_messages(&self) -> Vec<SerializableMessage> {
        if let Ok(history) = self.chat_history.read() {
            history.messages().to_vec()
        } else {
            Vec::new()
        }
    }

    // Rig integration methods (only available when rig feature is enabled)

    #[cfg(feature = "rig")]
    /// Get all chat history messages converted to rig::completion::Message format.
    ///
    /// This method is only available when the "rig" feature is enabled.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # #[cfg(feature = "rig")]
    /// # {
    /// use graph_flow::Context;
    ///
    /// # #[tokio::main]
    /// # async fn main() {
    /// let context = Context::new();
    /// context.add_user_message("Hello".to_string()).await;
    /// context.add_assistant_message("Hi there!".to_string()).await;
    ///
    /// let rig_messages = context.get_rig_messages().await;
    /// assert_eq!(rig_messages.len(), 2);
    /// # }
    /// # }
    /// ```
    pub async fn get_rig_messages(&self) -> Vec<Message> {
        let messages = self.get_all_messages().await;
        messages
            .iter()
            .map(|msg| self.to_rig_message(msg))
            .collect()
    }

    #[cfg(feature = "rig")]
    /// Get the last N messages converted to rig::completion::Message format.
    ///
    /// This method is only available when the "rig" feature is enabled.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # #[cfg(feature = "rig")]
    /// # {
    /// use graph_flow::Context;
    ///
    /// # #[tokio::main]
    /// # async fn main() {
    /// let context = Context::new();
    /// for i in 0..10 {
    ///     context.add_user_message(format!("Message {}", i)).await;
    /// }
    ///
    /// let last_5 = context.get_last_rig_messages(5).await;
    /// assert_eq!(last_5.len(), 5);
    /// # }
    /// # }
    /// ```
    pub async fn get_last_rig_messages(&self, n: usize) -> Vec<Message> {
        let messages = self.get_last_messages(n).await;
        messages
            .iter()
            .map(|msg| self.to_rig_message(msg))
            .collect()
    }

    #[cfg(feature = "rig")]
    /// Convert a SerializableMessage to a rig::completion::Message.
    ///
    /// This method is only available when the "rig" feature is enabled.
    fn to_rig_message(&self, msg: &SerializableMessage) -> Message {
        match msg.role {
            MessageRole::User => Message::user(msg.content.clone()),
            MessageRole::Assistant => Message::assistant(msg.content.clone()),
            // rig doesn't have a system message type, so we'll treat it as a user message
            // with a system prefix
            MessageRole::System => Message::user(format!("[SYSTEM] {}", msg.content)),
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

        let chat_history = if let Ok(history) = self.chat_history.read() {
            history.clone()
        } else {
            ChatHistory::new()
        };

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

    #[tokio::test]
    async fn test_basic_context_operations() {
        let context = Context::new();

        context.set("key", "value").await;
        let value: Option<String> = context.get("key").await;
        assert_eq!(value, Some("value".to_string()));
    }

    #[tokio::test]
    async fn test_chat_history_operations() {
        let context = Context::new();

        assert!(context.is_chat_history_empty().await);
        assert_eq!(context.chat_history_len().await, 0);

        context.add_user_message("Hello".to_string()).await;
        context.add_assistant_message("Hi there!".to_string()).await;

        assert!(!context.is_chat_history_empty().await);
        assert_eq!(context.chat_history_len().await, 2);

        let history = context.get_chat_history().await;
        assert_eq!(history.len(), 2);
        assert_eq!(history.messages()[0].content, "Hello");
        assert_eq!(history.messages()[0].role, MessageRole::User);
        assert_eq!(history.messages()[1].content, "Hi there!");
        assert_eq!(history.messages()[1].role, MessageRole::Assistant);
    }

    #[tokio::test]
    async fn test_chat_history_max_messages() {
        let context = Context::with_max_chat_messages(2);

        context.add_user_message("Message 1".to_string()).await;
        context
            .add_assistant_message("Response 1".to_string())
            .await;
        context.add_user_message("Message 2".to_string()).await;

        let history = context.get_chat_history().await;
        assert_eq!(history.len(), 2);
        assert_eq!(history.messages()[0].content, "Response 1");
        assert_eq!(history.messages()[1].content, "Message 2");
    }

    #[tokio::test]
    async fn test_last_messages() {
        let context = Context::new();

        context.add_user_message("Message 1".to_string()).await;
        context
            .add_assistant_message("Response 1".to_string())
            .await;
        context.add_user_message("Message 2".to_string()).await;
        context
            .add_assistant_message("Response 2".to_string())
            .await;

        let last_two = context.get_last_messages(2).await;
        assert_eq!(last_two.len(), 2);
        assert_eq!(last_two[0].content, "Message 2");
        assert_eq!(last_two[1].content, "Response 2");
    }

    #[tokio::test]
    async fn test_context_serialization() {
        let context = Context::new();
        context.set("key", "value").await;
        context.add_user_message("test message".to_string()).await;

        let serialized = serde_json::to_string(&context).unwrap();
        let deserialized: Context = serde_json::from_str(&serialized).unwrap();

        let value: Option<String> = deserialized.get("key").await;
        assert_eq!(value, Some("value".to_string()));

        assert_eq!(deserialized.chat_history_len().await, 1);
        let history = deserialized.get_chat_history().await;
        assert_eq!(history.messages()[0].content, "test message");
        assert_eq!(history.messages()[0].role, MessageRole::User);
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
        assert_eq!(deserialized.messages()[0].content, "Hello");
        assert_eq!(deserialized.messages()[1].content, "Hi!");
    }

    #[cfg(feature = "rig")]
    #[tokio::test]
    async fn test_rig_integration() {
        let context = Context::new();

        context.add_user_message("Hello".to_string()).await;
        context.add_assistant_message("Hi there!".to_string()).await;
        context
            .add_system_message("System message".to_string())
            .await;

        let rig_messages = context.get_rig_messages().await;
        assert_eq!(rig_messages.len(), 3);

        let last_two = context.get_last_rig_messages(2).await;
        assert_eq!(last_two.len(), 2);

        // Test that the conversion works without panicking
        // We can't easily verify the content since rig::Message doesn't expose it directly
        // but we can verify the conversion completes without error
        let _debug_output = format!("{:?}", rig_messages);
        // Test passes if we reach this point without panicking
    }
}
