//! Bridge module for converting between SerializableMessage and rig::completion::Message
//! This module provides conversion utilities to work with the rig library.

use graph_flow::{MessageRole, SerializableMessage};
use rig::completion::Message;

/// Convert a SerializableMessage to a rig::completion::Message
pub fn to_rig_message(msg: &SerializableMessage) -> Message {
    match msg.role {
        MessageRole::User => Message::user(msg.content.clone()),
        MessageRole::Assistant => Message::assistant(msg.content.clone()),
        // rig doesn't have a system message type, so we'll treat it as a user message
        // with a system prefix
        MessageRole::System => Message::user(format!("[SYSTEM] {}", msg.content)),
    }
}

/// Convert a rig::completion::Message to a SerializableMessage
/// Note: This is a lossy conversion since rig::Message doesn't expose its internal structure
#[allow(dead_code)]
pub fn from_rig_message(msg: &Message) -> SerializableMessage {
    // Since rig::completion::Message doesn't expose its content or role directly,
    // and doesn't implement Display, we need to work around this limitation.
    // For now, we'll use Debug formatting and try to extract what we can.
    let debug_str = format!("{:?}", msg);
    
    // Try to determine role and content from debug string
    // This is a best-effort approach and might need refinement based on actual rig implementation
    if debug_str.contains("user") || debug_str.contains("User") {
        // Extract content if possible, otherwise use the debug string
        SerializableMessage::user(debug_str)
    } else if debug_str.contains("assistant") || debug_str.contains("Assistant") {
        SerializableMessage::assistant(debug_str)
    } else {
        // Default to user role
        SerializableMessage::user(debug_str)
    }
}

/// Convert a vector of SerializableMessage to rig::completion::Message vector
pub fn to_rig_messages(messages: &[SerializableMessage]) -> Vec<Message> {
    messages.iter().map(to_rig_message).collect()
}

/// Convert a vector of rig::completion::Message to SerializableMessage vector
#[allow(dead_code)]
pub fn from_rig_messages(messages: &[Message]) -> Vec<SerializableMessage> {
    messages.iter().map(from_rig_message).collect()
}

/// Helper trait to add rig conversion methods to Context
pub trait ContextRigExt {
    /// Get all chat history messages converted to rig::completion::Message format
    async fn get_rig_messages(&self) -> Vec<Message>;
    
    /// Get the last N messages converted to rig::completion::Message format
    #[allow(dead_code)]
    async fn get_last_rig_messages(&self, n: usize) -> Vec<Message>;
}

impl ContextRigExt for graph_flow::Context {
    async fn get_rig_messages(&self) -> Vec<Message> {
        let messages = self.get_all_messages().await;
        to_rig_messages(&messages)
    }
    
    async fn get_last_rig_messages(&self, n: usize) -> Vec<Message> {
        let messages = self.get_last_messages(n).await;
        to_rig_messages(&messages)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use graph_flow::Context;

    #[tokio::test]
    async fn test_context_rig_extension() {
        let context = Context::new();
        
        context.add_user_message("Hello".to_string()).await;
        context.add_assistant_message("Hi there!".to_string()).await;
        
        let rig_messages = context.get_rig_messages().await;
        assert_eq!(rig_messages.len(), 2);
        
        let last_message = context.get_last_rig_messages(1).await;
        assert_eq!(last_message.len(), 1);
    }

    #[test]
    fn test_message_conversion() {
        let serializable = SerializableMessage::user("test content".to_string());
        let rig_msg = to_rig_message(&serializable);
        
        // Test that the conversion doesn't panic and produces a Message
        // We can't easily verify the content since rig::Message doesn't expose it directly
        // but we can verify the conversion completes without error
        let _debug_output = format!("{:?}", rig_msg);
        // Test passes if we reach this point without panicking
    }

    #[test]
    fn test_batch_conversion() {
        let messages = vec![
            SerializableMessage::user("Hello".to_string()),
            SerializableMessage::assistant("Hi".to_string()),
            SerializableMessage::system("System message".to_string()),
        ];
        
        let rig_messages = to_rig_messages(&messages);
        assert_eq!(rig_messages.len(), 3);
    }
}