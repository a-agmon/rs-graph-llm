use chrono::{DateTime, Utc};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::{Arc, RwLock};

#[cfg(feature = "rig")]
use rig::completion::Message;

/// Represents the role of a message in a conversation
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MessageRole {
    User,
    Assistant,
    System,
}

/// A serializable message that can be converted to/from rig::completion::Message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializableMessage {
    pub role: MessageRole,
    pub content: String,
    pub timestamp: DateTime<Utc>,
}

impl SerializableMessage {
    pub fn new(role: MessageRole, content: String) -> Self {
        Self {
            role,
            content,
            timestamp: Utc::now(),
        }
    }

    pub fn user(content: String) -> Self {
        Self::new(MessageRole::User, content)
    }

    pub fn assistant(content: String) -> Self {
        Self::new(MessageRole::Assistant, content)
    }

    pub fn system(content: String) -> Self {
        Self::new(MessageRole::System, content)
    }
}

/// Container for managing chat history with serialization support
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ChatHistory {
    messages: Vec<SerializableMessage>,
    max_messages: Option<usize>,
}

impl ChatHistory {
    /// Create a new empty chat history with a default limit of 1000 messages
    pub fn new() -> Self {
        Self {
            messages: Vec::new(),
            max_messages: Some(1000), // Default limit to prevent unbounded growth
        }
    }

    /// Create a new chat history with a maximum message limit
    pub fn with_max_messages(max: usize) -> Self {
        Self {
            messages: Vec::new(),
            max_messages: Some(max),
        }
    }

    /// Add a user message to the chat history
    pub fn add_user_message(&mut self, content: String) {
        self.add_message(SerializableMessage::user(content));
    }

    /// Add an assistant message to the chat history
    pub fn add_assistant_message(&mut self, content: String) {
        self.add_message(SerializableMessage::assistant(content));
    }

    /// Add a system message to the chat history
    pub fn add_system_message(&mut self, content: String) {
        self.add_message(SerializableMessage::system(content));
    }

    /// Add a message to the chat history, respecting max_messages limit
    fn add_message(&mut self, message: SerializableMessage) {
        self.messages.push(message);

        if let Some(max) = self.max_messages {
            if self.messages.len() > max {
                self.messages.drain(0..(self.messages.len() - max));
            }
        }
    }

    /// Clear all messages from the chat history
    pub fn clear(&mut self) {
        self.messages.clear();
    }

    /// Get the number of messages in the chat history
    pub fn len(&self) -> usize {
        self.messages.len()
    }

    /// Check if the chat history is empty
    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }

    /// Get a reference to all messages
    pub fn messages(&self) -> &[SerializableMessage] {
        &self.messages
    }

    /// Get the last N messages
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

/// Context for sharing data between tasks in a graph execution
/// Now includes dedicated chat history management
#[derive(Clone, Debug)]
pub struct Context {
    data: Arc<DashMap<String, Value>>,
    chat_history: Arc<RwLock<ChatHistory>>,
}

impl Context {
    /// Create a new empty context
    pub fn new() -> Self {
        Self {
            data: Arc::new(DashMap::new()),
            chat_history: Arc::new(RwLock::new(ChatHistory::new())),
        }
    }

    /// Create a new context with a maximum chat history size
    pub fn with_max_chat_messages(max: usize) -> Self {
        Self {
            data: Arc::new(DashMap::new()),
            chat_history: Arc::new(RwLock::new(ChatHistory::with_max_messages(max))),
        }
    }

    // Regular context methods (unchanged API)

    /// Set a value in the context
    pub async fn set(&self, key: impl Into<String>, value: impl serde::Serialize) {
        let value = serde_json::to_value(value).expect("Failed to serialize value");
        self.data.insert(key.into(), value);
    }

    /// Get a value from the context
    pub async fn get<T: serde::de::DeserializeOwned>(&self, key: &str) -> Option<T> {
        self.data
            .get(key)
            .and_then(|v| serde_json::from_value(v.clone()).ok())
    }

    /// Remove a value from the context
    pub async fn remove(&self, key: &str) -> Option<Value> {
        self.data.remove(key).map(|(_, v)| v)
    }

    /// Clear all regular context data (does not affect chat history)
    pub async fn clear(&self) {
        self.data.clear();
    }

    /// Synchronous version of get for use in edge conditions
    pub fn get_sync<T: serde::de::DeserializeOwned>(&self, key: &str) -> Option<T> {
        self.data
            .get(key)
            .and_then(|v| serde_json::from_value(v.clone()).ok())
    }

    /// Synchronous version of set for use when async is not available
    pub fn set_sync(&self, key: impl Into<String>, value: impl serde::Serialize) {
        let value = serde_json::to_value(value).expect("Failed to serialize value");
        self.data.insert(key.into(), value);
    }

    // Chat history methods

    /// Add a user message to the chat history
    pub async fn add_user_message(&self, content: String) {
        if let Ok(mut history) = self.chat_history.write() {
            history.add_user_message(content);
        }
    }

    /// Add an assistant message to the chat history
    pub async fn add_assistant_message(&self, content: String) {
        if let Ok(mut history) = self.chat_history.write() {
            history.add_assistant_message(content);
        }
    }

    /// Add a system message to the chat history
    pub async fn add_system_message(&self, content: String) {
        if let Ok(mut history) = self.chat_history.write() {
            history.add_system_message(content);
        }
    }

    /// Get a clone of the current chat history
    pub async fn get_chat_history(&self) -> ChatHistory {
        if let Ok(history) = self.chat_history.read() {
            history.clone()
        } else {
            ChatHistory::new()
        }
    }

    /// Clear the chat history
    pub async fn clear_chat_history(&self) {
        if let Ok(mut history) = self.chat_history.write() {
            history.clear();
        }
    }

    /// Get the number of messages in the chat history
    pub async fn chat_history_len(&self) -> usize {
        if let Ok(history) = self.chat_history.read() {
            history.len()
        } else {
            0
        }
    }

    /// Check if the chat history is empty
    pub async fn is_chat_history_empty(&self) -> bool {
        if let Ok(history) = self.chat_history.read() {
            history.is_empty()
        } else {
            true
        }
    }

    /// Get the last N messages from chat history
    pub async fn get_last_messages(&self, n: usize) -> Vec<SerializableMessage> {
        if let Ok(history) = self.chat_history.read() {
            history.last_messages(n).to_vec()
        } else {
            Vec::new()
        }
    }

    /// Get all messages from chat history as SerializableMessage
    pub async fn get_all_messages(&self) -> Vec<SerializableMessage> {
        if let Ok(history) = self.chat_history.read() {
            history.messages().to_vec()
        } else {
            Vec::new()
        }
    }

    // Rig integration methods (only available when rig feature is enabled)

    #[cfg(feature = "rig")]
    /// Get all chat history messages converted to rig::completion::Message format
    /// This method is only available when the "rig" feature is enabled
    pub async fn get_rig_messages(&self) -> Vec<Message> {
        let messages = self.get_all_messages().await;
        messages
            .iter()
            .map(|msg| self.to_rig_message(msg))
            .collect()
    }

    #[cfg(feature = "rig")]
    /// Get the last N messages converted to rig::completion::Message format
    /// This method is only available when the "rig" feature is enabled
    pub async fn get_last_rig_messages(&self, n: usize) -> Vec<Message> {
        let messages = self.get_last_messages(n).await;
        messages
            .iter()
            .map(|msg| self.to_rig_message(msg))
            .collect()
    }

    #[cfg(feature = "rig")]
    /// Convert a SerializableMessage to a rig::completion::Message
    /// This method is only available when the "rig" feature is enabled
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
