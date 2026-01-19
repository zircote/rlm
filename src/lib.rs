//! # {{crate_name}}
//!
//! A Rust crate description.
//!
//! ## Features
//!
//! - Feature 1
//! - Feature 2
//! - Feature 3
//!
//! ## Example
//!
//! ```rust
//! use {{crate_name}}::add;
//!
//! let result = add(2, 3);
//! assert_eq!(result, 5);
//! ```

#![doc = include_str!("../README.md")]
#![deny(clippy::all)]
#![warn(clippy::pedantic)]
#![warn(clippy::nursery)]
#![warn(missing_docs)]
#![forbid(unsafe_code)]

use std::error::Error as StdError;
use std::fmt;

/// Error type for {{crate_name}} operations.
#[derive(Debug)]
pub enum Error {
    /// Invalid input was provided.
    InvalidInput(String),
    /// An operation failed.
    OperationFailed {
        /// The operation that failed.
        operation: String,
        /// The underlying cause.
        cause: String,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInput(msg) => write!(f, "invalid input: {msg}"),
            Self::OperationFailed { operation, cause } => {
                write!(f, "operation '{operation}' failed: {cause}")
            }
        }
    }
}

impl StdError for Error {}

/// Result type alias for {{crate_name}} operations.
pub type Result<T> = std::result::Result<T, Error>;

/// Adds two numbers together.
///
/// # Arguments
///
/// * `a` - The first number.
/// * `b` - The second number.
///
/// # Returns
///
/// The sum of `a` and `b`.
///
/// # Examples
///
/// ```rust
/// use {{crate_name}}::add;
///
/// assert_eq!(add(2, 3), 5);
/// assert_eq!(add(-1, 1), 0);
/// ```
#[must_use]
pub const fn add(a: i64, b: i64) -> i64 {
    a + b
}

/// Divides two numbers.
///
/// # Arguments
///
/// * `dividend` - The number to divide.
/// * `divisor` - The number to divide by.
///
/// # Returns
///
/// The quotient, or an error if `divisor` is zero.
///
/// # Errors
///
/// Returns [`Error::InvalidInput`] if `divisor` is zero.
///
/// # Examples
///
/// ```rust
/// use {{crate_name}}::divide;
///
/// assert_eq!(divide(10, 2).unwrap(), 5);
/// assert!(divide(10, 0).is_err());
/// ```
pub fn divide(dividend: i64, divisor: i64) -> Result<i64> {
    if divisor == 0 {
        return Err(Error::InvalidInput("divisor cannot be zero".to_string()));
    }
    Ok(dividend / divisor)
}

/// Configuration for the crate.
#[derive(Debug, Clone, Default)]
pub struct Config {
    /// Enable verbose logging.
    pub verbose: bool,
    /// Maximum number of retries.
    pub max_retries: u32,
    /// Timeout in seconds.
    pub timeout_secs: u64,
}

impl Config {
    /// Creates a new configuration with default values.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            verbose: false,
            max_retries: 3,
            timeout_secs: 30,
        }
    }

    /// Sets the verbose flag.
    #[must_use]
    pub const fn with_verbose(mut self, verbose: bool) -> Self {
        self.verbose = verbose;
        self
    }

    /// Sets the maximum retries.
    #[must_use]
    pub const fn with_max_retries(mut self, retries: u32) -> Self {
        self.max_retries = retries;
        self
    }

    /// Sets the timeout in seconds.
    #[must_use]
    pub const fn with_timeout(mut self, secs: u64) -> Self {
        self.timeout_secs = secs;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add() {
        assert_eq!(add(2, 3), 5);
        assert_eq!(add(-1, 1), 0);
        assert_eq!(add(0, 0), 0);
        assert_eq!(add(i64::MAX, 0), i64::MAX);
    }

    #[test]
    fn test_divide_success() {
        assert_eq!(divide(10, 2).unwrap(), 5);
        assert_eq!(divide(-10, 2).unwrap(), -5);
        assert_eq!(divide(0, 5).unwrap(), 0);
    }

    #[test]
    fn test_divide_by_zero() {
        let result = divide(10, 0);
        assert!(result.is_err());
        match result.unwrap_err() {
            Error::InvalidInput(msg) => assert!(msg.contains("zero")),
            _ => panic!("Expected InvalidInput error"),
        }
    }

    #[test]
    fn test_config_builder() {
        let config = Config::new()
            .with_verbose(true)
            .with_max_retries(5)
            .with_timeout(60);

        assert!(config.verbose);
        assert_eq!(config.max_retries, 5);
        assert_eq!(config.timeout_secs, 60);
    }

    #[test]
    fn test_config_default() {
        let config = Config::default();
        assert!(!config.verbose);
        assert_eq!(config.max_retries, 3);
        assert_eq!(config.timeout_secs, 30);
    }

    #[test]
    fn test_error_display() {
        let err = Error::InvalidInput("test error".to_string());
        assert_eq!(err.to_string(), "invalid input: test error");

        let err = Error::OperationFailed {
            operation: "test".to_string(),
            cause: "failed".to_string(),
        };
        assert_eq!(err.to_string(), "operation 'test' failed: failed");
    }
}
