// Auto-extracted from /home/z/diccy/crates/viewer-core/src/cache.rs
// S13-T8: Move inline tests to tests/ directories


    use viewer_core::DeterministicCache;

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
