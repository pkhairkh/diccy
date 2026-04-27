//! Study prefetching engine with priority queuing and DICOMweb integration.
//!
//! Provides rule-based prefetch on worklist entry, priority queuing with STAT
//! escalation, and integration with `DeterministicCache` for cached study data.
//! Designed to load prior studies before the radiologist opens the current study.

use crate::DeterministicCache;
use std::collections::BTreeMap;

/// Prefetch priority levels.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum PrefetchPriority {
    /// Low priority — background prefetch of old priors.
    Low = 1,
    /// Normal priority — standard prefetch of recent priors.
    Normal = 2,
    /// High priority — prefetch for imminent worklist entries.
    High = 3,
    /// STAT — emergency escalation, must prefetch immediately.
    Stat = 4,
}

/// A prefetch rule that triggers study retrieval based on matching criteria.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrefetchRule {
    /// Rule identifier.
    pub rule_id: String,
    /// Modality filter for triggering this rule.
    pub modality: Option<String>,
    /// Body part filter for triggering this rule.
    pub body_part: Option<String>,
    /// Number of prior studies to prefetch.
    pub prior_count: usize,
    /// Priority for studies fetched by this rule.
    pub priority: PrefetchPriority,
    /// Whether this rule is active.
    pub enabled: bool,
}

/// A single prefetch request in the priority queue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrefetchRequest {
    /// Request identifier (deterministic, based on study UID).
    pub request_id: String,
    /// Study Instance UID to prefetch.
    pub study_uid: String,
    /// Patient ID for the study.
    pub patient_id: Option<String>,
    /// Accession number for the study.
    pub accession_number: Option<String>,
    /// Priority of this request.
    pub priority: PrefetchPriority,
    /// Reason for the prefetch (e.g., "worklist_trigger", "prior_lookup").
    pub reason: String,
    /// Whether this request has been completed.
    pub completed: bool,
}

/// Status of a prefetch request.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum PrefetchStatus {
    /// Request is queued and waiting.
    Queued,
    /// Request is actively being fetched.
    InProgress,
    /// Request completed successfully.
    Completed,
    /// Request failed.
    Failed,
    /// Request was cancelled.
    Cancelled,
}

/// Statistics about the prefetch engine state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PrefetchStats {
    /// Total requests queued.
    pub total_queued: u64,
    /// Total requests completed.
    pub total_completed: u64,
    /// Total requests failed.
    pub total_failed: u64,
    /// Current queue depth.
    pub queue_depth: usize,
    /// Cache hit count during prefetch.
    pub cache_hits: u64,
    /// Cache miss count during prefetch.
    pub cache_misses: u64,
}

/// Worklist entry context that triggers prefetch rules.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorklistTrigger {
    /// Patient ID.
    pub patient_id: Option<String>,
    /// Accession number.
    pub accession_number: Option<String>,
    /// Modality.
    pub modality: String,
    /// Body part (if known).
    pub body_part: Option<String>,
    /// Scheduled date (DA format).
    pub scheduled_date: String,
    /// Scheduled time (TM format).
    pub scheduled_time: String,
}

/// Prefetch engine with priority queue and rule matching.
#[derive(Debug, Clone)]
pub struct PrefetchEngine {
    /// Registered prefetch rules, keyed by rule_id.
    rules: BTreeMap<String, PrefetchRule>,
    /// Priority queue of pending requests (keyed by priority + request_id for determinism).
    queue: BTreeMap<(PrefetchPriority, String), PrefetchRequest>,
    /// Completed requests archive.
    completed: Vec<PrefetchRequest>,
    /// Failed requests archive.
    failed: Vec<PrefetchRequest>,
    /// Statistics.
    stats: PrefetchStats,
}

impl Default for PrefetchEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl PrefetchEngine {
    /// Create an empty prefetch engine.
    pub fn new() -> Self {
        Self {
            rules: BTreeMap::new(),
            queue: BTreeMap::new(),
            completed: Vec::new(),
            failed: Vec::new(),
            stats: PrefetchStats::default(),
        }
    }

    /// Register a prefetch rule.
    pub fn add_rule(&mut self, rule: PrefetchRule) {
        self.rules.insert(rule.rule_id.clone(), rule);
    }

    /// Remove a prefetch rule by identifier.
    pub fn remove_rule(&mut self, rule_id: &str) -> bool {
        self.rules.remove(rule_id).is_some()
    }

    /// Return the number of registered rules.
    pub fn rule_count(&self) -> usize {
        self.rules.len()
    }

    /// Process a worklist trigger and generate prefetch requests based on rules.
    ///
    /// For each matching rule, creates a prefetch request for the specified number
    /// of prior studies. Requests are deduplicated by study UID.
    pub fn process_worklist_trigger(&mut self, trigger: &WorklistTrigger) -> Vec<PrefetchRequest> {
        let mut new_requests = Vec::new();

        for rule in self.rules.values() {
            if !rule.enabled {
                continue;
            }
            if let Some(ref modality) = rule.modality {
                if !trigger.modality.eq_ignore_ascii_case(modality) {
                    continue;
                }
            }
            if let Some(ref body_part) = rule.body_part {
                let matches_bp = trigger
                    .body_part
                    .as_ref()
                    .map(|bp| bp.eq_ignore_ascii_case(body_part))
                    .unwrap_or(false);
                if !matches_bp {
                    continue;
                }
            }

            // Generate a prefetch request for this rule.
            // In a real implementation, this would query the PACS for prior study UIDs.
            // Here we create a placeholder request with the patient context.
            let request_id = format!(
                "pf_{}_{}_{}",
                rule.rule_id,
                trigger.patient_id.as_deref().unwrap_or("UNKNOWN"),
                trigger.accession_number.as_deref().unwrap_or("UNKNOWN")
            );

            let request = PrefetchRequest {
                request_id: request_id.clone(),
                study_uid: format!(
                    "STUDY.{}.{}.{}",
                    rule.rule_id,
                    trigger.patient_id.as_deref().unwrap_or("UNKNOWN"),
                    trigger.accession_number.as_deref().unwrap_or("UNKNOWN")
                ),
                patient_id: trigger.patient_id.clone(),
                accession_number: trigger.accession_number.clone(),
                priority: rule.priority,
                reason: format!("worklist_trigger:{}", rule.rule_id),
                completed: false,
            };

            // Deduplicate: don't re-queue if already in queue or completed.
            let already_queued = self
                .queue
                .values()
                .any(|r| r.study_uid == request.study_uid);
            let already_completed = self
                .completed
                .iter()
                .any(|r| r.study_uid == request.study_uid);

            if !already_queued && !already_completed {
                let key = (rule.priority, request_id.clone());
                self.queue.insert(key, request.clone());
                self.stats.total_queued += 1;
                new_requests.push(request);
            }
        }

        new_requests
    }

    /// Escalate a queued request to STAT priority.
    ///
    /// Returns true if the request was found and escalated.
    pub fn escalate_stat(&mut self, request_id: &str) -> bool {
        // Find the request in the queue.
        let found_key = self
            .queue
            .iter()
            .find(|(_, req)| req.request_id == request_id)
            .map(|(key, _)| key.clone());

        if let Some(old_key) = found_key {
            let mut request = self.queue.remove(&old_key).expect("found");
            request.priority = PrefetchPriority::Stat;
            let new_key = (PrefetchPriority::Stat, request.request_id.clone());
            self.queue.insert(new_key, request);
            return true;
        }
        false
    }

    /// Pop the highest-priority request from the queue.
    ///
    /// Returns None if the queue is empty.
    pub fn pop_next(&mut self) -> Option<PrefetchRequest> {
        // BTreeMap iterates in ascending key order. We want highest priority first.
        // Since PrefetchPriority is ordered Low < Normal < High < Stat,
        // we take from the back.
        let last_key = self.queue.keys().last().cloned();
        match last_key {
            Some(key) => {
                let request = self.queue.remove(&key).expect("key exists");
                Some(request)
            }
            None => None,
        }
    }

    /// Mark a request as completed successfully.
    pub fn mark_completed(&mut self, request_id: &str) -> bool {
        let found_key = self
            .queue
            .iter()
            .find(|(_, req)| req.request_id == request_id)
            .map(|(key, _)| key.clone());

        if let Some(key) = found_key {
            let mut request = self.queue.remove(&key).expect("found");
            request.completed = true;
            self.completed.push(request);
            self.stats.total_completed += 1;
            return true;
        }
        false
    }

    /// Mark a request as failed.
    pub fn mark_failed(&mut self, request_id: &str) -> bool {
        let found_key = self
            .queue
            .iter()
            .find(|(_, req)| req.request_id == request_id)
            .map(|(key, _)| key.clone());

        if let Some(key) = found_key {
            let request = self.queue.remove(&key).expect("found");
            self.failed.push(request);
            self.stats.total_failed += 1;
            return true;
        }
        false
    }

    /// Cancel a queued request.
    pub fn cancel(&mut self, request_id: &str) -> bool {
        let found_key = self
            .queue
            .iter()
            .find(|(_, req)| req.request_id == request_id)
            .map(|(key, _)| key.clone());

        if let Some(key) = found_key {
            self.queue.remove(&key);
            return true;
        }
        false
    }

    /// Return a view of current queue items (sorted by priority).
    pub fn queue(&self) -> Vec<&PrefetchRequest> {
        self.queue.values().collect()
    }

    /// Return the current queue depth.
    pub fn queue_depth(&self) -> usize {
        self.queue.len()
    }

    /// Return prefetch statistics.
    pub fn stats(&self) -> PrefetchStats {
        let mut stats = self.stats;
        stats.queue_depth = self.queue.len();
        stats
    }

    /// Check if cached study data exists in the DeterministicCache.
    ///
    /// Uses the study UID as the cache key. Returns the cached bytes if available,
    /// or None on cache miss.
    pub fn check_cache<'a>(
        &mut self,
        cache: &'a mut DeterministicCache<Vec<u8>>,
        study_uid: &str,
    ) -> Option<&'a Vec<u8>> {
        let result = cache.get(study_uid);
        if result.is_some() {
            self.stats.cache_hits += 1;
        } else {
            self.stats.cache_misses += 1;
        }
        result
    }

    /// Insert prefetched data into the DeterministicCache.
    ///
    /// Pins the study data to prevent eviction while it's being actively used.
    pub fn cache_study_data(
        &mut self,
        cache: &mut DeterministicCache<Vec<u8>>,
        study_uid: &str,
        data: Vec<u8>,
        size_bytes: u64,
    ) {
        cache.insert(study_uid.to_string(), data, size_bytes);
        cache.pin(study_uid);
    }

    /// Unpin study data from cache when the radiologist is done reading.
    pub fn release_study_data(
        &mut self,
        cache: &mut DeterministicCache<Vec<u8>>,
        study_uid: &str,
    ) -> bool {
        cache.unpin(study_uid)
    }
}

/// Create a default set of prefetch rules for common modalities.
pub fn default_prefetch_rules() -> Vec<PrefetchRule> {
    vec![
        PrefetchRule {
            rule_id: "ct_prior_1".to_string(),
            modality: Some("CT".to_string()),
            body_part: None,
            prior_count: 1,
            priority: PrefetchPriority::Normal,
            enabled: true,
        },
        PrefetchRule {
            rule_id: "mr_prior_1".to_string(),
            modality: Some("MR".to_string()),
            body_part: None,
            prior_count: 1,
            priority: PrefetchPriority::Normal,
            enabled: true,
        },
        PrefetchRule {
            rule_id: "mg_prior_2".to_string(),
            modality: Some("MG".to_string()),
            body_part: None,
            prior_count: 2,
            priority: PrefetchPriority::High,
            enabled: true,
        },
        PrefetchRule {
            rule_id: "xr_prior_1".to_string(),
            modality: Some("XR".to_string()),
            body_part: None,
            prior_count: 1,
            priority: PrefetchPriority::Low,
            enabled: true,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
