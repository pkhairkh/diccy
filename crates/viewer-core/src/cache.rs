//! Deterministic cache utilities with stable LRU eviction.

use std::collections::BTreeMap;

/// Cache metrics snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CacheMetrics {
    /// Cache hits.
    pub hits: u64,
    /// Cache misses.
    pub misses: u64,
    /// Evicted entries.
    pub evictions: u64,
    /// Current cached bytes.
    pub bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Entry<V> {
    value: V,
    size_bytes: u64,
    pinned: bool,
    last_touch: u64,
}

/// Deterministic cache with stable LRU eviction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeterministicCache<V> {
    max_bytes: u64,
    used_bytes: u64,
    touch_seq: u64,
    entries: BTreeMap<String, Entry<V>>,
    metrics: CacheMetrics,
}

impl<V> DeterministicCache<V> {
    /// Create a cache with a max byte budget.
    pub fn new(max_bytes: u64) -> Self {
        Self {
            max_bytes,
            used_bytes: 0,
            touch_seq: 0,
            entries: BTreeMap::new(),
            metrics: CacheMetrics::default(),
        }
    }

    /// Return the configured max bytes.
    pub fn max_bytes(&self) -> u64 {
        self.max_bytes
    }

    /// Return used bytes.
    pub fn used_bytes(&self) -> u64 {
        self.used_bytes
    }

    /// Return true when cache has no entries.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Return immutable metrics.
    pub fn metrics(&self) -> CacheMetrics {
        let mut metrics = self.metrics;
        metrics.bytes = self.used_bytes;
        metrics
    }

    /// Return number of entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Pin an entry, protecting it from eviction.
    pub fn pin(&mut self, key: &str) -> bool {
        if let Some(entry) = self.entries.get_mut(key) {
            entry.pinned = true;
            return true;
        }
        false
    }

    /// Unpin an entry.
    pub fn unpin(&mut self, key: &str) -> bool {
        if let Some(entry) = self.entries.get_mut(key) {
            entry.pinned = false;
            return true;
        }
        false
    }

    /// Insert or replace an entry and evict deterministically until under budget.
    pub fn insert(&mut self, key: String, value: V, size_bytes: u64) {
        self.touch_seq = self.touch_seq.saturating_add(1);
        if let Some(previous) = self.entries.remove(&key) {
            self.used_bytes = self.used_bytes.saturating_sub(previous.size_bytes);
        }
        self.used_bytes = self.used_bytes.saturating_add(size_bytes);
        self.entries.insert(
            key,
            Entry {
                value,
                size_bytes,
                pinned: false,
                last_touch: self.touch_seq,
            },
        );
        self.evict_to_budget();
    }

    /// Borrow a cached value and update LRU touch order.
    pub fn get(&mut self, key: &str) -> Option<&V> {
        let entry = self.entries.get_mut(key);
        match entry {
            Some(entry) => {
                self.metrics.hits = self.metrics.hits.saturating_add(1);
                self.touch_seq = self.touch_seq.saturating_add(1);
                entry.last_touch = self.touch_seq;
                Some(&entry.value)
            }
            None => {
                self.metrics.misses = self.metrics.misses.saturating_add(1);
                None
            }
        }
    }

    /// Remove a cache entry.
    pub fn remove(&mut self, key: &str) -> Option<V> {
        let removed = self.entries.remove(key)?;
        self.used_bytes = self.used_bytes.saturating_sub(removed.size_bytes);
        Some(removed.value)
    }

    fn evict_to_budget(&mut self) {
        while self.used_bytes > self.max_bytes {
            let eviction_key = self
                .entries
                .iter()
                .filter(|(_, entry)| !entry.pinned)
                .min_by(|(lhs_key, lhs), (rhs_key, rhs)| {
                    lhs.last_touch
                        .cmp(&rhs.last_touch)
                        .then(lhs_key.cmp(rhs_key))
                })
                .map(|(key, _)| key.clone());
            let Some(key) = eviction_key else {
                break;
            };
            if let Some(entry) = self.entries.remove(&key) {
                self.used_bytes = self.used_bytes.saturating_sub(entry.size_bytes);
                self.metrics.evictions = self.metrics.evictions.saturating_add(1);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::DeterministicCache;

    #[test]
    fn stable_lru_evicts_oldest_touch_then_key() {
        let mut cache = DeterministicCache::new(10);
        cache.insert("b".to_string(), 2u8, 5);
        cache.insert("a".to_string(), 1u8, 5);
        cache.insert("c".to_string(), 3u8, 5);
        assert!(cache.get("a").is_some());
        assert!(cache.get("b").is_none());
        assert!(cache.get("c").is_some());
    }

    #[test]
    fn pinned_entries_are_not_evicted() {
        let mut cache = DeterministicCache::new(8);
        cache.insert("keep".to_string(), 1u8, 6);
        cache.pin("keep");
        cache.insert("drop".to_string(), 2u8, 6);
        assert!(cache.get("keep").is_some());
        assert!(cache.get("drop").is_none());
    }
}
