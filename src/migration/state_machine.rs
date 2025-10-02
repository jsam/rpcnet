use crate::migration::types::*;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use tokio::sync::RwLock;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MigrationState {
    Idle,
    Initiating,
    WaitingConfirmation,
    Preparing,
    CapturingState,
    TransferringState,
    Pivoting,
    Verifying,
    Completing,
    Completed,
    Aborted,
    Failed,
}

impl Default for MigrationState {
    fn default() -> Self {
        MigrationState::Idle
    }
}

#[derive(Debug, Clone)]
pub struct StateTransition {
    pub from_state: MigrationState,
    pub to_state: MigrationState,
    pub timestamp: SystemTime,
    pub reason: Option<String>,
}

#[derive(Clone)]
pub struct MigrationStateMachine {
    current_state: Arc<RwLock<MigrationState>>,
    session_id: Uuid,
    transitions: Arc<RwLock<Vec<StateTransition>>>,
    created_at: SystemTime,
    timeout: Duration,
}

impl MigrationStateMachine {
    pub fn new(session_id: Uuid, timeout: Duration) -> Self {
        Self {
            current_state: Arc::new(RwLock::new(MigrationState::default())),
            session_id,
            transitions: Arc::new(RwLock::new(Vec::new())),
            created_at: SystemTime::now(),
            timeout,
        }
    }

    pub fn with_default_timeout(session_id: Uuid) -> Self {
        Self::new(session_id, Duration::from_secs(30))
    }

    pub async fn get_state(&self) -> MigrationState {
        self.current_state.read().await.clone()
    }

    pub async fn transition_to(&self, new_state: MigrationState, reason: Option<String>) -> Result<(), String> {
        let mut current = self.current_state.write().await;
        
        if !self.is_valid_transition(&current, &new_state) {
            return Err(format!(
                "Invalid state transition from {:?} to {:?}",
                current, new_state
            ));
        }

        let transition = StateTransition {
            from_state: current.clone(),
            to_state: new_state.clone(),
            timestamp: SystemTime::now(),
            reason,
        };

        self.transitions.write().await.push(transition);
        *current = new_state;
        
        Ok(())
    }

    pub async fn initiate(&self) -> Result<(), String> {
        self.transition_to(MigrationState::Initiating, Some("Migration initiated".to_string())).await
    }

    pub async fn wait_for_confirmation(&self) -> Result<(), String> {
        self.transition_to(MigrationState::WaitingConfirmation, Some("Awaiting confirmation".to_string())).await
    }

    pub async fn prepare(&self) -> Result<(), String> {
        self.transition_to(MigrationState::Preparing, Some("Preparing migration".to_string())).await
    }

    pub async fn capture_state(&self) -> Result<(), String> {
        self.transition_to(MigrationState::CapturingState, Some("Capturing connection state".to_string())).await
    }

    pub async fn transfer_state(&self) -> Result<(), String> {
        self.transition_to(MigrationState::TransferringState, Some("Transferring state".to_string())).await
    }

    pub async fn pivot(&self) -> Result<(), String> {
        self.transition_to(MigrationState::Pivoting, Some("Pivoting connection".to_string())).await
    }

    pub async fn verify(&self) -> Result<(), String> {
        self.transition_to(MigrationState::Verifying, Some("Verifying migration".to_string())).await
    }

    pub async fn complete(&self) -> Result<(), String> {
        self.transition_to(MigrationState::Completing, Some("Completing migration".to_string())).await?;
        self.transition_to(MigrationState::Completed, Some("Migration completed".to_string())).await
    }

    pub async fn abort(&self, reason: String) -> Result<(), String> {
        self.transition_to(MigrationState::Aborted, Some(reason)).await
    }

    pub async fn fail(&self, reason: String) -> Result<(), String> {
        self.transition_to(MigrationState::Failed, Some(reason)).await
    }

    fn is_valid_transition(&self, from: &MigrationState, to: &MigrationState) -> bool {
        use MigrationState::*;

        match (from, to) {
            (Idle, Initiating) => true,
            (Initiating, WaitingConfirmation) => true,
            (Initiating, Preparing) => true,
            (WaitingConfirmation, Preparing) => true,
            (WaitingConfirmation, Aborted) => true,
            (Preparing, CapturingState) => true,
            (Preparing, Aborted) => true,
            (CapturingState, TransferringState) => true,
            (CapturingState, Failed) => true,
            (TransferringState, Pivoting) => true,
            (TransferringState, Failed) => true,
            (Pivoting, Verifying) => true,
            (Pivoting, Failed) => true,
            (Verifying, Completing) => true,
            (Verifying, Failed) => true,
            (Completing, Completed) => true,
            (Completing, Failed) => true,
            
            (_, Aborted) if !matches!(from, Completed | Failed) => true,
            (_, Failed) if !matches!(from, Completed | Aborted) => true,
            
            _ => false,
        }
    }

    pub async fn is_in_progress(&self) -> bool {
        let state = self.get_state().await;
        !matches!(state, MigrationState::Idle | MigrationState::Completed | MigrationState::Aborted | MigrationState::Failed)
    }

    pub async fn is_completed(&self) -> bool {
        matches!(self.get_state().await, MigrationState::Completed)
    }

    pub async fn is_failed(&self) -> bool {
        matches!(self.get_state().await, MigrationState::Failed)
    }

    pub async fn is_aborted(&self) -> bool {
        matches!(self.get_state().await, MigrationState::Aborted)
    }

    pub async fn is_terminal_state(&self) -> bool {
        let state = self.get_state().await;
        matches!(state, MigrationState::Completed | MigrationState::Aborted | MigrationState::Failed)
    }

    pub async fn is_timed_out(&self) -> bool {
        SystemTime::now()
            .duration_since(self.created_at)
            .unwrap_or(Duration::ZERO) > self.timeout
    }

    pub async fn age(&self) -> Duration {
        SystemTime::now()
            .duration_since(self.created_at)
            .unwrap_or(Duration::ZERO)
    }

    pub async fn time_in_current_state(&self) -> Duration {
        let transitions = self.transitions.read().await;
        if let Some(last_transition) = transitions.last() {
            SystemTime::now()
                .duration_since(last_transition.timestamp)
                .unwrap_or(Duration::ZERO)
        } else {
            self.age().await
        }
    }

    pub async fn get_transition_history(&self) -> Vec<StateTransition> {
        self.transitions.read().await.clone()
    }

    pub fn session_id(&self) -> Uuid {
        self.session_id
    }

    pub fn timeout(&self) -> Duration {
        self.timeout
    }

    pub async fn current_phase(&self) -> MigrationPhase {
        match self.get_state().await {
            MigrationState::Idle => MigrationPhase::Initiation,
            MigrationState::Initiating | MigrationState::WaitingConfirmation => MigrationPhase::Initiation,
            MigrationState::Preparing => MigrationPhase::Initiation,
            MigrationState::CapturingState => MigrationPhase::StateCapture,
            MigrationState::TransferringState => MigrationPhase::StateTransfer,
            MigrationState::Pivoting => MigrationPhase::ConnectionEstablishment,
            MigrationState::Verifying => MigrationPhase::Verification,
            MigrationState::Completing | MigrationState::Completed => MigrationPhase::Completion,
            MigrationState::Aborted | MigrationState::Failed => MigrationPhase::Rollback,
        }
    }

    pub async fn summary(&self) -> String {
        let state = self.get_state().await;
        let age = self.age().await;
        let time_in_state = self.time_in_current_state().await;
        
        format!(
            "Migration {} - State: {:?}, Age: {:?}, Time in state: {:?}",
            self.session_id, state, age, time_in_state
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_state_machine_creation() {
        let session_id = Uuid::new_v4();
        let sm = MigrationStateMachine::new(session_id, Duration::from_secs(30));
        
        assert_eq!(sm.session_id(), session_id);
        assert_eq!(sm.get_state().await, MigrationState::Idle);
        assert_eq!(sm.timeout(), Duration::from_secs(30));
    }

    #[tokio::test]
    async fn test_valid_transition_flow() {
        let sm = MigrationStateMachine::with_default_timeout(Uuid::new_v4());
        
        assert!(sm.initiate().await.is_ok());
        assert_eq!(sm.get_state().await, MigrationState::Initiating);
        
        assert!(sm.wait_for_confirmation().await.is_ok());
        assert_eq!(sm.get_state().await, MigrationState::WaitingConfirmation);
        
        assert!(sm.prepare().await.is_ok());
        assert_eq!(sm.get_state().await, MigrationState::Preparing);
        
        assert!(sm.capture_state().await.is_ok());
        assert_eq!(sm.get_state().await, MigrationState::CapturingState);
        
        assert!(sm.transfer_state().await.is_ok());
        assert_eq!(sm.get_state().await, MigrationState::TransferringState);
        
        assert!(sm.pivot().await.is_ok());
        assert_eq!(sm.get_state().await, MigrationState::Pivoting);
        
        assert!(sm.verify().await.is_ok());
        assert_eq!(sm.get_state().await, MigrationState::Verifying);
        
        assert!(sm.complete().await.is_ok());
        assert_eq!(sm.get_state().await, MigrationState::Completed);
    }

    #[tokio::test]
    async fn test_invalid_transitions() {
        let sm = MigrationStateMachine::with_default_timeout(Uuid::new_v4());
        
        assert!(sm.capture_state().await.is_err());
        assert_eq!(sm.get_state().await, MigrationState::Idle);
        
        assert!(sm.complete().await.is_err());
        assert_eq!(sm.get_state().await, MigrationState::Idle);
    }

    #[tokio::test]
    async fn test_abort_from_any_state() {
        let sm = MigrationStateMachine::with_default_timeout(Uuid::new_v4());
        
        sm.initiate().await.unwrap();
        sm.prepare().await.unwrap();
        
        assert!(sm.abort("User requested abort".to_string()).await.is_ok());
        assert_eq!(sm.get_state().await, MigrationState::Aborted);
    }

    #[tokio::test]
    async fn test_failure_from_active_states() {
        let sm = MigrationStateMachine::with_default_timeout(Uuid::new_v4());
        
        sm.initiate().await.unwrap();
        sm.prepare().await.unwrap();
        sm.capture_state().await.unwrap();
        
        assert!(sm.fail("Network error".to_string()).await.is_ok());
        assert_eq!(sm.get_state().await, MigrationState::Failed);
    }

    #[tokio::test]
    async fn test_transition_history() {
        let sm = MigrationStateMachine::with_default_timeout(Uuid::new_v4());
        
        sm.initiate().await.unwrap();
        sm.prepare().await.unwrap();
        sm.capture_state().await.unwrap();
        
        let history = sm.get_transition_history().await;
        assert_eq!(history.len(), 3);
        assert_eq!(history[0].from_state, MigrationState::Idle);
        assert_eq!(history[0].to_state, MigrationState::Initiating);
        assert_eq!(history[2].to_state, MigrationState::CapturingState);
    }

    #[tokio::test]
    async fn test_state_queries() {
        let sm = MigrationStateMachine::with_default_timeout(Uuid::new_v4());
        
        assert!(!sm.is_in_progress().await);
        assert!(!sm.is_completed().await);
        assert!(!sm.is_failed().await);
        
        sm.initiate().await.unwrap();
        assert!(sm.is_in_progress().await);
        
        sm.prepare().await.unwrap();
        sm.capture_state().await.unwrap();
        sm.transfer_state().await.unwrap();
        sm.pivot().await.unwrap();
        sm.verify().await.unwrap();
        sm.complete().await.unwrap();
        assert!(!sm.is_in_progress().await);
        assert!(sm.is_completed().await);
        assert!(sm.is_terminal_state().await);
    }

    #[tokio::test]
    async fn test_phase_mapping() {
        let sm = MigrationStateMachine::with_default_timeout(Uuid::new_v4());
        
        assert_eq!(sm.current_phase().await, MigrationPhase::Initiation);
        
        sm.initiate().await.unwrap();
        sm.prepare().await.unwrap();
        sm.capture_state().await.unwrap();
        assert_eq!(sm.current_phase().await, MigrationPhase::StateCapture);
        
        sm.transfer_state().await.unwrap();
        assert_eq!(sm.current_phase().await, MigrationPhase::StateTransfer);
        
        sm.pivot().await.unwrap();
        assert_eq!(sm.current_phase().await, MigrationPhase::ConnectionEstablishment);
        
        sm.verify().await.unwrap();
        assert_eq!(sm.current_phase().await, MigrationPhase::Verification);
        
        sm.complete().await.unwrap();
        assert_eq!(sm.current_phase().await, MigrationPhase::Completion);
    }

    #[tokio::test]
    async fn test_timeout_detection() {
        let sm = MigrationStateMachine::new(Uuid::new_v4(), Duration::from_millis(10));
        
        assert!(!sm.is_timed_out().await);
        
        tokio::time::sleep(Duration::from_millis(20)).await;
        
        assert!(sm.is_timed_out().await);
    }

    #[tokio::test]
    async fn test_time_tracking() {
        let sm = MigrationStateMachine::with_default_timeout(Uuid::new_v4());
        
        tokio::time::sleep(Duration::from_millis(10)).await;
        
        let age = sm.age().await;
        assert!(age >= Duration::from_millis(10));
        
        sm.initiate().await.unwrap();
        tokio::time::sleep(Duration::from_millis(10)).await;
        
        let time_in_state = sm.time_in_current_state().await;
        assert!(time_in_state >= Duration::from_millis(10));
    }

    #[tokio::test]
    async fn test_cannot_transition_from_terminal_states() {
        let sm = MigrationStateMachine::with_default_timeout(Uuid::new_v4());
        
        sm.initiate().await.unwrap();
        sm.prepare().await.unwrap();
        sm.capture_state().await.unwrap();
        sm.transfer_state().await.unwrap();
        sm.pivot().await.unwrap();
        sm.verify().await.unwrap();
        sm.complete().await.unwrap();
        
        assert!(sm.initiate().await.is_err());
        assert!(sm.prepare().await.is_err());
        assert!(sm.abort("test".to_string()).await.is_err());
    }
}
