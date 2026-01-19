//! Integration tests for {{crate_name}}.

use {{crate_name}}::{add, divide, Config, Error, Result};

#[test]
fn test_add_integration() {
    // Test basic addition
    assert_eq!(add(1, 2), 3);
    assert_eq!(add(-5, 5), 0);

    // Test boundary conditions
    assert_eq!(add(i64::MAX, 0), i64::MAX);
    assert_eq!(add(i64::MIN, 0), i64::MIN);
}

#[test]
fn test_divide_integration() {
    // Test successful division
    assert_eq!(divide(100, 10).unwrap(), 10);
    assert_eq!(divide(-100, 10).unwrap(), -10);
    assert_eq!(divide(100, -10).unwrap(), -10);
    assert_eq!(divide(-100, -10).unwrap(), 10);

    // Test integer division truncation
    assert_eq!(divide(7, 3).unwrap(), 2);
    assert_eq!(divide(-7, 3).unwrap(), -2);
}

#[test]
fn test_divide_by_zero() {
    let result = divide(42, 0);
    assert!(result.is_err());

    if let Err(Error::InvalidInput(msg)) = result {
        assert!(msg.contains("zero"), "Error message should mention zero");
    } else {
        panic!("Expected InvalidInput error");
    }
}

#[test]
fn test_config_builder_pattern() {
    let config = Config::new()
        .with_verbose(true)
        .with_max_retries(10)
        .with_timeout(120);

    assert!(config.verbose);
    assert_eq!(config.max_retries, 10);
    assert_eq!(config.timeout_secs, 120);
}

#[test]
fn test_config_clone() {
    let config1 = Config::new().with_verbose(true);
    let config2 = config1.clone();

    assert_eq!(config1.verbose, config2.verbose);
    assert_eq!(config1.max_retries, config2.max_retries);
    assert_eq!(config1.timeout_secs, config2.timeout_secs);
}

#[test]
fn test_error_types() {
    // Test InvalidInput error
    let err = Error::InvalidInput("test message".to_string());
    let display = format!("{err}");
    assert!(display.contains("invalid input"));
    assert!(display.contains("test message"));

    // Test OperationFailed error
    let err = Error::OperationFailed {
        operation: "read".to_string(),
        cause: "file not found".to_string(),
    };
    let display = format!("{err}");
    assert!(display.contains("read"));
    assert!(display.contains("file not found"));
}

/// Helper function demonstrating Result handling patterns.
fn process_numbers(a: i64, b: i64) -> Result<i64> {
    let sum = add(a, b);
    divide(sum, 2)
}

#[test]
fn test_result_chaining() {
    // Successful case
    let result = process_numbers(10, 6);
    assert_eq!(result.unwrap(), 8);

    // Error case (would need different logic to trigger)
}

mod property_tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn add_is_commutative(a in any::<i32>(), b in any::<i32>()) {
            let a = i64::from(a);
            let b = i64::from(b);
            prop_assert_eq!(add(a, b), add(b, a));
        }

        #[test]
        fn add_zero_is_identity(n in any::<i64>()) {
            prop_assert_eq!(add(n, 0), n);
            prop_assert_eq!(add(0, n), n);
        }

        #[test]
        fn divide_by_one_is_identity(n in any::<i64>()) {
            prop_assert_eq!(divide(n, 1).unwrap(), n);
        }

        #[test]
        fn divide_by_nonzero_succeeds(dividend in any::<i64>(), divisor in any::<i64>().prop_filter("non-zero", |&x| x != 0)) {
            prop_assert!(divide(dividend, divisor).is_ok());
        }
    }
}
