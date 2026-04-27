//! FHIR Subscription mechanism for real-time notification.
//!
//! Implements the FHIR R4 Subscription resource and a simple subscription
//! manager that supports registering subscriptions and notifying
//! subscribers when resources matching the subscription criteria are
//! created or updated.

use serde::{Deserialize, Serialize};
use std::fmt;

/// FHIR R4 Subscription status values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SubscriptionStatus {
    /// Client has requested the subscription but server has not yet set it up.
    Requested,
    /// Subscription is active and notifications will be sent.
    Active,
    /// Subscription has encountered an error.
    Error,
    /// Subscription has been turned off.
    Off,
}

impl fmt::Display for SubscriptionStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SubscriptionStatus::Requested => write!(f, "requested"),
            SubscriptionStatus::Active => write!(f, "active"),
            SubscriptionStatus::Error => write!(f, "error"),
            SubscriptionStatus::Off => write!(f, "off"),
        }
    }
}

/// Channel type for subscription notifications.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChannelType {
    /// RESTful hook (POST to endpoint URL).
    RestHook,
    /// WebSocket notification.
    Websocket,
    /// Email notification.
    Email,
    /// SMS notification.
    Sms,
}

impl fmt::Display for ChannelType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ChannelType::RestHook => write!(f, "rest-hook"),
            ChannelType::Websocket => write!(f, "websocket"),
            ChannelType::Email => write!(f, "email"),
            ChannelType::Sms => write!(f, "sms"),
        }
    }
}

/// Subscription channel configuration.
///
/// Defines how and where notifications are delivered for a subscription.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubscriptionChannel {
    /// Channel type (delivery mechanism).
    pub channel_type: ChannelType,
    /// Endpoint URL or address for notification delivery.
    pub endpoint: String,
    /// MIME type for notification payload.
    pub payload: Option<String>,
    /// Additional HTTP headers for rest-hook notifications.
    pub headers: Vec<String>,
}

/// FHIR R4 Subscription resource.
///
/// Represents a subscription to a FHIR resource type. When resources
/// matching the criteria are created or updated, notifications are
/// sent to the specified channel endpoint.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FhirSubscription {
    /// Logical id of the subscription.
    pub id: String,
    /// Current status of the subscription.
    pub status: SubscriptionStatus,
    /// FHIR search criteria (e.g., "ImagingStudy?patient=PAT001").
    pub criteria: String,
    /// Notification channel configuration.
    pub channel: SubscriptionChannel,
    /// Human-readable reason for the subscription.
    pub reason: Option<String>,
}

impl FhirSubscription {
    /// Create a new subscription.
    pub fn new(
        id: &str,
        criteria: &str,
        channel_type: ChannelType,
        endpoint: &str,
    ) -> Self {
        Self {
            id: id.to_string(),
            status: SubscriptionStatus::Requested,
            criteria: criteria.to_string(),
            channel: SubscriptionChannel {
                channel_type,
                endpoint: endpoint.to_string(),
                payload: Some("application/fhir+json".to_string()),
                headers: Vec::new(),
            },
            reason: None,
        }
    }

    /// Activate the subscription.
    pub fn activate(&mut self) {
        self.status = SubscriptionStatus::Active;
    }

    /// Deactivate the subscription.
    pub fn deactivate(&mut self) {
        self.status = SubscriptionStatus::Off;
    }

    /// Check if the subscription is active.
    pub fn is_active(&self) -> bool {
        self.status == SubscriptionStatus::Active
    }

    /// Return the FHIR resource type string.
    pub fn resource_type(&self) -> &str {
        "Subscription"
    }

    /// Check if a resource matches this subscription's criteria.
    ///
    /// Simple criteria matching: supports `ResourceType?param=value`
    /// and `ResourceType?param1=value1&param2=value2` patterns.
    pub fn matches(&self, resource_type: &str, resource_id: &str) -> bool {
        if !self.is_active() {
            return false;
        }

        // Parse criteria: "ImagingStudy?patient=PAT001&modality=CT"
        let parts: Vec<&str> = self.criteria.split('?').collect();
        if parts.is_empty() {
            return false;
        }

        let criteria_resource_type = parts[0];

        // Resource type must match
        if criteria_resource_type != resource_type {
            return false;
        }

        // If no query parameters, any resource of the right type matches
        if parts.len() == 1 {
            return true;
        }

        // Check if the criteria has a `_id` parameter that matches
        let query = parts[1];
        for param in query.split('&') {
            if let Some((key, value)) = param.split_once('=') {
                if key == "_id" && value == resource_id {
                    return true;
                }
            }
        }

        // Without specific _id matching, a broad criteria matches all
        // resources of the right type
        !query.contains("_id=")
    }
}

/// Simple subscription manager.
///
/// Manages FHIR Subscriptions and provides notification capabilities
/// when resources are created or updated. Subscriptions are held in
/// memory and matched against resource events.
pub struct SubscriptionManager {
    /// Registered subscriptions.
    subscriptions: Vec<FhirSubscription>,
}

impl fmt::Debug for SubscriptionManager {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SubscriptionManager")
            .field("subscription_count", &self.subscriptions.len())
            .finish()
    }
}

impl Default for SubscriptionManager {
    fn default() -> Self {
        Self::new()
    }
}

impl SubscriptionManager {
    /// Create a new subscription manager.
    pub fn new() -> Self {
        Self {
            subscriptions: Vec::new(),
        }
    }

    /// Add a subscription.
    pub fn add_subscription(&mut self, sub: FhirSubscription) {
        self.subscriptions.push(sub);
    }

    /// Remove a subscription by ID.
    pub fn remove_subscription(&mut self, id: &str) -> bool {
        let before = self.subscriptions.len();
        self.subscriptions.retain(|s| s.id != id);
        self.subscriptions.len() < before
    }

    /// Return the number of subscriptions.
    pub fn len(&self) -> usize {
        self.subscriptions.len()
    }

    /// Check if there are no subscriptions.
    pub fn is_empty(&self) -> bool {
        self.subscriptions.is_empty()
    }

    /// Get a subscription by ID.
    pub fn get(&self, id: &str) -> Option<&FhirSubscription> {
        self.subscriptions.iter().find(|s| s.id == id)
    }

    /// Get a mutable subscription by ID.
    pub fn get_mut(&mut self, id: &str) -> Option<&mut FhirSubscription> {
        self.subscriptions.iter_mut().find(|s| s.id == id)
    }

    /// Notify subscribers matching a resource event.
    ///
    /// Checks all active subscriptions against the given resource type
    /// and ID, returning the results of each notification attempt.
    /// In a real implementation, this would send HTTP requests to
    /// the subscription endpoints.
    pub fn notify_subscribers(
        &self,
        resource_type: &str,
        resource_id: &str,
    ) -> Vec<Result<(), String>> {
        self.subscriptions
            .iter()
            .filter(|sub| sub.matches(resource_type, resource_id))
            .map(|sub| {
                // In production, this would POST to sub.channel.endpoint
                // For now, just validate the endpoint is not empty
                if sub.channel.endpoint.is_empty() {
                    Err(format!(
                        "subscription {} has empty endpoint",
                        sub.id
                    ))
                } else {
                    Ok(())
                }
            })
            .collect()
    }

    /// Return all active subscriptions matching a resource type.
    pub fn active_for_type(&self, resource_type: &str) -> Vec<&FhirSubscription> {
        self.subscriptions
            .iter()
            .filter(|sub| {
                sub.is_active() && sub.criteria.split('?').next() == Some(resource_type)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_subscription() {
        let sub = FhirSubscription::new(
            "sub-1",
            "ImagingStudy?patient=PAT001",
            ChannelType::RestHook,
            "https://example.com/hook",
        );
        assert_eq!(sub.id, "sub-1");
        assert_eq!(sub.status, SubscriptionStatus::Requested);
        assert!(!sub.is_active());
    }

    #[test]
    fn activate_subscription() {
        let mut sub = FhirSubscription::new(
            "sub-1",
            "ImagingStudy",
            ChannelType::RestHook,
            "https://example.com/hook",
        );
        sub.activate();
        assert_eq!(sub.status, SubscriptionStatus::Active);
        assert!(sub.is_active());
    }

    #[test]
    fn deactivate_subscription() {
        let mut sub = FhirSubscription::new(
            "sub-1",
            "ImagingStudy",
            ChannelType::RestHook,
            "https://example.com/hook",
        );
        sub.activate();
        sub.deactivate();
        assert_eq!(sub.status, SubscriptionStatus::Off);
        assert!(!sub.is_active());
    }

    #[test]
    fn subscription_matches_resource_type() {
        let mut sub = FhirSubscription::new(
            "sub-1",
            "ImagingStudy",
            ChannelType::RestHook,
            "https://example.com/hook",
        );
        sub.activate();

        assert!(sub.matches("ImagingStudy", "any-id"));
        assert!(!sub.matches("Patient", "any-id"));
    }

    #[test]
    fn subscription_matches_with_id_criteria() {
        let mut sub = FhirSubscription::new(
            "sub-1",
            "ImagingStudy?_id=study-123",
            ChannelType::RestHook,
            "https://example.com/hook",
        );
        sub.activate();

        assert!(sub.matches("ImagingStudy", "study-123"));
        assert!(!sub.matches("ImagingStudy", "study-456"));
    }

    #[test]
    fn inactive_subscription_does_not_match() {
        let sub = FhirSubscription::new(
            "sub-1",
            "ImagingStudy",
            ChannelType::RestHook,
            "https://example.com/hook",
        );
        // Not activated — status is Requested
        assert!(!sub.matches("ImagingStudy", "any-id"));
    }

    #[test]
    fn subscription_manager_add_and_remove() {
        let mut mgr = SubscriptionManager::new();
        let sub = FhirSubscription::new(
            "sub-1",
            "ImagingStudy",
            ChannelType::RestHook,
            "https://example.com/hook",
        );
        mgr.add_subscription(sub);
        assert_eq!(mgr.len(), 1);

        assert!(mgr.remove_subscription("sub-1"));
        assert!(mgr.is_empty());
    }

    #[test]
    fn subscription_manager_notify() {
        let mut mgr = SubscriptionManager::new();
        let mut sub = FhirSubscription::new(
            "sub-1",
            "ImagingStudy",
            ChannelType::RestHook,
            "https://example.com/hook",
        );
        sub.activate();
        mgr.add_subscription(sub);

        let results = mgr.notify_subscribers("ImagingStudy", "study-123");
        assert_eq!(results.len(), 1);
        assert!(results[0].is_ok());
    }

    #[test]
    fn subscription_manager_notify_wrong_type() {
        let mut mgr = SubscriptionManager::new();
        let mut sub = FhirSubscription::new(
            "sub-1",
            "ImagingStudy",
            ChannelType::RestHook,
            "https://example.com/hook",
        );
        sub.activate();
        mgr.add_subscription(sub);

        let results = mgr.notify_subscribers("Patient", "PAT001");
        assert!(results.is_empty());
    }

    #[test]
    fn subscription_manager_active_for_type() {
        let mut mgr = SubscriptionManager::new();

        let mut sub1 = FhirSubscription::new(
            "sub-1",
            "ImagingStudy",
            ChannelType::RestHook,
            "https://example.com/hook1",
        );
        sub1.activate();

        let mut sub2 = FhirSubscription::new(
            "sub-2",
            "Patient",
            ChannelType::RestHook,
            "https://example.com/hook2",
        );
        sub2.activate();

        let sub3 = FhirSubscription::new(
            "sub-3",
            "ImagingStudy",
            ChannelType::RestHook,
            "https://example.com/hook3",
        );
        // sub3 is not activated

        mgr.add_subscription(sub1);
        mgr.add_subscription(sub2);
        mgr.add_subscription(sub3);

        let imaging_subs = mgr.active_for_type("ImagingStudy");
        assert_eq!(imaging_subs.len(), 1);
        assert_eq!(imaging_subs[0].id, "sub-1");
    }

    #[test]
    fn subscription_serialization() {
        let sub = FhirSubscription::new(
            "sub-1",
            "ImagingStudy?patient=PAT001",
            ChannelType::RestHook,
            "https://example.com/hook",
        );
        let json = serde_json::to_string(&sub).expect("serialize");
        let restored: FhirSubscription = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored.id, sub.id);
        assert_eq!(restored.criteria, sub.criteria);
    }

    #[test]
    fn subscription_status_display() {
        assert_eq!(SubscriptionStatus::Requested.to_string(), "requested");
        assert_eq!(SubscriptionStatus::Active.to_string(), "active");
        assert_eq!(SubscriptionStatus::Error.to_string(), "error");
        assert_eq!(SubscriptionStatus::Off.to_string(), "off");
    }

    #[test]
    fn channel_type_display() {
        assert_eq!(ChannelType::RestHook.to_string(), "rest-hook");
        assert_eq!(ChannelType::Websocket.to_string(), "websocket");
        assert_eq!(ChannelType::Email.to_string(), "email");
        assert_eq!(ChannelType::Sms.to_string(), "sms");
    }
}
