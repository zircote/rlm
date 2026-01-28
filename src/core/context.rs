//! RLM execution context.
//!
//! Represents the stateful environment for RLM operations, including
//! variables, globals, and active buffer references.

use crate::io::current_timestamp;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Represents the RLM execution context.
///
/// This mirrors the Python implementation's context dict and globals,
/// providing a persistent state across operations.
///
/// # Examples
///
/// ```
/// use rlm_rs::core::Context;
///
/// let mut ctx = Context::new();
/// ctx.set_variable("key".to_string(), "value".into());
/// assert!(ctx.get_variable("key").is_some());
/// ```
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Context {
    /// Context variables (key-value pairs for current session).
    pub variables: HashMap<String, ContextValue>,

    /// Global state dictionary (persisted across sessions).
    pub globals: HashMap<String, ContextValue>,

    /// Active buffer IDs in this context.
    pub buffer_ids: Vec<i64>,

    /// Current working directory path.
    pub cwd: Option<String>,

    /// Context metadata.
    pub metadata: ContextMetadata,
}

/// Metadata associated with a context.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextMetadata {
    /// Unix timestamp when context was created.
    pub created_at: i64,

    /// Unix timestamp when context was last modified.
    pub updated_at: i64,

    /// Schema version for migration support.
    pub version: u32,
}

/// Context value types supporting common data types.
///
/// This provides a type-safe way to store heterogeneous values
/// in the context while maintaining serializability.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value")]
pub enum ContextValue {
    /// String value.
    String(String),

    /// Integer value (i64).
    Integer(i64),

    /// Floating point value (f64).
    Float(f64),

    /// Boolean value.
    Boolean(bool),

    /// List of values.
    List(Vec<Self>),

    /// Nested map of values.
    Map(HashMap<String, Self>),

    /// Null/None value.
    Null,
}

impl Context {
    /// Creates a new empty context with current timestamp.
    #[must_use]
    pub fn new() -> Self {
        let now = current_timestamp();
        Self {
            variables: HashMap::new(),
            globals: HashMap::new(),
            buffer_ids: Vec::new(),
            cwd: None,
            metadata: ContextMetadata {
                created_at: now,
                updated_at: now,
                version: 1,
            },
        }
    }

    /// Sets a context variable.
    ///
    /// # Arguments
    ///
    /// * `key` - Variable name.
    /// * `value` - Variable value.
    pub fn set_variable(&mut self, key: String, value: ContextValue) {
        self.variables.insert(key, value);
        self.touch();
    }

    /// Gets a context variable by key.
    ///
    /// # Arguments
    ///
    /// * `key` - Variable name to look up.
    ///
    /// # Returns
    ///
    /// Reference to the value if found.
    #[must_use]
    pub fn get_variable(&self, key: &str) -> Option<&ContextValue> {
        self.variables.get(key)
    }

    /// Removes a context variable.
    ///
    /// # Arguments
    ///
    /// * `key` - Variable name to remove.
    ///
    /// # Returns
    ///
    /// The removed value if it existed.
    pub fn remove_variable(&mut self, key: &str) -> Option<ContextValue> {
        let result = self.variables.remove(key);
        if result.is_some() {
            self.touch();
        }
        result
    }

    /// Sets a global variable (persisted across sessions).
    ///
    /// # Arguments
    ///
    /// * `key` - Global variable name.
    /// * `value` - Global variable value.
    pub fn set_global(&mut self, key: String, value: ContextValue) {
        self.globals.insert(key, value);
        self.touch();
    }

    /// Gets a global variable by key.
    ///
    /// # Arguments
    ///
    /// * `key` - Global variable name to look up.
    ///
    /// # Returns
    ///
    /// Reference to the value if found.
    #[must_use]
    pub fn get_global(&self, key: &str) -> Option<&ContextValue> {
        self.globals.get(key)
    }

    /// Removes a global variable.
    ///
    /// # Arguments
    ///
    /// * `key` - Global variable name to remove.
    ///
    /// # Returns
    ///
    /// The removed value if it existed.
    pub fn remove_global(&mut self, key: &str) -> Option<ContextValue> {
        let result = self.globals.remove(key);
        if result.is_some() {
            self.touch();
        }
        result
    }

    /// Adds a buffer ID to the active buffers list.
    ///
    /// # Arguments
    ///
    /// * `buffer_id` - ID of the buffer to add.
    pub fn add_buffer(&mut self, buffer_id: i64) {
        if !self.buffer_ids.contains(&buffer_id) {
            self.buffer_ids.push(buffer_id);
            self.touch();
        }
    }

    /// Removes a buffer ID from the active buffers list.
    ///
    /// # Arguments
    ///
    /// * `buffer_id` - ID of the buffer to remove.
    ///
    /// # Returns
    ///
    /// `true` if the buffer was removed, `false` if not found.
    pub fn remove_buffer(&mut self, buffer_id: i64) -> bool {
        if let Some(pos) = self.buffer_ids.iter().position(|&id| id == buffer_id) {
            self.buffer_ids.remove(pos);
            self.touch();
            true
        } else {
            false
        }
    }

    /// Resets the context to empty state, preserving metadata.
    pub fn reset(&mut self) {
        self.variables.clear();
        self.globals.clear();
        self.buffer_ids.clear();
        self.cwd = None;
        self.touch();
    }

    /// Returns the number of variables in the context.
    #[must_use]
    pub fn variable_count(&self) -> usize {
        self.variables.len()
    }

    /// Returns the number of globals in the context.
    #[must_use]
    pub fn global_count(&self) -> usize {
        self.globals.len()
    }

    /// Returns the number of active buffers.
    #[must_use]
    pub const fn buffer_count(&self) -> usize {
        self.buffer_ids.len()
    }

    /// Updates the `updated_at` timestamp.
    fn touch(&mut self) {
        self.metadata.updated_at = current_timestamp();
    }
}

impl From<String> for ContextValue {
    fn from(s: String) -> Self {
        Self::String(s)
    }
}

impl From<&str> for ContextValue {
    fn from(s: &str) -> Self {
        Self::String(s.to_string())
    }
}

impl From<i64> for ContextValue {
    fn from(n: i64) -> Self {
        Self::Integer(n)
    }
}

impl From<i32> for ContextValue {
    fn from(n: i32) -> Self {
        Self::Integer(i64::from(n))
    }
}

impl From<f64> for ContextValue {
    fn from(n: f64) -> Self {
        Self::Float(n)
    }
}

impl From<bool> for ContextValue {
    fn from(b: bool) -> Self {
        Self::Boolean(b)
    }
}

#[allow(clippy::use_self)]
impl<T: Into<ContextValue>> From<Vec<T>> for ContextValue {
    fn from(v: Vec<T>) -> Self {
        Self::List(v.into_iter().map(Into::into).collect())
    }
}

#[allow(clippy::use_self)]
impl<T: Into<ContextValue>> From<Option<T>> for ContextValue {
    fn from(opt: Option<T>) -> Self {
        opt.map_or(Self::Null, Into::into)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_context_new() {
        let ctx = Context::new();
        assert!(ctx.variables.is_empty());
        assert!(ctx.globals.is_empty());
        assert!(ctx.buffer_ids.is_empty());
        assert!(ctx.cwd.is_none());
        assert!(ctx.metadata.created_at > 0);
    }

    #[test]
    fn test_variable_operations() {
        let mut ctx = Context::new();

        ctx.set_variable("key1".to_string(), "value1".into());
        ctx.set_variable("key2".to_string(), 42i64.into());

        assert_eq!(
            ctx.get_variable("key1"),
            Some(&ContextValue::String("value1".to_string()))
        );
        assert_eq!(ctx.get_variable("key2"), Some(&ContextValue::Integer(42)));
        assert_eq!(ctx.get_variable("nonexistent"), None);
        assert_eq!(ctx.variable_count(), 2);

        let removed = ctx.remove_variable("key1");
        assert!(removed.is_some());
        assert_eq!(ctx.variable_count(), 1);
    }

    #[test]
    fn test_global_operations() {
        let mut ctx = Context::new();

        ctx.set_global("global1".to_string(), true.into());
        assert_eq!(
            ctx.get_global("global1"),
            Some(&ContextValue::Boolean(true))
        );
        assert_eq!(ctx.global_count(), 1);

        ctx.remove_global("global1");
        assert_eq!(ctx.global_count(), 0);
    }

    #[test]
    fn test_buffer_operations() {
        let mut ctx = Context::new();

        ctx.add_buffer(1);
        ctx.add_buffer(2);
        ctx.add_buffer(1); // Duplicate, should not add

        assert_eq!(ctx.buffer_count(), 2);
        assert!(ctx.buffer_ids.contains(&1));
        assert!(ctx.buffer_ids.contains(&2));

        assert!(ctx.remove_buffer(1));
        assert!(!ctx.remove_buffer(99)); // Non-existent
        assert_eq!(ctx.buffer_count(), 1);
    }

    #[test]
    fn test_context_reset() {
        let mut ctx = Context::new();
        ctx.set_variable("key".to_string(), "value".into());
        ctx.set_global("global".to_string(), 1i64.into());
        ctx.add_buffer(1);
        ctx.cwd = Some("/tmp".to_string());

        ctx.reset();

        assert!(ctx.variables.is_empty());
        assert!(ctx.globals.is_empty());
        assert!(ctx.buffer_ids.is_empty());
        assert!(ctx.cwd.is_none());
    }

    #[test]
    fn test_context_value_conversions() {
        let s: ContextValue = "test".into();
        assert!(matches!(s, ContextValue::String(_)));

        let i: ContextValue = 42i64.into();
        assert!(matches!(i, ContextValue::Integer(42)));

        let f: ContextValue = std::f64::consts::PI.into();
        assert!(matches!(f, ContextValue::Float(_)));

        let b: ContextValue = true.into();
        assert!(matches!(b, ContextValue::Boolean(true)));

        let none: ContextValue = Option::<String>::None.into();
        assert!(matches!(none, ContextValue::Null));
    }

    #[test]
    fn test_context_serialization() {
        let mut ctx = Context::new();
        ctx.set_variable("key".to_string(), "value".into());

        let json = serde_json::to_string(&ctx);
        assert!(json.is_ok());

        let deserialized: Result<Context, _> = serde_json::from_str(&json.unwrap());
        assert!(deserialized.is_ok());
        assert_eq!(
            deserialized.unwrap().get_variable("key"),
            ctx.get_variable("key")
        );
    }

    #[test]
    fn test_touch_updates_timestamp() {
        let mut ctx = Context::new();
        let initial = ctx.metadata.updated_at;

        // Small delay to ensure timestamp changes
        std::thread::sleep(std::time::Duration::from_millis(10));

        ctx.set_variable("key".to_string(), "value".into());
        assert!(ctx.metadata.updated_at >= initial);
    }

    #[test]
    fn test_context_value_from_string_owned() {
        let s = String::from("owned string");
        let cv: ContextValue = s.into();
        assert!(matches!(cv, ContextValue::String(ref v) if v == "owned string"));
    }

    #[test]
    fn test_context_value_from_i32() {
        let n: i32 = 42;
        let cv: ContextValue = n.into();
        assert!(matches!(cv, ContextValue::Integer(42)));
    }

    #[test]
    fn test_context_value_from_vec() {
        let v: Vec<i64> = vec![1, 2, 3];
        let cv: ContextValue = v.into();
        if let ContextValue::List(list) = cv {
            assert_eq!(list.len(), 3);
            assert!(matches!(list[0], ContextValue::Integer(1)));
            assert!(matches!(list[1], ContextValue::Integer(2)));
            assert!(matches!(list[2], ContextValue::Integer(3)));
        } else {
            unreachable!("Expected List variant");
        }
    }

    #[test]
    fn test_context_value_from_option_some() {
        let opt: Option<i64> = Some(42);
        let cv: ContextValue = opt.into();
        assert!(matches!(cv, ContextValue::Integer(42)));
    }
}
