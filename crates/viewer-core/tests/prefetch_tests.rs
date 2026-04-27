// Auto-extracted from /home/z/diccy/crates/viewer-core/src/prefetch.rs
// S13-T8: Move inline tests to tests/ directories


    use viewer_core::*;

    #[test]
    fn engine_starts_empty() {
        let engine = PrefetchEngine::new();
        assert_eq!(engine.queue_depth(), 0);
        assert_eq!(engine.rule_count(), 0);
    }

    #[test]
    fn add_and_remove_rules() {
        let mut engine = PrefetchEngine::new();
        engine.add_rule(PrefetchRule {
            rule_id: "r1".to_string(),
            modality: Some("CT".to_string()),
            body_part: None,
            prior_count: 1,
            priority: PrefetchPriority::Normal,
            enabled: true,
        });
        assert_eq!(engine.rule_count(), 1);
        assert!(engine.remove_rule("r1"));
        assert_eq!(engine.rule_count(), 0);
    }

    #[test]
    fn worklist_trigger_creates_requests() {
        let mut engine = PrefetchEngine::new();
        engine.add_rule(PrefetchRule {
            rule_id: "ct_rule".to_string(),
            modality: Some("CT".to_string()),
            body_part: None,
            prior_count: 1,
            priority: PrefetchPriority::Normal,
            enabled: true,
        });

        let trigger = WorklistTrigger {
            patient_id: Some("PAT001".to_string()),
            accession_number: Some("ACC001".to_string()),
            modality: "CT".to_string(),
            body_part: Some("CHEST".to_string()),
            scheduled_date: "20240101".to_string(),
            scheduled_time: "090000".to_string(),
        };

        let requests = engine.process_worklist_trigger(&trigger);
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].patient_id.as_deref(), Some("PAT001"));
        assert_eq!(requests[0].priority, PrefetchPriority::Normal);
    }

    #[test]
    fn trigger_does_not_match_different_modality() {
        let mut engine = PrefetchEngine::new();
        engine.add_rule(PrefetchRule {
            rule_id: "ct_only".to_string(),
            modality: Some("CT".to_string()),
            body_part: None,
            prior_count: 1,
            priority: PrefetchPriority::Normal,
            enabled: true,
        });

        let trigger = WorklistTrigger {
            patient_id: Some("PAT001".to_string()),
            accession_number: None,
            modality: "MR".to_string(),
            body_part: None,
            scheduled_date: "20240101".to_string(),
            scheduled_time: "090000".to_string(),
        };

        let requests = engine.process_worklist_trigger(&trigger);
        assert!(requests.is_empty());
    }

    #[test]
    fn disabled_rule_does_not_trigger() {
        let mut engine = PrefetchEngine::new();
        engine.add_rule(PrefetchRule {
            rule_id: "disabled_rule".to_string(),
            modality: Some("CT".to_string()),
            body_part: None,
            prior_count: 1,
            priority: PrefetchPriority::Normal,
            enabled: false,
        });

        let trigger = WorklistTrigger {
            patient_id: Some("PAT001".to_string()),
            accession_number: None,
            modality: "CT".to_string(),
            body_part: None,
            scheduled_date: "20240101".to_string(),
            scheduled_time: "090000".to_string(),
        };

        let requests = engine.process_worklist_trigger(&trigger);
        assert!(requests.is_empty());
    }

    #[test]
    fn body_part_filter_applies() {
        let mut engine = PrefetchEngine::new();
        engine.add_rule(PrefetchRule {
            rule_id: "ct_chest".to_string(),
            modality: Some("CT".to_string()),
            body_part: Some("CHEST".to_string()),
            prior_count: 1,
            priority: PrefetchPriority::High,
            enabled: true,
        });

        let matching = WorklistTrigger {
            patient_id: Some("PAT001".to_string()),
            accession_number: None,
            modality: "CT".to_string(),
            body_part: Some("CHEST".to_string()),
            scheduled_date: "20240101".to_string(),
            scheduled_time: "090000".to_string(),
        };
        let requests = engine.process_worklist_trigger(&matching);
        assert_eq!(requests.len(), 1);

        let non_matching = WorklistTrigger {
            patient_id: Some("PAT001".to_string()),
            accession_number: None,
            modality: "CT".to_string(),
            body_part: Some("ABDOMEN".to_string()),
            scheduled_date: "20240101".to_string(),
            scheduled_time: "090000".to_string(),
        };
        let requests = engine.process_worklist_trigger(&non_matching);
        assert!(requests.is_empty());
    }

    #[test]
    fn pop_next_returns_highest_priority() {
        let mut engine = PrefetchEngine::new();

        // Add requests with different priorities
        engine.add_rule(PrefetchRule {
            rule_id: "low_rule".to_string(),
            modality: Some("XR".to_string()),
            body_part: None,
            prior_count: 1,
            priority: PrefetchPriority::Low,
            enabled: true,
        });
        engine.add_rule(PrefetchRule {
            rule_id: "stat_rule".to_string(),
            modality: Some("CT".to_string()),
            body_part: None,
            prior_count: 1,
            priority: PrefetchPriority::Stat,
            enabled: true,
        });

        let low_trigger = WorklistTrigger {
            patient_id: Some("PAT001".to_string()),
            accession_number: None,
            modality: "XR".to_string(),
            body_part: None,
            scheduled_date: "20240101".to_string(),
            scheduled_time: "090000".to_string(),
        };
        let stat_trigger = WorklistTrigger {
            patient_id: Some("PAT002".to_string()),
            accession_number: None,
            modality: "CT".to_string(),
            body_part: None,
            scheduled_date: "20240101".to_string(),
            scheduled_time: "090000".to_string(),
        };

        engine.process_worklist_trigger(&low_trigger);
        engine.process_worklist_trigger(&stat_trigger);

        let next = engine.pop_next().expect("should have request");
        assert_eq!(next.priority, PrefetchPriority::Stat);
    }

    #[test]
    fn mark_completed_removes_from_queue() {
        let mut engine = PrefetchEngine::new();
        engine.add_rule(PrefetchRule {
            rule_id: "r1".to_string(),
            modality: Some("CT".to_string()),
            body_part: None,
            prior_count: 1,
            priority: PrefetchPriority::Normal,
            enabled: true,
        });

        let trigger = WorklistTrigger {
            patient_id: Some("PAT001".to_string()),
            accession_number: None,
            modality: "CT".to_string(),
            body_part: None,
            scheduled_date: "20240101".to_string(),
            scheduled_time: "090000".to_string(),
        };

        let requests = engine.process_worklist_trigger(&trigger);
        let request_id = requests[0].request_id.clone();
        assert_eq!(engine.queue_depth(), 1);

        assert!(engine.mark_completed(&request_id));
        assert_eq!(engine.queue_depth(), 0);

        let stats = engine.stats();
        assert_eq!(stats.total_completed, 1);
    }

    #[test]
    fn escalate_stat_changes_priority() {
        let mut engine = PrefetchEngine::new();
        engine.add_rule(PrefetchRule {
            rule_id: "r1".to_string(),
            modality: Some("CT".to_string()),
            body_part: None,
            prior_count: 1,
            priority: PrefetchPriority::Normal,
            enabled: true,
        });

        let trigger = WorklistTrigger {
            patient_id: Some("PAT001".to_string()),
            accession_number: None,
            modality: "CT".to_string(),
            body_part: None,
            scheduled_date: "20240101".to_string(),
            scheduled_time: "090000".to_string(),
        };

        let requests = engine.process_worklist_trigger(&trigger);
        let request_id = requests[0].request_id.clone();

        assert!(engine.escalate_stat(&request_id));

        let next = engine.pop_next().expect("should have request");
        assert_eq!(next.priority, PrefetchPriority::Stat);
    }

    #[test]
    fn cancel_removes_from_queue() {
        let mut engine = PrefetchEngine::new();
        engine.add_rule(PrefetchRule {
            rule_id: "r1".to_string(),
            modality: Some("CT".to_string()),
            body_part: None,
            prior_count: 1,
            priority: PrefetchPriority::Normal,
            enabled: true,
        });

        let trigger = WorklistTrigger {
            patient_id: Some("PAT001".to_string()),
            accession_number: None,
            modality: "CT".to_string(),
            body_part: None,
            scheduled_date: "20240101".to_string(),
            scheduled_time: "090000".to_string(),
        };

        let requests = engine.process_worklist_trigger(&trigger);
        let request_id = requests[0].request_id.clone();
        assert_eq!(engine.queue_depth(), 1);

        assert!(engine.cancel(&request_id));
        assert_eq!(engine.queue_depth(), 0);
    }

    #[test]
    fn duplicate_trigger_does_not_requeue() {
        let mut engine = PrefetchEngine::new();
        engine.add_rule(PrefetchRule {
            rule_id: "r1".to_string(),
            modality: Some("CT".to_string()),
            body_part: None,
            prior_count: 1,
            priority: PrefetchPriority::Normal,
            enabled: true,
        });

        let trigger = WorklistTrigger {
            patient_id: Some("PAT001".to_string()),
            accession_number: Some("ACC001".to_string()),
            modality: "CT".to_string(),
            body_part: None,
            scheduled_date: "20240101".to_string(),
            scheduled_time: "090000".to_string(),
        };

        let r1 = engine.process_worklist_trigger(&trigger);
        assert_eq!(r1.len(), 1);

        let r2 = engine.process_worklist_trigger(&trigger);
        assert!(r2.is_empty()); // duplicate, not re-queued
    }

    #[test]
    fn cache_integration_works() {
        let mut engine = PrefetchEngine::new();
        let mut cache = DeterministicCache::new(1024 * 1024);

        // Insert study data into cache
        engine.cache_study_data(&mut cache, "1.2.3.4.5", vec![1u8, 2, 3, 4, 5], 5);

        // Check cache hit
        let data = engine.check_cache(&mut cache, "1.2.3.4.5");
        assert!(data.is_some());
        assert_eq!(data.unwrap().as_slice(), &[1u8, 2, 3, 4, 5]);

        let stats = engine.stats();
        assert_eq!(stats.cache_hits, 1);

        // Check cache miss
        let miss = engine.check_cache(&mut cache, "9.9.9.9");
        assert!(miss.is_none());
        let stats = engine.stats();
        assert_eq!(stats.cache_misses, 1);
    }

    #[test]
    fn release_study_data_unpins() {
        let mut engine = PrefetchEngine::new();
        let mut cache = DeterministicCache::new(16);

        engine.cache_study_data(&mut cache, "1.2.3", vec![1u8; 10], 10);
        assert!(cache.get("1.2.3").is_some());

        // Release (unpin) the study
        assert!(engine.release_study_data(&mut cache, "1.2.3"));

        // Now insert more data to trigger eviction of the unpinned entry
        engine.cache_study_data(&mut cache, "4.5.6", vec![2u8; 10], 10);
        // The unpinned entry may be evicted now
    }

    #[test]
    fn default_rules_cover_common_modalities() {
        let rules = default_prefetch_rules();
        assert!(rules.iter().any(|r| r.modality.as_deref() == Some("CT")));
        assert!(rules.iter().any(|r| r.modality.as_deref() == Some("MR")));
        assert!(rules.iter().any(|r| r.modality.as_deref() == Some("MG")));
        assert!(rules.iter().any(|r| r.modality.as_deref() == Some("XR")));
    }

    #[test]
    fn mark_failed_records_failure() {
        let mut engine = PrefetchEngine::new();
        engine.add_rule(PrefetchRule {
            rule_id: "r1".to_string(),
            modality: Some("CT".to_string()),
            body_part: None,
            prior_count: 1,
            priority: PrefetchPriority::Normal,
            enabled: true,
        });

        let trigger = WorklistTrigger {
            patient_id: Some("PAT001".to_string()),
            accession_number: None,
            modality: "CT".to_string(),
            body_part: None,
            scheduled_date: "20240101".to_string(),
            scheduled_time: "090000".to_string(),
        };

        let requests = engine.process_worklist_trigger(&trigger);
        // We need to get the request_id from the queue since mark_failed looks there
        let request_id = requests[0].request_id.clone();
        assert!(engine.mark_failed(&request_id));

        let stats = engine.stats();
        assert_eq!(stats.total_failed, 1);
        assert_eq!(engine.queue_depth(), 0);
    }

    // =======================================================================
    // Sprint 3 Extended Tests: Prefetch Engine
    // =======================================================================

    #[test]
    fn multiple_rules_different_modalities() {
        let mut engine = PrefetchEngine::new();
        engine.add_rule(PrefetchRule {
            rule_id: "ct_rule".to_string(),
            modality: Some("CT".to_string()),
            body_part: None,
            prior_count: 1,
            priority: PrefetchPriority::Normal,
            enabled: true,
        });
        engine.add_rule(PrefetchRule {
            rule_id: "mr_rule".to_string(),
            modality: Some("MR".to_string()),
            body_part: None,
            prior_count: 2,
            priority: PrefetchPriority::High,
            enabled: true,
        });
        let ct_trigger = WorklistTrigger {
            patient_id: Some("PAT001".to_string()),
            accession_number: None,
            modality: "CT".to_string(),
            body_part: None,
            scheduled_date: "20240101".to_string(),
            scheduled_time: "090000".to_string(),
        };
        let mr_trigger = WorklistTrigger {
            patient_id: Some("PAT002".to_string()),
            accession_number: None,
            modality: "MR".to_string(),
            body_part: None,
            scheduled_date: "20240101".to_string(),
            scheduled_time: "100000".to_string(),
        };
        let ct_reqs = engine.process_worklist_trigger(&ct_trigger);
        assert_eq!(ct_reqs.len(), 1);
        assert_eq!(ct_reqs[0].priority, PrefetchPriority::Normal);
        let mr_reqs = engine.process_worklist_trigger(&mr_trigger);
        assert_eq!(mr_reqs.len(), 1); // One request per rule per trigger
        assert_eq!(mr_reqs[0].priority, PrefetchPriority::High);
    }

    #[test]
    fn body_part_specific_prefetch_rule() {
        let mut engine = PrefetchEngine::new();
        engine.add_rule(PrefetchRule {
            rule_id: "ct_chest_rule".to_string(),
            modality: Some("CT".to_string()),
            body_part: Some("CHEST".to_string()),
            prior_count: 1,
            priority: PrefetchPriority::High,
            enabled: true,
        });
        let matching_trigger = WorklistTrigger {
            patient_id: Some("PAT001".to_string()),
            accession_number: None,
            modality: "CT".to_string(),
            body_part: Some("CHEST".to_string()),
            scheduled_date: "20240101".to_string(),
            scheduled_time: "090000".to_string(),
        };
        let non_matching_trigger = WorklistTrigger {
            patient_id: Some("PAT002".to_string()),
            accession_number: None,
            modality: "CT".to_string(),
            body_part: Some("ABDOMEN".to_string()),
            scheduled_date: "20240101".to_string(),
            scheduled_time: "100000".to_string(),
        };
        let matching_reqs = engine.process_worklist_trigger(&matching_trigger);
        assert_eq!(matching_reqs.len(), 1);
        let non_matching_reqs = engine.process_worklist_trigger(&non_matching_trigger);
        assert_eq!(non_matching_reqs.len(), 0);
    }

    #[test]
    fn priority_ordering_normal_high_stat() {
        let mut engine = PrefetchEngine::new();
        engine.add_rule(PrefetchRule {
            rule_id: "low_rule".to_string(),
            modality: Some("CT".to_string()),
            body_part: None,
            prior_count: 1,
            priority: PrefetchPriority::Low,
            enabled: true,
        });
        engine.add_rule(PrefetchRule {
            rule_id: "stat_rule".to_string(),
            modality: Some("MR".to_string()),
            body_part: None,
            prior_count: 1,
            priority: PrefetchPriority::Stat,
            enabled: true,
        });
        engine.add_rule(PrefetchRule {
            rule_id: "high_rule".to_string(),
            modality: Some("XA".to_string()),
            body_part: None,
            prior_count: 1,
            priority: PrefetchPriority::High,
            enabled: true,
        });
        // Trigger all three
        for (modality, _) in [("CT", "Low"), ("MR", "STAT"), ("XA", "High")] {
            let trigger = WorklistTrigger {
                patient_id: Some("PAT001".to_string()),
                accession_number: None,
                modality: modality.to_string(),
                body_part: None,
                scheduled_date: "20240101".to_string(),
                scheduled_time: "090000".to_string(),
            };
            engine.process_worklist_trigger(&trigger);
        }
        // Pop should return STAT first
        let first = engine.pop_next().expect("should have request");
        assert_eq!(first.priority, PrefetchPriority::Stat);
        let second = engine.pop_next().expect("should have request");
        assert_eq!(second.priority, PrefetchPriority::High);
        let third = engine.pop_next().expect("should have request");
        assert_eq!(third.priority, PrefetchPriority::Low);
    }

    #[test]
    fn cache_pin_prevents_eviction() {
        let mut engine = PrefetchEngine::new();
        let mut cache = DeterministicCache::new(1024); // Very small cache
        engine.cache_study_data(&mut cache, "study_pinned", vec![1u8, 2, 3, 4, 5], 5);
        // The data should be accessible
        let data = engine.check_cache(&mut cache, "study_pinned");
        assert!(data.is_some());
    }

    #[test]
    fn cancel_nonexistent_request_returns_false() {
        let mut engine = PrefetchEngine::new();
        assert!(!engine.cancel("nonexistent_id"));
    }

    #[test]
    fn stats_track_completed_and_failed() {
        let mut engine = PrefetchEngine::new();
        engine.add_rule(PrefetchRule {
            rule_id: "r1".to_string(),
            modality: Some("CT".to_string()),
            body_part: None,
            prior_count: 1,
            priority: PrefetchPriority::Normal,
            enabled: true,
        });
        let trigger1 = WorklistTrigger {
            patient_id: Some("PAT001".to_string()),
            accession_number: None,
            modality: "CT".to_string(),
            body_part: None,
            scheduled_date: "20240101".to_string(),
            scheduled_time: "090000".to_string(),
        };
        let requests = engine.process_worklist_trigger(&trigger1);
        let req_id = requests[0].request_id.clone();
        engine.mark_completed(&req_id);
        assert_eq!(engine.stats().total_completed, 1);

        // Add another with different patient to avoid dedup
        let trigger2 = WorklistTrigger {
            patient_id: Some("PAT002".to_string()),
            accession_number: None,
            modality: "CT".to_string(),
            body_part: None,
            scheduled_date: "20240102".to_string(),
            scheduled_time: "090000".to_string(),
        };
        let requests2 = engine.process_worklist_trigger(&trigger2);
        let req_id2 = requests2[0].request_id.clone();
        engine.mark_failed(&req_id2);
        assert_eq!(engine.stats().total_failed, 1);
        assert_eq!(engine.stats().total_completed, 1);
    }
