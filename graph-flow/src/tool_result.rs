//! Structured tool result types with error handling and fallback support.
//!
//! Provides `ToolResult` as an alternative to raw `GraphError` for tool
//! execution outcomes, supporting retry hints and fallback values.
//!
//! # Examples
//!
//! ```rust
//! use graph_flow::tool_result::ToolResult;
//!
//! // Success
//! let result = ToolResult::success(serde_json::json!({"data": "found"}));
//!
//! // Retryable error
//! let result = ToolResult::retryable_error("Rate limited, try again");
//!
//! // Non-retryable error
//! let result = ToolResult::error("Invalid API key");
//!
//! // Fallback value on error
//! let result = ToolResult::fallback(
//!     serde_json::json!({"data": "cached"}),
//!     "Live API unavailable, using cached data",
//! );
//! ```

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Structured result from a tool execution.
///
/// Maps to LangGraph's `ToolException` / `handle_tool_error` pattern,
/// but as a Rust enum with explicit retry/fallback semantics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ToolResult {
    /// Tool executed successfully.
    Success(Value),
    /// Tool failed with an error.
    Error {
        /// Human-readable error message.
        message: String,
        /// Whether the caller should retry.
        retry: bool,
    },
    /// Tool failed but a fallback value is available.
    Fallback {
        /// The fallback value to use instead.
        value: Value,
        /// Reason the fallback was used.
        reason: String,
    },
}

impl ToolResult {
    /// Create a successful tool result.
    pub fn success(value: Value) -> Self {
        Self::Success(value)
    }

    /// Create an error result (non-retryable by default).
    pub fn error(message: impl Into<String>) -> Self {
        Self::Error {
            message: message.into(),
            retry: false,
        }
    }

    /// Create a retryable error result.
    pub fn retryable_error(message: impl Into<String>) -> Self {
        Self::Error {
            message: message.into(),
            retry: true,
        }
    }

    /// Create a fallback result.
    pub fn fallback(value: Value, reason: impl Into<String>) -> Self {
        Self::Fallback {
            value,
            reason: reason.into(),
        }
    }

    /// Whether this result is a success.
    pub fn is_success(&self) -> bool {
        matches!(self, Self::Success(_))
    }

    /// Whether this result indicates a retryable error.
    pub fn is_retryable(&self) -> bool {
        matches!(self, Self::Error { retry: true, .. })
    }

    /// Get the value if success or fallback, None if error.
    pub fn value(&self) -> Option<&Value> {
        match self {
            Self::Success(v) => Some(v),
            Self::Fallback { value, .. } => Some(value),
            Self::Error { .. } => None,
        }
    }

    /// Convert to a Result, using the value for success/fallback and error message for errors.
    pub fn into_result(self) -> Result<Value, String> {
        match self {
            Self::Success(v) => Ok(v),
            Self::Fallback { value, .. } => Ok(value),
            Self::Error { message, .. } => Err(message),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_success() {
        let r = ToolResult::success(serde_json::json!({"key": "val"}));
        assert!(r.is_success());
        assert!(!r.is_retryable());
        assert_eq!(r.value().unwrap(), &serde_json::json!({"key": "val"}));
    }

    #[test]
    fn test_error() {
        let r = ToolResult::error("bad request");
        assert!(!r.is_success());
        assert!(!r.is_retryable());
        assert!(r.value().is_none());
        assert_eq!(r.into_result().unwrap_err(), "bad request");
    }

    #[test]
    fn test_retryable_error() {
        let r = ToolResult::retryable_error("rate limited");
        assert!(r.is_retryable());
        assert!(!r.is_success());
    }

    #[test]
    fn test_fallback() {
        let r = ToolResult::fallback(serde_json::json!("cached"), "API down");
        assert!(!r.is_success());
        assert_eq!(r.value().unwrap(), &serde_json::json!("cached"));
        match r {
            ToolResult::Fallback { reason, .. } => assert_eq!(reason, "API down"),
            _ => panic!("Expected Fallback"),
        }
    }

    #[test]
    fn test_into_result() {
        let ok = ToolResult::success(serde_json::json!(1));
        assert!(ok.into_result().is_ok());

        let fb = ToolResult::fallback(serde_json::json!(2), "cached");
        assert_eq!(fb.into_result().unwrap(), serde_json::json!(2));

        let err = ToolResult::error("fail");
        assert!(err.into_result().is_err());
    }

    #[test]
    fn test_serialization() {
        let r = ToolResult::success(serde_json::json!("data"));
        let json = serde_json::to_string(&r).unwrap();
        let deser: ToolResult = serde_json::from_str(&json).unwrap();
        assert!(deser.is_success());
    }
}
