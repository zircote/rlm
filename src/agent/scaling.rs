//! Adaptive scaling for the agentic pipeline.
//!
//! Computes batch size, concurrency, and search depth based on dataset
//! characteristics. The [`ScalingProfile`] is a pure function of
//! [`DatasetProfile`], making it deterministic and easy to test.
//!
//! # Resolution Chain
//!
//! Parameters are resolved in priority order:
//! **CLI → Plan → Scaling → Config → Default**
//!
//! The scaling profile fills in parameters that neither the CLI nor the
//! LLM plan specified, adapting to the actual data size rather than
//! falling back to static config defaults.

/// Characteristics of the dataset being queried.
#[derive(Debug, Clone, Copy)]
pub struct DatasetProfile {
    /// Total number of chunks across all buffers in scope.
    pub chunk_count: usize,
    /// Total content size in bytes across all chunks in scope.
    pub total_bytes: usize,
}

/// Scaling recommendations computed from [`DatasetProfile`].
///
/// Each field is `Some` only when the scaling logic recommends a value.
/// `None` means "defer to the next level in the resolution chain."
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScalingProfile {
    /// Recommended scaling tier.
    pub tier: ScalingTier,
    /// Recommended chunks per subcall batch.
    pub batch_size: Option<usize>,
    /// Maximum concurrent API requests.
    pub max_concurrency: Option<usize>,
    /// Search depth (top-k results).
    pub top_k: Option<usize>,
    /// Maximum chunks to load for analysis.
    pub max_chunks: Option<usize>,
}

/// Size-based tier classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ScalingTier {
    /// <20 chunks — analyze everything, minimal parallelism.
    Tiny,
    /// 20–100 chunks — light parallelism.
    Small,
    /// 100–500 chunks — moderate parallelism and scoping.
    Medium,
    /// 500–2000 chunks — high parallelism, aggressive scoping.
    Large,
    /// >2000 chunks — maximum parallelism, tight scoping.
    XLarge,
}

impl std::fmt::Display for ScalingTier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Tiny => write!(f, "tiny"),
            Self::Small => write!(f, "small"),
            Self::Medium => write!(f, "medium"),
            Self::Large => write!(f, "large"),
            Self::XLarge => write!(f, "xlarge"),
        }
    }
}

/// Computes a [`ScalingProfile`] from the dataset characteristics.
///
/// This is a pure function — no I/O, no config reads, fully deterministic.
///
/// # Tier Boundaries
///
/// | Tier    | Chunks    | Batch | Concurrency | Top-K | Max Chunks |
/// |---------|-----------|-------|-------------|-------|------------|
/// | Tiny    | <20       | 1*    | 5           | all†  | none       |
/// | Small   | 20–99     | 5     | 15          | 100   | none       |
/// | Medium  | 100–499   | 10    | 30          | 200   | 100        |
/// | Large   | 500–1999  | 20    | 60          | 400   | 200        |
/// | `XLarge` | 2000+     | 50    | 100         | 500   | 300        |
///
/// *Tiny uses `batch_size=1` to give each chunk its own agent for maximum
/// extraction quality on small datasets.
///
/// †Tiny returns `top_k: None` and `max_chunks: None` to indicate
/// "use all available" (no scoping).
#[must_use]
pub const fn compute_scaling_profile(dataset: &DatasetProfile) -> ScalingProfile {
    let n = dataset.chunk_count;

    if n < 20 {
        ScalingProfile {
            tier: ScalingTier::Tiny,
            batch_size: Some(1),
            max_concurrency: Some(5),
            top_k: None,
            max_chunks: None,
        }
    } else if n < 100 {
        ScalingProfile {
            tier: ScalingTier::Small,
            batch_size: Some(5),
            max_concurrency: Some(15),
            top_k: Some(100),
            max_chunks: None,
        }
    } else if n < 500 {
        ScalingProfile {
            tier: ScalingTier::Medium,
            batch_size: Some(10),
            max_concurrency: Some(30),
            top_k: Some(200),
            max_chunks: Some(100),
        }
    } else if n < 2000 {
        ScalingProfile {
            tier: ScalingTier::Large,
            batch_size: Some(20),
            max_concurrency: Some(60),
            top_k: Some(400),
            max_chunks: Some(200),
        }
    } else {
        ScalingProfile {
            tier: ScalingTier::XLarge,
            batch_size: Some(50),
            max_concurrency: Some(100),
            top_k: Some(500),
            max_chunks: Some(300),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tiny_dataset() {
        let profile = compute_scaling_profile(&DatasetProfile {
            chunk_count: 5,
            total_bytes: 15_000,
        });
        assert_eq!(profile.tier, ScalingTier::Tiny);
        assert_eq!(profile.batch_size, Some(1));
        assert_eq!(profile.max_concurrency, Some(5));
        assert!(profile.top_k.is_none());
        assert!(profile.max_chunks.is_none());
    }

    #[test]
    fn test_small_dataset() {
        let profile = compute_scaling_profile(&DatasetProfile {
            chunk_count: 50,
            total_bytes: 150_000,
        });
        assert_eq!(profile.tier, ScalingTier::Small);
        assert_eq!(profile.batch_size, Some(5));
        assert_eq!(profile.max_concurrency, Some(15));
        assert_eq!(profile.top_k, Some(100));
        assert!(profile.max_chunks.is_none());
    }

    #[test]
    fn test_medium_dataset() {
        let profile = compute_scaling_profile(&DatasetProfile {
            chunk_count: 250,
            total_bytes: 750_000,
        });
        assert_eq!(profile.tier, ScalingTier::Medium);
        assert_eq!(profile.batch_size, Some(10));
        assert_eq!(profile.max_concurrency, Some(30));
        assert_eq!(profile.top_k, Some(200));
        assert_eq!(profile.max_chunks, Some(100));
    }

    #[test]
    fn test_large_dataset() {
        let profile = compute_scaling_profile(&DatasetProfile {
            chunk_count: 1000,
            total_bytes: 3_000_000,
        });
        assert_eq!(profile.tier, ScalingTier::Large);
        assert_eq!(profile.batch_size, Some(20));
        assert_eq!(profile.max_concurrency, Some(60));
        assert_eq!(profile.top_k, Some(400));
        assert_eq!(profile.max_chunks, Some(200));
    }

    #[test]
    fn test_xlarge_dataset() {
        let profile = compute_scaling_profile(&DatasetProfile {
            chunk_count: 5000,
            total_bytes: 100_000_000,
        });
        assert_eq!(profile.tier, ScalingTier::XLarge);
        assert_eq!(profile.batch_size, Some(50));
        assert_eq!(profile.max_concurrency, Some(100));
        assert_eq!(profile.top_k, Some(500));
        assert_eq!(profile.max_chunks, Some(300));
    }

    #[test]
    fn test_boundary_19_is_tiny() {
        let profile = compute_scaling_profile(&DatasetProfile {
            chunk_count: 19,
            total_bytes: 57_000,
        });
        assert_eq!(profile.tier, ScalingTier::Tiny);
    }

    #[test]
    fn test_boundary_20_is_small() {
        let profile = compute_scaling_profile(&DatasetProfile {
            chunk_count: 20,
            total_bytes: 60_000,
        });
        assert_eq!(profile.tier, ScalingTier::Small);
    }

    #[test]
    fn test_boundary_100_is_medium() {
        let profile = compute_scaling_profile(&DatasetProfile {
            chunk_count: 100,
            total_bytes: 300_000,
        });
        assert_eq!(profile.tier, ScalingTier::Medium);
    }

    #[test]
    fn test_boundary_500_is_large() {
        let profile = compute_scaling_profile(&DatasetProfile {
            chunk_count: 500,
            total_bytes: 1_500_000,
        });
        assert_eq!(profile.tier, ScalingTier::Large);
    }

    #[test]
    fn test_boundary_2000_is_xlarge() {
        let profile = compute_scaling_profile(&DatasetProfile {
            chunk_count: 2000,
            total_bytes: 6_000_000,
        });
        assert_eq!(profile.tier, ScalingTier::XLarge);
    }

    #[test]
    fn test_zero_chunks() {
        let profile = compute_scaling_profile(&DatasetProfile {
            chunk_count: 0,
            total_bytes: 0,
        });
        assert_eq!(profile.tier, ScalingTier::Tiny);
    }

    #[test]
    fn test_tier_ordering() {
        assert!(ScalingTier::Tiny < ScalingTier::Small);
        assert!(ScalingTier::Small < ScalingTier::Medium);
        assert!(ScalingTier::Medium < ScalingTier::Large);
        assert!(ScalingTier::Large < ScalingTier::XLarge);
    }

    #[test]
    fn test_tier_display() {
        assert_eq!(ScalingTier::Tiny.to_string(), "tiny");
        assert_eq!(ScalingTier::Small.to_string(), "small");
        assert_eq!(ScalingTier::Medium.to_string(), "medium");
        assert_eq!(ScalingTier::Large.to_string(), "large");
        assert_eq!(ScalingTier::XLarge.to_string(), "xlarge");
    }
}
