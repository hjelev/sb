//! Entry filtering and iteration helpers.
//!
//! Provides a small, tested abstraction for working with a filtered set of
//! entry indices (counting them and summing a size function over them).
//!
//! Intentionally allowed to be dead code: this is a deliberately-kept,
//! unit-tested utility that does not yet have a call site. The allow is
//! scoped to this file so the rest of the crate still gets `dead_code`
//! enforcement (which is what surfaces abandoned helpers like this one).
#![allow(dead_code)]

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
        let sizes = [100, 200, 300, 400];
        let total = iter.total_size(|idx| {
            sizes.get(idx).copied().unwrap_or(0)
        });
        assert_eq!(total, 600); // 100 + 200 + 300
    }
}
