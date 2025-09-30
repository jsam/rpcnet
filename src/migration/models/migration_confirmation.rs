use serde::{Deserialize, Serialize};
use std::time::SystemTime;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConfirmationDecision {
    Accept,
    Reject,
    Defer,
}

impl Default for ConfirmationDecision {
    fn default() -> Self {
        ConfirmationDecision::Accept
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MigrationParty {
    Client,
    Server,
    LoadBalancer,
    Administrator,
    System,
}

impl Default for MigrationParty {
    fn default() -> Self {
        MigrationParty::System
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConfirmationState {
    Created,
    Processed,
}

impl Default for ConfirmationState {
    fn default() -> Self {
        ConfirmationState::Created
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationConfirmation {
    pub confirmation_id: Uuid,
    pub request_id: Uuid,
    pub decision: ConfirmationDecision,
    pub confirmed_by: MigrationParty,
    pub confirmed_at: SystemTime,
    pub reason: Option<String>,
    pub auto_confirmed: bool,
    pub state: ConfirmationState,
    pub timeout_at: Option<SystemTime>,
    pub priority_override: Option<u8>, // Optional priority boost/reduction
}

impl MigrationConfirmation {
    pub fn new(request_id: Uuid, decision: ConfirmationDecision, confirmed_by: MigrationParty) -> Self {
        Self {
            confirmation_id: Uuid::new_v4(),
            request_id,
            decision,
            confirmed_by,
            confirmed_at: SystemTime::now(),
            reason: None,
            auto_confirmed: false,
            state: ConfirmationState::default(),
            timeout_at: None,
            priority_override: None,
        }
    }

    pub fn accept(request_id: Uuid, confirmed_by: MigrationParty) -> Self {
        Self::new(request_id, ConfirmationDecision::Accept, confirmed_by)
    }

    pub fn reject(request_id: Uuid, confirmed_by: MigrationParty, reason: String) -> Self {
        Self::new(request_id, ConfirmationDecision::Reject, confirmed_by)
            .with_reason(reason)
    }

    pub fn defer(request_id: Uuid, confirmed_by: MigrationParty, reason: String) -> Self {
        Self::new(request_id, ConfirmationDecision::Defer, confirmed_by)
            .with_reason(reason)
    }

    pub fn auto_accept(request_id: Uuid) -> Self {
        Self::new(request_id, ConfirmationDecision::Accept, MigrationParty::System)
            .as_auto_confirmed()
    }

    pub fn auto_reject(request_id: Uuid, reason: String) -> Self {
        Self::new(request_id, ConfirmationDecision::Reject, MigrationParty::System)
            .with_reason(reason)
            .as_auto_confirmed()
    }

    pub fn with_reason(mut self, reason: String) -> Self {
        self.reason = Some(reason);
        self
    }

    pub fn as_auto_confirmed(mut self) -> Self {
        self.auto_confirmed = true;
        self
    }

    pub fn with_timeout(mut self, timeout_at: SystemTime) -> Self {
        self.timeout_at = Some(timeout_at);
        self
    }

    pub fn with_priority_override(mut self, priority: u8) -> Self {
        self.priority_override = Some(priority);
        self
    }

    pub fn mark_processed(&mut self) {
        self.state = ConfirmationState::Processed;
    }

    pub fn is_acceptance(&self) -> bool {
        matches!(self.decision, ConfirmationDecision::Accept)
    }

    pub fn is_rejection(&self) -> bool {
        matches!(self.decision, ConfirmationDecision::Reject)
    }

    pub fn is_deferral(&self) -> bool {
        matches!(self.decision, ConfirmationDecision::Defer)
    }

    pub fn is_auto_confirmed(&self) -> bool {
        self.auto_confirmed
    }

    pub fn is_processed(&self) -> bool {
        matches!(self.state, ConfirmationState::Processed)
    }

    pub fn is_from_client(&self) -> bool {
        matches!(self.confirmed_by, MigrationParty::Client)
    }

    pub fn is_from_server(&self) -> bool {
        matches!(self.confirmed_by, MigrationParty::Server)
    }

    pub fn is_from_admin(&self) -> bool {
        matches!(self.confirmed_by, MigrationParty::Administrator)
    }

    pub fn age(&self) -> std::time::Duration {
        SystemTime::now()
            .duration_since(self.confirmed_at)
            .unwrap_or(std::time::Duration::ZERO)
    }

    pub fn is_timed_out(&self) -> bool {
        if let Some(timeout_at) = self.timeout_at {
            SystemTime::now() > timeout_at
        } else {
            false
        }
    }

    pub fn time_until_timeout(&self) -> Option<std::time::Duration> {
        self.timeout_at.and_then(|timeout_at| {
            timeout_at.duration_since(SystemTime::now()).ok()
        })
    }

    pub fn validate(&self) -> Result<(), String> {
        // Validation for rejection/deferral
        if (self.is_rejection() || self.is_deferral()) && self.reason.is_none() {
            return Err("Rejection and deferral decisions must include a reason".to_string());
        }

        // Validation for timeout
        if let Some(timeout_at) = self.timeout_at {
            if timeout_at <= self.confirmed_at {
                return Err("Timeout must be set in the future".to_string());
            }
        }

        // Validation for priority override
        if let Some(priority) = self.priority_override {
            if priority > 10 {
                return Err("Priority override must be between 0 and 10".to_string());
            }
        }

        // Auto-confirmed validations
        if self.auto_confirmed && !matches!(self.confirmed_by, MigrationParty::System) {
            return Err("Auto-confirmed decisions must be from System party".to_string());
        }

        Ok(())
    }

    pub fn rejection_reason(&self) -> Option<&str> {
        if self.is_rejection() {
            self.reason.as_deref()
        } else {
            None
        }
    }

    pub fn deferral_reason(&self) -> Option<&str> {
        if self.is_deferral() {
            self.reason.as_deref()
        } else {
            None
        }
    }

    pub fn should_retry(&self) -> bool {
        // Only deferrals suggest a retry might succeed
        self.is_deferral() && !self.is_timed_out()
    }

    pub fn priority_score(&self) -> u8 {
        let base_score = match self.confirmed_by {
            MigrationParty::Administrator => 10,
            MigrationParty::System => 7,
            MigrationParty::LoadBalancer => 6,
            MigrationParty::Server => 5,
            MigrationParty::Client => 3,
        };

        let decision_modifier = match self.decision {
            ConfirmationDecision::Accept => 2,
            ConfirmationDecision::Defer => 1,
            ConfirmationDecision::Reject => 0,
        };

        let auto_modifier = if self.auto_confirmed { 1 } else { 0 };

        let final_score = base_score + decision_modifier + auto_modifier;

        // Apply priority override if present
        if let Some(override_priority) = self.priority_override {
            override_priority.min(15)
        } else {
            final_score.min(15)
        }
    }

    pub fn summary(&self) -> String {
        let decision_str = match self.decision {
            ConfirmationDecision::Accept => "ACCEPTED",
            ConfirmationDecision::Reject => "REJECTED",
            ConfirmationDecision::Defer => "DEFERRED",
        };

        let party_str = match self.confirmed_by {
            MigrationParty::Client => "client",
            MigrationParty::Server => "server",
            MigrationParty::LoadBalancer => "load balancer",
            MigrationParty::Administrator => "administrator",
            MigrationParty::System => "system",
        };

        let auto_str = if self.auto_confirmed { " (auto)" } else { "" };

        let reason_str = self.reason
            .as_ref()
            .map(|r| format!(" - {}", r))
            .unwrap_or_default();

        format!("{} by {}{}{}", decision_str, party_str, auto_str, reason_str)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_new_confirmation() {
        let request_id = Uuid::new_v4();
        let confirmation = MigrationConfirmation::new(
            request_id,
            ConfirmationDecision::Accept,
            MigrationParty::Client,
        );

        assert_eq!(confirmation.request_id, request_id);
        assert_eq!(confirmation.decision, ConfirmationDecision::Accept);
        assert_eq!(confirmation.confirmed_by, MigrationParty::Client);
        assert!(!confirmation.auto_confirmed);
        assert_eq!(confirmation.state, ConfirmationState::Created);
    }

    #[test]
    fn test_factory_methods() {
        let request_id = Uuid::new_v4();

        // Test accept
        let accept = MigrationConfirmation::accept(request_id, MigrationParty::Server);
        assert!(accept.is_acceptance());
        assert_eq!(accept.confirmed_by, MigrationParty::Server);

        // Test reject
        let reject = MigrationConfirmation::reject(
            request_id, 
            MigrationParty::Administrator, 
            "Security policy violation".to_string()
        );
        assert!(reject.is_rejection());
        assert_eq!(reject.reason, Some("Security policy violation".to_string()));

        // Test defer
        let defer = MigrationConfirmation::defer(
            request_id, 
            MigrationParty::LoadBalancer, 
            "High load period".to_string()
        );
        assert!(defer.is_deferral());
        assert_eq!(defer.reason, Some("High load period".to_string()));
    }

    #[test]
    fn test_auto_confirmation() {
        let request_id = Uuid::new_v4();

        let auto_accept = MigrationConfirmation::auto_accept(request_id);
        assert!(auto_accept.is_acceptance());
        assert!(auto_accept.is_auto_confirmed());
        assert_eq!(auto_accept.confirmed_by, MigrationParty::System);

        let auto_reject = MigrationConfirmation::auto_reject(
            request_id, 
            "Resource constraints".to_string()
        );
        assert!(auto_reject.is_rejection());
        assert!(auto_reject.is_auto_confirmed());
        assert_eq!(auto_reject.reason, Some("Resource constraints".to_string()));
    }

    #[test]
    fn test_builder_pattern() {
        let request_id = Uuid::new_v4();
        let timeout_at = SystemTime::now() + Duration::from_secs(300);

        let confirmation = MigrationConfirmation::new(
            request_id,
            ConfirmationDecision::Accept,
            MigrationParty::Client,
        )
        .with_timeout(timeout_at)
        .with_priority_override(8)
        .with_reason("Client requested migration".to_string());

        assert_eq!(confirmation.timeout_at, Some(timeout_at));
        assert_eq!(confirmation.priority_override, Some(8));
        assert_eq!(confirmation.reason, Some("Client requested migration".to_string()));
    }

    #[test]
    fn test_state_transitions() {
        let request_id = Uuid::new_v4();
        let mut confirmation = MigrationConfirmation::accept(request_id, MigrationParty::Server);

        assert_eq!(confirmation.state, ConfirmationState::Created);
        assert!(!confirmation.is_processed());

        confirmation.mark_processed();
        assert_eq!(confirmation.state, ConfirmationState::Processed);
        assert!(confirmation.is_processed());
    }

    #[test]
    fn test_timeout_functionality() {
        let request_id = Uuid::new_v4();
        
        // Test no timeout
        let no_timeout = MigrationConfirmation::accept(request_id, MigrationParty::Client);
        assert!(!no_timeout.is_timed_out());
        assert!(no_timeout.time_until_timeout().is_none());

        // Test future timeout
        let future_timeout = MigrationConfirmation::accept(request_id, MigrationParty::Client)
            .with_timeout(SystemTime::now() + Duration::from_secs(60));
        assert!(!future_timeout.is_timed_out());
        assert!(future_timeout.time_until_timeout().is_some());

        // Test past timeout (would be timed out if it were in the past)
        let past_timeout = MigrationConfirmation::accept(request_id, MigrationParty::Client)
            .with_timeout(SystemTime::now() - Duration::from_secs(60));
        assert!(past_timeout.is_timed_out());
        assert!(past_timeout.time_until_timeout().is_none());
    }

    #[test]
    fn test_validation() {
        let request_id = Uuid::new_v4();

        // Valid acceptance
        let valid_accept = MigrationConfirmation::accept(request_id, MigrationParty::Client);
        assert!(valid_accept.validate().is_ok());

        // Valid rejection with reason
        let valid_reject = MigrationConfirmation::reject(
            request_id, 
            MigrationParty::Server, 
            "Server maintenance".to_string()
        );
        assert!(valid_reject.validate().is_ok());

        // Invalid rejection without reason
        let invalid_reject = MigrationConfirmation::new(
            request_id,
            ConfirmationDecision::Reject,
            MigrationParty::Server,
        );
        assert!(invalid_reject.validate().is_err());

        // Invalid deferral without reason
        let invalid_defer = MigrationConfirmation::new(
            request_id,
            ConfirmationDecision::Defer,
            MigrationParty::LoadBalancer,
        );
        assert!(invalid_defer.validate().is_err());

        // Invalid timeout in the past
        let invalid_timeout = MigrationConfirmation::accept(request_id, MigrationParty::Client)
            .with_timeout(SystemTime::now() - Duration::from_secs(60));
        assert!(invalid_timeout.validate().is_err());

        // Invalid priority override
        let invalid_priority = MigrationConfirmation::accept(request_id, MigrationParty::Client)
            .with_priority_override(15);
        assert!(invalid_priority.validate().is_err());
    }

    #[test]
    fn test_priority_scoring() {
        let request_id = Uuid::new_v4();

        let admin_accept = MigrationConfirmation::accept(request_id, MigrationParty::Administrator);
        let client_reject = MigrationConfirmation::reject(
            request_id, 
            MigrationParty::Client, 
            "Not now".to_string()
        );
        let system_auto = MigrationConfirmation::auto_accept(request_id);

        // Administrator acceptance should have highest score
        assert!(admin_accept.priority_score() > client_reject.priority_score());
        assert!(admin_accept.priority_score() > system_auto.priority_score());

        // Auto confirmations get bonus points
        assert!(system_auto.priority_score() > 7); // base + decision + auto
    }

    #[test]
    fn test_summary_generation() {
        let request_id = Uuid::new_v4();

        let accept = MigrationConfirmation::accept(request_id, MigrationParty::Client);
        assert_eq!(accept.summary(), "ACCEPTED by client");

        let reject = MigrationConfirmation::reject(
            request_id, 
            MigrationParty::Administrator, 
            "Security policy".to_string()
        );
        assert_eq!(reject.summary(), "REJECTED by administrator - Security policy");

        let auto_defer = MigrationConfirmation::defer(
            request_id, 
            MigrationParty::System, 
            "High load".to_string()
        ).as_auto_confirmed();
        assert_eq!(auto_defer.summary(), "DEFERRED by system (auto) - High load");
    }

    #[test]
    fn test_party_checks() {
        let request_id = Uuid::new_v4();

        let client_confirm = MigrationConfirmation::accept(request_id, MigrationParty::Client);
        assert!(client_confirm.is_from_client());
        assert!(!client_confirm.is_from_server());
        assert!(!client_confirm.is_from_admin());

        let admin_confirm = MigrationConfirmation::accept(request_id, MigrationParty::Administrator);
        assert!(!admin_confirm.is_from_client());
        assert!(!admin_confirm.is_from_server());
        assert!(admin_confirm.is_from_admin());
    }

    #[test]
    fn test_retry_logic() {
        let request_id = Uuid::new_v4();

        // Rejections should not suggest retry
        let reject = MigrationConfirmation::reject(
            request_id, 
            MigrationParty::Server, 
            "Permanently unavailable".to_string()
        );
        assert!(!reject.should_retry());

        // Acceptances don't need retry
        let accept = MigrationConfirmation::accept(request_id, MigrationParty::Client);
        assert!(!accept.should_retry());

        // Deferrals should suggest retry (if not timed out)
        let defer = MigrationConfirmation::defer(
            request_id, 
            MigrationParty::LoadBalancer, 
            "Temporary high load".to_string()
        );
        assert!(defer.should_retry());

        // Timed out deferrals should not suggest retry
        let timed_out_defer = MigrationConfirmation::defer(
            request_id, 
            MigrationParty::Server, 
            "Busy".to_string()
        ).with_timeout(SystemTime::now() - Duration::from_secs(60));
        assert!(!timed_out_defer.should_retry());
    }

    #[test]
    fn test_reason_accessors() {
        let request_id = Uuid::new_v4();

        let reject = MigrationConfirmation::reject(
            request_id, 
            MigrationParty::Server, 
            "Server overloaded".to_string()
        );
        assert_eq!(reject.rejection_reason(), Some("Server overloaded"));
        assert_eq!(reject.deferral_reason(), None);

        let defer = MigrationConfirmation::defer(
            request_id, 
            MigrationParty::LoadBalancer, 
            "High traffic period".to_string()
        );
        assert_eq!(defer.rejection_reason(), None);
        assert_eq!(defer.deferral_reason(), Some("High traffic period"));

        let accept = MigrationConfirmation::accept(request_id, MigrationParty::Client);
        assert_eq!(accept.rejection_reason(), None);
        assert_eq!(accept.deferral_reason(), None);
    }
}