//! Retry policy for automatic task retry on failure.
//!
//! Provides configurable retry with fixed or exponential backoff.
//!
//! # Examples
//!
//! ```rust
//! use graph_flow::retry::{RetryPolicy, BackoffStrategy};
//! use std::time::Duration;
//!
//! let policy = RetryPolicy::new(3, BackoffStrategy::Exponential {
//!     base: Duration::from_millis(100),
//!     max: Duration::from_secs(5),
//! });
//! ```

use std::time::Duration;

/// Configuration for automatic retry on task failure.
#[derive(Debug, Clone)]
pub struct RetryPolicy {
    /// Maximum number of retries before giving up.
    pub max_retries: usize,
    /// Backoff strategy between retries.
    pub backoff: BackoffStrategy,
}

impl RetryPolicy {
    pub fn new(max_retries: usize, backoff: BackoffStrategy) -> Self {
        Self {
            max_retries,
            backoff,
        }
    }

    /// Create a simple retry policy with fixed delay.
    pub fn fixed(max_retries: usize, delay: Duration) -> Self {
        Self {
            max_retries,
            backoff: BackoffStrategy::Fixed(delay),
        }
    }

    /// Create a retry policy with exponential backoff.
    pub fn exponential(max_retries: usize, base: Duration, max: Duration) -> Self {
        Self {
            max_retries,
            backoff: BackoffStrategy::Exponential { base, max },
        }
    }

    /// Compute the delay for a given attempt (0-indexed).
    pub fn delay_for_attempt(&self, attempt: usize) -> Duration {
        match &self.backoff {
            BackoffStrategy::Fixed(d) => *d,
            BackoffStrategy::Exponential { base, max } => {
                let delay = base.saturating_mul(1u32.wrapping_shl(attempt as u32));
                delay.min(*max)
            }
            BackoffStrategy::None => Duration::ZERO,
        }
    }
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_retries: 3,
            backoff: BackoffStrategy::Exponential {
                base: Duration::from_millis(100),
                max: Duration::from_secs(10),
            },
        }
    }
}

/// Strategy for computing delay between retries.
#[derive(Debug, Clone)]
pub enum BackoffStrategy {
    /// No delay between retries.
    None,
    /// Fixed delay between every retry.
    Fixed(Duration),
    /// Exponential backoff: delay = base * 2^attempt, capped at max.
    Exponential {
        base: Duration,
        max: Duration,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fixed_delay() {
        let policy = RetryPolicy::fixed(3, Duration::from_millis(500));
        assert_eq!(policy.delay_for_attempt(0), Duration::from_millis(500));
        assert_eq!(policy.delay_for_attempt(2), Duration::from_millis(500));
    }

    #[test]
    fn test_exponential_delay() {
        let policy = RetryPolicy::exponential(
            5,
            Duration::from_millis(100),
            Duration::from_secs(5),
        );
        assert_eq!(policy.delay_for_attempt(0), Duration::from_millis(100));
        assert_eq!(policy.delay_for_attempt(1), Duration::from_millis(200));
        assert_eq!(policy.delay_for_attempt(2), Duration::from_millis(400));
        // Should cap at max
        assert_eq!(policy.delay_for_attempt(20), Duration::from_secs(5));
    }

    #[test]
    fn test_default_policy() {
        let policy = RetryPolicy::default();
        assert_eq!(policy.max_retries, 3);
        assert!(matches!(policy.backoff, BackoffStrategy::Exponential { .. }));
    }

    #[test]
    fn test_no_backoff() {
        let policy = RetryPolicy::new(2, BackoffStrategy::None);
        assert_eq!(policy.delay_for_attempt(0), Duration::ZERO);
    }
}
