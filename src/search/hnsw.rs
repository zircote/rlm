//! HNSW (Hierarchical Navigable Small World) vector index.
//!
//! Provides fast approximate nearest neighbor search using the usearch library.
//! Falls back to brute-force search when usearch is not available.

use crate::error::{Result, SearchError};
#[cfg(feature = "usearch-hnsw")]
use std::collections::HashMap;
use std::path::Path;

#[cfg(feature = "usearch-hnsw")]
use usearch::{Index, IndexOptions, MetricKind, ScalarKind};

/// HNSW vector index for scalable semantic search.
///
/// This wraps the usearch HNSW implementation, providing:
/// - O(log n) approximate nearest neighbor search
/// - Persistence to disk
/// - Incremental updates
///
/// When the `usearch-hnsw` feature is not enabled, operations return
/// appropriate errors, and callers should fall back to brute-force search.
pub struct HnswIndex {
    #[cfg(feature = "usearch-hnsw")]
    inner: Index,
    #[cfg(feature = "usearch-hnsw")]
    id_map: HashMap<u64, i64>, // usearch key -> chunk_id
    #[cfg(feature = "usearch-hnsw")]
    reverse_map: HashMap<i64, u64>, // chunk_id -> usearch key
    #[cfg(feature = "usearch-hnsw")]
    next_key: u64,
    dimensions: usize,
}

// Allow missing fields in Debug - inner (usearch::Index) doesn't implement Debug,
// and reverse_map is redundant with id_map_len.
#[allow(clippy::missing_fields_in_debug)]
impl std::fmt::Debug for HnswIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut debug_struct = f.debug_struct("HnswIndex");
        debug_struct.field("dimensions", &self.dimensions);
        #[cfg(feature = "usearch-hnsw")]
        {
            debug_struct.field("id_map_len", &self.id_map.len());
            debug_struct.field("next_key", &self.next_key);
        }
        debug_struct.finish()
    }
}

/// Configuration for HNSW index.
#[derive(Debug, Clone)]
pub struct HnswConfig {
    /// Number of dimensions in the vectors.
    pub dimensions: usize,
    /// M parameter (max connections per node, higher = more accurate but slower).
    pub connectivity: usize,
    /// `ef_construction` (search depth during build, higher = better quality index).
    pub expansion_add: usize,
    /// `ef_search` (search depth during query, higher = more accurate results).
    pub expansion_search: usize,
}

impl Default for HnswConfig {
    fn default() -> Self {
        Self {
            dimensions: 384, // Default for all-MiniLM-L6-v2
            connectivity: 16,
            expansion_add: 128,
            expansion_search: 64,
        }
    }
}

impl HnswConfig {
    /// Creates a new configuration with custom dimensions.
    #[must_use]
    pub const fn with_dimensions(dimensions: usize) -> Self {
        Self {
            dimensions,
            connectivity: 16,
            expansion_add: 128,
            expansion_search: 64,
        }
    }

    /// Sets the connectivity parameter (M).
    #[must_use]
    pub const fn connectivity(mut self, m: usize) -> Self {
        self.connectivity = m;
        self
    }

    /// Sets the expansion during add (`ef_construction`).
    #[must_use]
    pub const fn expansion_add(mut self, ef: usize) -> Self {
        self.expansion_add = ef;
        self
    }

    /// Sets the expansion during search (`ef_search`).
    #[must_use]
    pub const fn expansion_search(mut self, ef: usize) -> Self {
        self.expansion_search = ef;
        self
    }
}

/// Search result from HNSW index.
#[derive(Debug, Clone)]
pub struct HnswResult {
    /// Chunk ID.
    pub chunk_id: i64,
    /// Distance (lower is more similar for cosine).
    pub distance: f32,
    /// Similarity score (1 - distance for normalized vectors).
    pub similarity: f32,
}

impl HnswIndex {
    /// Creates a new HNSW index with the given configuration.
    ///
    /// # Errors
    ///
    /// Returns an error if the usearch feature is not enabled or index creation fails.
    #[cfg(feature = "usearch-hnsw")]
    pub fn new(config: &HnswConfig) -> Result<Self> {
        let options = IndexOptions {
            dimensions: config.dimensions,
            metric: MetricKind::Cos, // Cosine similarity
            quantization: ScalarKind::F32,
            connectivity: config.connectivity,
            expansion_add: config.expansion_add,
            expansion_search: config.expansion_search,
            multi: false,
        };

        let index = Index::new(&options).map_err(|e| SearchError::IndexError {
            message: format!("Failed to create HNSW index: {e}"),
        })?;

        Ok(Self {
            inner: index,
            id_map: HashMap::new(),
            reverse_map: HashMap::new(),
            next_key: 0,
            dimensions: config.dimensions,
        })
    }

    /// Creates a new HNSW index (feature not enabled).
    #[cfg(not(feature = "usearch-hnsw"))]
    pub fn new(config: &HnswConfig) -> Result<Self> {
        Ok(Self {
            dimensions: config.dimensions,
        })
    }

    /// Returns whether HNSW is available.
    #[must_use]
    pub const fn is_available() -> bool {
        cfg!(feature = "usearch-hnsw")
    }

    /// Returns the number of dimensions in the index.
    #[must_use]
    pub const fn dimensions(&self) -> usize {
        self.dimensions
    }

    /// Returns the number of vectors in the index.
    #[cfg(feature = "usearch-hnsw")]
    #[must_use]
    pub fn len(&self) -> usize {
        self.inner.size()
    }

    /// Returns the number of vectors in the index.
    #[cfg(not(feature = "usearch-hnsw"))]
    #[must_use]
    pub const fn len(&self) -> usize {
        0
    }

    /// Returns whether the index is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Adds a vector to the index.
    ///
    /// # Errors
    ///
    /// Returns an error if the vector dimensions don't match or insertion fails.
    #[cfg(feature = "usearch-hnsw")]
    pub fn add(&mut self, chunk_id: i64, vector: &[f32]) -> Result<()> {
        if vector.len() != self.dimensions {
            return Err(SearchError::DimensionMismatch {
                expected: self.dimensions,
                got: vector.len(),
            }
            .into());
        }

        // Check if already indexed
        if self.reverse_map.contains_key(&chunk_id) {
            // Remove old entry first
            self.remove(chunk_id)?;
        }

        let key = self.next_key;
        self.next_key += 1;

        self.inner
            .add(key, vector)
            .map_err(|e| SearchError::IndexError {
                message: format!("Failed to add vector: {e}"),
            })?;

        self.id_map.insert(key, chunk_id);
        self.reverse_map.insert(chunk_id, key);

        Ok(())
    }

    /// Adds a vector to the index (feature not enabled).
    #[cfg(not(feature = "usearch-hnsw"))]
    pub fn add(&mut self, _chunk_id: i64, _vector: &[f32]) -> Result<()> {
        Err(SearchError::FeatureNotEnabled {
            feature: "usearch-hnsw".to_string(),
        }
        .into())
    }

    /// Adds multiple vectors to the index in batch.
    ///
    /// # Errors
    ///
    /// Returns an error if any insertion fails.
    #[cfg(feature = "usearch-hnsw")]
    pub fn add_batch(&mut self, items: &[(i64, Vec<f32>)]) -> Result<usize> {
        let mut count = 0;
        for (chunk_id, vector) in items {
            self.add(*chunk_id, vector)?;
            count += 1;
        }
        Ok(count)
    }

    /// Adds multiple vectors to the index in batch (feature not enabled).
    #[cfg(not(feature = "usearch-hnsw"))]
    pub fn add_batch(&mut self, _items: &[(i64, Vec<f32>)]) -> Result<usize> {
        Err(SearchError::FeatureNotEnabled {
            feature: "usearch-hnsw".to_string(),
        }
        .into())
    }

    /// Removes a vector from the index.
    ///
    /// # Errors
    ///
    /// Returns an error if removal fails.
    #[cfg(feature = "usearch-hnsw")]
    pub fn remove(&mut self, chunk_id: i64) -> Result<bool> {
        if let Some(key) = self.reverse_map.remove(&chunk_id) {
            self.inner
                .remove(key)
                .map_err(|e| SearchError::IndexError {
                    message: format!("Failed to remove vector: {e}"),
                })?;
            self.id_map.remove(&key);
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Removes a vector from the index (feature not enabled).
    #[cfg(not(feature = "usearch-hnsw"))]
    pub fn remove(&mut self, _chunk_id: i64) -> Result<bool> {
        Err(SearchError::FeatureNotEnabled {
            feature: "usearch-hnsw".to_string(),
        }
        .into())
    }

    /// Searches for the k nearest neighbors.
    ///
    /// # Arguments
    ///
    /// * `query` - The query vector.
    /// * `k` - Maximum number of results.
    ///
    /// # Errors
    ///
    /// Returns an error if the search fails.
    #[cfg(feature = "usearch-hnsw")]
    pub fn search(&self, query: &[f32], k: usize) -> Result<Vec<HnswResult>> {
        if query.len() != self.dimensions {
            return Err(SearchError::DimensionMismatch {
                expected: self.dimensions,
                got: query.len(),
            }
            .into());
        }

        if self.is_empty() {
            return Ok(Vec::new());
        }

        let results = self
            .inner
            .search(query, k)
            .map_err(|e| SearchError::IndexError {
                message: format!("Search failed: {e}"),
            })?;

        let mut output = Vec::with_capacity(results.keys.len());
        for (key, distance) in results.keys.iter().zip(results.distances.iter()) {
            if let Some(&chunk_id) = self.id_map.get(key) {
                output.push(HnswResult {
                    chunk_id,
                    distance: *distance,
                    // For cosine metric, similarity = 1 - distance (for normalized vectors)
                    similarity: 1.0 - distance,
                });
            }
        }

        Ok(output)
    }

    /// Searches for the k nearest neighbors (feature not enabled).
    #[cfg(not(feature = "usearch-hnsw"))]
    pub fn search(&self, _query: &[f32], _k: usize) -> Result<Vec<HnswResult>> {
        Err(SearchError::FeatureNotEnabled {
            feature: "usearch-hnsw".to_string(),
        }
        .into())
    }

    /// Saves the index to a file.
    ///
    /// # Errors
    ///
    /// Returns an error if saving fails.
    #[cfg(feature = "usearch-hnsw")]
    pub fn save(&self, path: &Path) -> Result<()> {
        let path_str = path.to_str().ok_or_else(|| SearchError::IndexError {
            message: "Invalid path: non-UTF8 characters".to_string(),
        })?;
        self.inner
            .save(path_str)
            .map_err(|e| SearchError::IndexError {
                message: format!("Failed to save index: {e}"),
            })?;

        // Save the ID mappings alongside the index
        let map_path = path.with_extension("map");
        let map_data = serde_json::json!({
            "id_map": self.id_map.iter().map(|(k, v)| (k.to_string(), v)).collect::<HashMap<_, _>>(),
            "next_key": self.next_key,
            "dimensions": self.dimensions,
        });
        std::fs::write(
            &map_path,
            serde_json::to_string(&map_data).unwrap_or_default(),
        )
        .map_err(|e| SearchError::IndexError {
            message: format!("Failed to save ID map: {e}"),
        })?;

        Ok(())
    }

    /// Saves the index to a file (feature not enabled).
    #[cfg(not(feature = "usearch-hnsw"))]
    pub fn save(&self, _path: &Path) -> Result<()> {
        Err(SearchError::FeatureNotEnabled {
            feature: "usearch-hnsw".to_string(),
        }
        .into())
    }

    /// Loads an index from a file.
    ///
    /// # Errors
    ///
    /// Returns an error if loading fails.
    #[cfg(feature = "usearch-hnsw")]
    pub fn load(path: &Path, config: &HnswConfig) -> Result<Self> {
        let options = IndexOptions {
            dimensions: config.dimensions,
            metric: MetricKind::Cos,
            quantization: ScalarKind::F32,
            connectivity: config.connectivity,
            expansion_add: config.expansion_add,
            expansion_search: config.expansion_search,
            multi: false,
        };

        let index = Index::new(&options).map_err(|e| SearchError::IndexError {
            message: format!("Failed to create index for loading: {e}"),
        })?;

        let path_str = path.to_str().ok_or_else(|| SearchError::IndexError {
            message: "Invalid path: non-UTF8 characters".to_string(),
        })?;
        index.load(path_str).map_err(|e| SearchError::IndexError {
            message: format!("Failed to load index: {e}"),
        })?;

        // Load the ID mappings
        let map_path = path.with_extension("map");
        let (id_map, reverse_map, next_key, dimensions) = if map_path.exists() {
            let map_str =
                std::fs::read_to_string(&map_path).map_err(|e| SearchError::IndexError {
                    message: format!("Failed to read ID map: {e}"),
                })?;
            let map_data: serde_json::Value =
                serde_json::from_str(&map_str).map_err(|e| SearchError::IndexError {
                    message: format!("Failed to parse ID map: {e}"),
                })?;

            let mut id_map = HashMap::new();
            let mut reverse_map = HashMap::new();
            if let Some(obj) = map_data.get("id_map").and_then(|v| v.as_object()) {
                for (k, v) in obj {
                    if let (Ok(key), Some(val)) = (k.parse::<u64>(), v.as_i64()) {
                        id_map.insert(key, val);
                        reverse_map.insert(val, key);
                    }
                }
            }
            let next_key = map_data
                .get("next_key")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or(0);
            let dimensions = map_data
                .get("dimensions")
                .and_then(serde_json::Value::as_u64)
                .map_or(config.dimensions, |d| {
                    usize::try_from(d).unwrap_or(config.dimensions)
                });

            (id_map, reverse_map, next_key, dimensions)
        } else {
            (HashMap::new(), HashMap::new(), 0, config.dimensions)
        };

        Ok(Self {
            inner: index,
            id_map,
            reverse_map,
            next_key,
            dimensions,
        })
    }

    /// Loads an index from a file (feature not enabled).
    #[cfg(not(feature = "usearch-hnsw"))]
    pub fn load(_path: &Path, config: &HnswConfig) -> Result<Self> {
        Self::new(config)
    }

    /// Clears all vectors from the index.
    ///
    /// # Errors
    ///
    /// Returns an error if the reset fails.
    #[cfg(feature = "usearch-hnsw")]
    pub fn clear(&mut self) -> Result<()> {
        self.inner.reset().map_err(|e| SearchError::IndexError {
            message: format!("Failed to reset index: {e}"),
        })?;
        self.id_map.clear();
        self.reverse_map.clear();
        self.next_key = 0;
        Ok(())
    }

    /// Clears all vectors from the index.
    ///
    /// # Errors
    ///
    /// Returns an error if the reset fails.
    #[cfg(not(feature = "usearch-hnsw"))]
    pub fn clear(&mut self) -> Result<()> {
        // No-op when feature not enabled
        Ok(())
    }

    /// Checks if a chunk is indexed.
    #[cfg(feature = "usearch-hnsw")]
    #[must_use]
    pub fn contains(&self, chunk_id: i64) -> bool {
        self.reverse_map.contains_key(&chunk_id)
    }

    /// Checks if a chunk is indexed.
    #[cfg(not(feature = "usearch-hnsw"))]
    #[must_use]
    pub const fn contains(&self, _chunk_id: i64) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hnsw_config_default() {
        let config = HnswConfig::default();
        assert_eq!(config.dimensions, 384);
        assert_eq!(config.connectivity, 16);
        assert_eq!(config.expansion_add, 128);
        assert_eq!(config.expansion_search, 64);
    }

    #[test]
    fn test_hnsw_config_builder() {
        let config = HnswConfig::with_dimensions(256)
            .connectivity(32)
            .expansion_add(256)
            .expansion_search(128);

        assert_eq!(config.dimensions, 256);
        assert_eq!(config.connectivity, 32);
        assert_eq!(config.expansion_add, 256);
        assert_eq!(config.expansion_search, 128);
    }

    #[test]
    fn test_hnsw_is_available() {
        // This tests the compile-time feature detection
        let available = HnswIndex::is_available();
        // The result depends on whether the feature is enabled
        #[cfg(feature = "usearch-hnsw")]
        assert!(available);
        #[cfg(not(feature = "usearch-hnsw"))]
        assert!(!available);
    }

    #[test]
    #[cfg(not(feature = "usearch-hnsw"))]
    fn test_hnsw_new() {
        let config = HnswConfig::with_dimensions(128);
        let result = HnswIndex::new(&config);
        assert!(result.is_ok());
        let index = result.unwrap();
        assert_eq!(index.dimensions(), 128);
    }

    #[test]
    #[cfg(feature = "usearch-hnsw")]
    fn test_hnsw_new_usearch() {
        let config = HnswConfig::with_dimensions(128);
        let result = HnswIndex::new(&config);
        assert!(result.is_ok());
        let index = result.unwrap();
        assert!(index.is_empty());
        assert_eq!(index.dimensions(), 128);
    }

    #[cfg(feature = "usearch-hnsw")]
    mod usearch_tests {
        use super::*;

        #[test]
        fn test_hnsw_add_and_search() {
            let config = HnswConfig::with_dimensions(4);
            let mut index = HnswIndex::new(&config).unwrap();

            // Add some vectors
            index.add(1, &[1.0, 0.0, 0.0, 0.0]).unwrap();
            index.add(2, &[0.0, 1.0, 0.0, 0.0]).unwrap();
            index.add(3, &[0.0, 0.0, 1.0, 0.0]).unwrap();

            assert_eq!(index.len(), 3);
            assert!(index.contains(1));
            assert!(index.contains(2));
            assert!(index.contains(3));
            assert!(!index.contains(4));

            // Search for vector similar to [1, 0, 0, 0]
            let results = index.search(&[1.0, 0.0, 0.0, 0.0], 2).unwrap();
            assert!(!results.is_empty());
            assert_eq!(results[0].chunk_id, 1);
        }

        #[test]
        fn test_hnsw_remove() {
            let config = HnswConfig::with_dimensions(4);
            let mut index = HnswIndex::new(&config).unwrap();

            index.add(1, &[1.0, 0.0, 0.0, 0.0]).unwrap();
            index.add(2, &[0.0, 1.0, 0.0, 0.0]).unwrap();

            assert_eq!(index.len(), 2);

            let removed = index.remove(1).unwrap();
            assert!(removed);
            assert_eq!(index.len(), 1);
            assert!(!index.contains(1));

            let removed_again = index.remove(1).unwrap();
            assert!(!removed_again);
        }

        #[test]
        fn test_hnsw_dimension_mismatch() {
            let config = HnswConfig::with_dimensions(4);
            let mut index = HnswIndex::new(&config).unwrap();

            // Try to add wrong dimensions
            let result = index.add(1, &[1.0, 0.0]); // Only 2 dimensions
            assert!(result.is_err());
        }

        #[test]
        fn test_hnsw_clear() {
            let config = HnswConfig::with_dimensions(4);
            let mut index = HnswIndex::new(&config).unwrap();

            index.add(1, &[1.0, 0.0, 0.0, 0.0]).unwrap();
            index.add(2, &[0.0, 1.0, 0.0, 0.0]).unwrap();
            assert_eq!(index.len(), 2);

            index.clear().unwrap();
            assert!(index.is_empty());
        }

        #[test]
        fn test_hnsw_update_existing() {
            let config = HnswConfig::with_dimensions(4);
            let mut index = HnswIndex::new(&config).unwrap();

            // Add initial vector
            index.add(1, &[1.0, 0.0, 0.0, 0.0]).unwrap();

            // Update with new vector
            index.add(1, &[0.0, 1.0, 0.0, 0.0]).unwrap();

            // Should still only have 1 entry
            assert_eq!(index.len(), 1);

            // Search should find the updated vector
            let results = index.search(&[0.0, 1.0, 0.0, 0.0], 1).unwrap();
            assert_eq!(results[0].chunk_id, 1);
        }

        #[test]
        fn test_hnsw_save_load() {
            use tempfile::TempDir;

            let temp_dir = TempDir::new().unwrap();
            let index_path = temp_dir.path().join("test.index");

            let config = HnswConfig::with_dimensions(4);

            // Create and populate index
            {
                let mut index = HnswIndex::new(&config).unwrap();
                index.add(1, &[1.0, 0.0, 0.0, 0.0]).unwrap();
                index.add(2, &[0.0, 1.0, 0.0, 0.0]).unwrap();
                index.save(&index_path).unwrap();
            }

            // Load and verify
            {
                let index = HnswIndex::load(&index_path, &config).unwrap();
                assert_eq!(index.len(), 2);
                assert!(index.contains(1));
                assert!(index.contains(2));
            }
        }
    }
}
