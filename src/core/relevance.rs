//! Relevance level for query findings.
//!
//! This type lives in `core` (outside the `agent` feature gate) so that
//! CLI commands like `aggregate` can share it without duplicating the
//! ordering and threshold logic.

use serde::{Deserialize, Serialize};

/// Relevance level of a finding, ordered from highest to lowest.
///
/// Discriminants are inverted (`High = 0`, `None = 3`) so that the
/// derived [`Ord`] implementation sorts high-relevance findings first.
/// [`meets_threshold`](Relevance::meets_threshold) relies on this:
/// `(self as u8) <= (threshold as u8)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Relevance {
    /// No relevance to the query.
    None = 3,
    /// Low relevance.
    Low = 2,
    /// Medium relevance.
    Medium = 1,
    /// High relevance.
    High = 0,
}

impl Relevance {
    /// Parses a relevance string (case-insensitive).
    #[must_use]
    pub fn parse(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "high" => Self::High,
            "medium" => Self::Medium,
            "low" => Self::Low,
            _ => Self::None,
        }
    }

    /// Returns `true` if this relevance meets or exceeds the threshold.
    #[must_use]
    pub const fn meets_threshold(self, threshold: Self) -> bool {
        (self as u8) <= (threshold as u8)
    }

    /// Returns the string representation.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::High => "high",
            Self::Medium => "medium",
            Self::Low => "low",
            Self::None => "none",
        }
    }
}

impl std::fmt::Display for Relevance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_relevance_ordering() {
        assert!(Relevance::High < Relevance::Medium);
        assert!(Relevance::Medium < Relevance::Low);
        assert!(Relevance::Low < Relevance::None);
    }

    #[test]
    fn test_relevance_parse() {
        assert_eq!(Relevance::parse("high"), Relevance::High);
        assert_eq!(Relevance::parse("HIGH"), Relevance::High);
        assert_eq!(Relevance::parse("Medium"), Relevance::Medium);
        assert_eq!(Relevance::parse("low"), Relevance::Low);
        assert_eq!(Relevance::parse("unknown"), Relevance::None);
    }

    #[test]
    fn test_relevance_threshold() {
        assert!(Relevance::High.meets_threshold(Relevance::High));
        assert!(Relevance::High.meets_threshold(Relevance::Low));
        assert!(!Relevance::Low.meets_threshold(Relevance::High));
        assert!(Relevance::Medium.meets_threshold(Relevance::Medium));
    }

    #[test]
    fn test_relevance_display() {
        assert_eq!(format!("{}", Relevance::High), "high");
        assert_eq!(format!("{}", Relevance::None), "none");
    }
}
