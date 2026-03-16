//! Channel abstractions with reducer semantics.
//!
//! Provides `ChannelReducer` and `ChannelConfig` for LangGraph-style
//! state management where values can be appended, overwritten, or
//! merged using custom reducer functions.
//!
//! # Examples
//!
//! ```rust
//! use graph_flow::channels::{ChannelReducer, Channels};
//!
//! let mut channels = Channels::new();
//! channels.register("messages", ChannelReducer::Append);
//! channels.register("current_step", ChannelReducer::LastValue);
//!
//! // Append mode: values accumulate
//! channels.apply("messages", serde_json::json!("hello"));
//! channels.apply("messages", serde_json::json!("world"));
//! let messages = channels.get("messages").unwrap();
//! assert_eq!(messages, serde_json::json!(["hello", "world"]));
//!
//! // LastValue mode: overwrite
//! channels.apply("current_step", serde_json::json!("step_1"));
//! channels.apply("current_step", serde_json::json!("step_2"));
//! assert_eq!(channels.get("current_step").unwrap(), serde_json::json!("step_2"));
//! ```

use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

/// Reducer function type for custom channel merging.
pub type ReducerFn = Arc<dyn Fn(Value, Value) -> Value + Send + Sync>;

/// Defines how values are reduced when written to a channel.
#[derive(Clone)]
pub enum ChannelReducer {
    /// Overwrite with the latest value (default behavior).
    LastValue,
    /// Append to a JSON array.
    Append,
    /// Use a custom merge function: `new_state = f(old_state, new_value)`.
    Custom(ReducerFn),
}

impl std::fmt::Debug for ChannelReducer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::LastValue => write!(f, "LastValue"),
            Self::Append => write!(f, "Append"),
            Self::Custom(_) => write!(f, "Custom(fn)"),
        }
    }
}

/// Configuration for a single channel.
#[derive(Debug, Clone)]
pub struct ChannelConfig {
    /// The key name for this channel.
    pub key: String,
    /// The reducer to apply when values are written.
    pub reducer: ChannelReducer,
    /// Optional default value.
    pub default: Option<Value>,
}

impl ChannelConfig {
    pub fn new(key: impl Into<String>, reducer: ChannelReducer) -> Self {
        Self {
            key: key.into(),
            reducer,
            default: None,
        }
    }

    pub fn with_default(mut self, default: Value) -> Self {
        self.default = Some(default);
        self
    }
}

/// A collection of channels with reducer semantics.
///
/// Values written to channels are reduced according to their configured
/// reducer (LastValue, Append, or Custom).
pub struct Channels {
    configs: HashMap<String, ChannelReducer>,
    values: HashMap<String, Value>,
}

impl Channels {
    pub fn new() -> Self {
        Self {
            configs: HashMap::new(),
            values: HashMap::new(),
        }
    }

    /// Register a channel with a reducer.
    pub fn register(&mut self, key: impl Into<String>, reducer: ChannelReducer) {
        self.configs.insert(key.into(), reducer);
    }

    /// Register a channel with a config (includes default value).
    pub fn register_config(&mut self, config: ChannelConfig) {
        if let Some(default) = &config.default {
            self.values.insert(config.key.clone(), default.clone());
        }
        self.configs.insert(config.key, config.reducer);
    }

    /// Apply a value to a channel using its configured reducer.
    ///
    /// If the channel has no configured reducer, it defaults to LastValue.
    pub fn apply(&mut self, key: &str, value: Value) {
        let reducer = self.configs.get(key).cloned().unwrap_or(ChannelReducer::LastValue);

        let new_value = match reducer {
            ChannelReducer::LastValue => value,
            ChannelReducer::Append => {
                let mut arr = match self.values.remove(key) {
                    Some(Value::Array(a)) => a,
                    Some(other) => vec![other],
                    None => Vec::new(),
                };
                arr.push(value);
                Value::Array(arr)
            }
            ChannelReducer::Custom(f) => {
                let old = self.values.remove(key).unwrap_or(Value::Null);
                f(old, value)
            }
        };

        self.values.insert(key.to_string(), new_value);
    }

    /// Get the current value of a channel.
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.values.get(key)
    }

    /// Get all channel values as a HashMap.
    pub fn snapshot(&self) -> &HashMap<String, Value> {
        &self.values
    }

    /// Clear all channel values.
    pub fn clear(&mut self) {
        self.values.clear();
    }

    /// List all registered channel keys.
    pub fn keys(&self) -> Vec<&String> {
        self.configs.keys().collect()
    }
}

impl Default for Channels {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_last_value_reducer() {
        let mut channels = Channels::new();
        channels.register("step", ChannelReducer::LastValue);

        channels.apply("step", serde_json::json!("step_1"));
        channels.apply("step", serde_json::json!("step_2"));

        assert_eq!(channels.get("step").unwrap(), &serde_json::json!("step_2"));
    }

    #[test]
    fn test_append_reducer() {
        let mut channels = Channels::new();
        channels.register("messages", ChannelReducer::Append);

        channels.apply("messages", serde_json::json!("hello"));
        channels.apply("messages", serde_json::json!("world"));

        assert_eq!(
            channels.get("messages").unwrap(),
            &serde_json::json!(["hello", "world"])
        );
    }

    #[test]
    fn test_custom_reducer() {
        let sum_reducer: ReducerFn = Arc::new(|old, new| {
            let old_n = old.as_f64().unwrap_or(0.0);
            let new_n = new.as_f64().unwrap_or(0.0);
            serde_json::json!(old_n + new_n)
        });

        let mut channels = Channels::new();
        channels.register("total", ChannelReducer::Custom(sum_reducer));

        channels.apply("total", serde_json::json!(10));
        channels.apply("total", serde_json::json!(5));
        channels.apply("total", serde_json::json!(3));

        assert_eq!(channels.get("total").unwrap(), &serde_json::json!(18.0));
    }

    #[test]
    fn test_default_value() {
        let mut channels = Channels::new();
        channels.register_config(
            ChannelConfig::new("counter", ChannelReducer::LastValue)
                .with_default(serde_json::json!(0)),
        );

        assert_eq!(channels.get("counter").unwrap(), &serde_json::json!(0));
    }

    #[test]
    fn test_unregistered_channel_defaults_to_last_value() {
        let mut channels = Channels::new();
        channels.apply("unknown", serde_json::json!("a"));
        channels.apply("unknown", serde_json::json!("b"));
        assert_eq!(channels.get("unknown").unwrap(), &serde_json::json!("b"));
    }

    #[test]
    fn test_clear_and_keys() {
        let mut channels = Channels::new();
        channels.register("a", ChannelReducer::LastValue);
        channels.register("b", ChannelReducer::Append);
        channels.apply("a", serde_json::json!(1));
        channels.apply("b", serde_json::json!(2));

        assert_eq!(channels.keys().len(), 2);
        channels.clear();
        assert!(channels.get("a").is_none());
    }
}
