//! Entry filtering and iteration helpers.
//!
//! Replaces index-heavy loops with cleaner, more abstract methods for:
//! - Iterating visible entries
//! - Iterating marked entries
#![allow(dead_code)]
//! - Getting the selected entry
//! - Computing summary stats over marked entries

// This module will be expanded as App entries structure is understood better.
// For now, it provides the interface that will be used to abstract index-based logic.

/// Iterator result over filtered entries.
///
/// Used to abstract whether we're iterating marked entries, visible entries, or all entries.
pub struct FilteredEntryIter {
    /// List of indices to iterate
    pub indices: Vec<usize>,
}

impl FilteredEntryIter {
    /// Create a new filtered iterator.
    pub fn new(indices: Vec<usize>) -> Self {
        Self { indices }
    }

    /// Get count of entries in this filtered set.
    pub fn count(&self) -> usize {
        self.indices.len()
    }

    /// Get total size of all entries in this set.
    /// (To be called with metadata from the app)
    pub fn total_size(&self, size_fn: impl Fn(usize) -> u64) -> u64 {
        self.indices.iter().map(|&idx| size_fn(idx)).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filtered_entry_iter_count() {
        let iter = FilteredEntryIter::new(vec![0, 2, 5, 9]);
        assert_eq!(iter.count(), 4);
    }

    #[test]
    fn test_filtered_entry_iter_total_size() {
        let iter = FilteredEntryIter::new(vec![0, 1, 2]);
        let sizes = vec![100, 200, 300, 400];
        let total = iter.total_size(|idx| {
            sizes.get(idx).copied().unwrap_or(0)
        });
        assert_eq!(total, 600); // 100 + 200 + 300
    }
}
