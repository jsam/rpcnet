use crate::migration::models::*;
use crate::migration::state_machine::MigrationStateMachine;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use uuid::Uuid;

pub struct ConnectionSessionManager {
    sessions: Arc<RwLock<HashMap<Uuid, ConnectionSession>>>,
    state_machines: Arc<RwLock<HashMap<Uuid, MigrationStateMachine>>>,
}

impl ConnectionSessionManager {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            state_machines: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn create_session(
        &self,
        connection_id: Uuid,
        timeout: Duration,
    ) -> Result<Uuid, String> {
        let session = ConnectionSession::new(connection_id);
        let session_id = session.session_id();
        
        let state_machine = MigrationStateMachine::new(session_id, timeout);
        
        self.sessions.write().await.insert(session_id, session);
        self.state_machines.write().await.insert(session_id, state_machine);
        
        Ok(session_id)
    }

    pub async fn get_session(&self, session_id: Uuid) -> Option<ConnectionSession> {
        self.sessions.read().await.get(&session_id).cloned()
    }

    pub async fn update_session(&self, session: ConnectionSession) -> Result<(), String> {
        let session_id = session.session_id();
        
        let mut sessions = self.sessions.write().await;
        if sessions.contains_key(&session_id) {
            sessions.insert(session_id, session);
            Ok(())
        } else {
            Err(format!("Session {} not found", session_id))
        }
    }

    pub async fn get_state_machine(&self, session_id: Uuid) -> Option<MigrationStateMachine> {
        self.state_machines.read().await.get(&session_id).cloned()
    }

    pub async fn remove_session(&self, session_id: Uuid) -> Option<ConnectionSession> {
        self.state_machines.write().await.remove(&session_id);
        self.sessions.write().await.remove(&session_id)
    }

    pub async fn session_exists(&self, session_id: Uuid) -> bool {
        self.sessions.read().await.contains_key(&session_id)
    }

    pub async fn list_sessions(&self) -> Vec<Uuid> {
        self.sessions.read().await.keys().cloned().collect()
    }

    pub async fn count_sessions(&self) -> usize {
        self.sessions.read().await.len()
    }

    pub async fn list_active_sessions(&self) -> Vec<Uuid> {
        let sessions = self.sessions.read().await;
        sessions
            .iter()
            .filter(|(_, session)| session.state() == &ConnectionState::Active)
            .map(|(id, _)| *id)
            .collect()
    }

    pub async fn list_migrating_sessions(&self) -> Vec<Uuid> {
        let sessions = self.sessions.read().await;
        sessions
            .iter()
            .filter(|(_, session)| session.state() == &ConnectionState::Migrating)
            .map(|(id, _)| *id)
            .collect()
    }

    pub async fn cleanup_completed_sessions(&self, retention_duration: Duration) -> usize {
        let now = std::time::SystemTime::now();
        let mut sessions_to_remove = Vec::new();

        let sessions = self.sessions.read().await;
        for (session_id, session) in sessions.iter() {
            if matches!(session.state(), ConnectionState::Migrated | ConnectionState::Failed | ConnectionState::Closed) {
                if let Ok(elapsed) = now.duration_since(session.updated_at()) {
                    if elapsed > retention_duration {
                        sessions_to_remove.push(*session_id);
                    }
                }
            }
        }
        drop(sessions);

        let removed_count = sessions_to_remove.len();
        for session_id in sessions_to_remove {
            self.remove_session(session_id).await;
        }

        removed_count
    }

    pub async fn get_session_metrics(&self, session_id: Uuid) -> Option<ConnectionMetrics> {
        self.sessions.read().await
            .get(&session_id)
            .map(|session| session.metrics().clone())
    }

    pub async fn update_session_state(
        &self,
        session_id: Uuid,
        new_state: ConnectionState,
    ) -> Result<(), String> {
        let mut sessions = self.sessions.write().await;
        
        if let Some(session) = sessions.get_mut(&session_id) {
            *session = session.clone().with_state(new_state);
            Ok(())
        } else {
            Err(format!("Session {} not found", session_id))
        }
    }

    pub async fn update_session_stage(
        &self,
        session_id: Uuid,
        new_stage: MigrationStage,
    ) -> Result<(), String> {
        let mut sessions = self.sessions.write().await;
        
        if let Some(session) = sessions.get_mut(&session_id) {
            *session = session.clone().with_migration_stage(new_stage);
            Ok(())
        } else {
            Err(format!("Session {} not found", session_id))
        }
    }

    pub async fn increment_retry_count(&self, session_id: Uuid) -> Result<u32, String> {
        let mut sessions = self.sessions.write().await;
        
        if let Some(session) = sessions.get_mut(&session_id) {
            let new_count = session.retry_count() + 1;
            *session = session.clone().with_retry_count(new_count);
            Ok(new_count)
        } else {
            Err(format!("Session {} not found", session_id))
        }
    }
}

impl Default for ConnectionSessionManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_create_and_get_session() {
        let manager = ConnectionSessionManager::new();
        let connection_id = Uuid::new_v4();
        
        let session_id = manager.create_session(connection_id, Duration::from_secs(30)).await.unwrap();
        
        assert!(manager.session_exists(session_id).await);
        
        let session = manager.get_session(session_id).await.unwrap();
        assert_eq!(session.connection_id(), connection_id);
        assert_eq!(session.session_id(), session_id);
    }

    #[tokio::test]
    async fn test_update_session() {
        let manager = ConnectionSessionManager::new();
        let connection_id = Uuid::new_v4();
        
        let session_id = manager.create_session(connection_id, Duration::from_secs(30)).await.unwrap();
        
        let mut session = manager.get_session(session_id).await.unwrap();
        session = session.with_state(ConnectionState::Migrating);
        
        assert!(manager.update_session(session).await.is_ok());
        
        let updated_session = manager.get_session(session_id).await.unwrap();
        assert_eq!(updated_session.state(), &ConnectionState::Migrating);
    }

    #[tokio::test]
    async fn test_remove_session() {
        let manager = ConnectionSessionManager::new();
        let connection_id = Uuid::new_v4();
        
        let session_id = manager.create_session(connection_id, Duration::from_secs(30)).await.unwrap();
        assert!(manager.session_exists(session_id).await);
        
        let removed = manager.remove_session(session_id).await;
        assert!(removed.is_some());
        assert!(!manager.session_exists(session_id).await);
    }

    #[tokio::test]
    async fn test_list_sessions() {
        let manager = ConnectionSessionManager::new();
        
        let session_id_1 = manager.create_session(Uuid::new_v4(), Duration::from_secs(30)).await.unwrap();
        let session_id_2 = manager.create_session(Uuid::new_v4(), Duration::from_secs(30)).await.unwrap();
        
        let sessions = manager.list_sessions().await;
        assert_eq!(sessions.len(), 2);
        assert!(sessions.contains(&session_id_1));
        assert!(sessions.contains(&session_id_2));
    }

    #[tokio::test]
    async fn test_count_sessions() {
        let manager = ConnectionSessionManager::new();
        
        assert_eq!(manager.count_sessions().await, 0);
        
        manager.create_session(Uuid::new_v4(), Duration::from_secs(30)).await.unwrap();
        assert_eq!(manager.count_sessions().await, 1);
        
        manager.create_session(Uuid::new_v4(), Duration::from_secs(30)).await.unwrap();
        assert_eq!(manager.count_sessions().await, 2);
    }

    #[tokio::test]
    async fn test_list_active_sessions() {
        let manager = ConnectionSessionManager::new();
        
        let session_id_1 = manager.create_session(Uuid::new_v4(), Duration::from_secs(30)).await.unwrap();
        let session_id_2 = manager.create_session(Uuid::new_v4(), Duration::from_secs(30)).await.unwrap();
        
        manager.update_session_state(session_id_1, ConnectionState::Active).await.unwrap();
        manager.update_session_state(session_id_2, ConnectionState::Migrating).await.unwrap();
        
        let active_sessions = manager.list_active_sessions().await;
        assert_eq!(active_sessions.len(), 1);
        assert!(active_sessions.contains(&session_id_1));
    }

    #[tokio::test]
    async fn test_list_migrating_sessions() {
        let manager = ConnectionSessionManager::new();
        
        let session_id_1 = manager.create_session(Uuid::new_v4(), Duration::from_secs(30)).await.unwrap();
        let session_id_2 = manager.create_session(Uuid::new_v4(), Duration::from_secs(30)).await.unwrap();
        
        manager.update_session_state(session_id_1, ConnectionState::Migrating).await.unwrap();
        manager.update_session_state(session_id_2, ConnectionState::Active).await.unwrap();
        
        let migrating_sessions = manager.list_migrating_sessions().await;
        assert_eq!(migrating_sessions.len(), 1);
        assert!(migrating_sessions.contains(&session_id_1));
    }

    #[tokio::test]
    async fn test_get_state_machine() {
        let manager = ConnectionSessionManager::new();
        let connection_id = Uuid::new_v4();
        
        let session_id = manager.create_session(connection_id, Duration::from_secs(30)).await.unwrap();
        
        let state_machine = manager.get_state_machine(session_id).await;
        assert!(state_machine.is_some());
        
        let sm = state_machine.unwrap();
        assert_eq!(sm.session_id(), session_id);
    }

    #[tokio::test]
    async fn test_update_session_stage() {
        let manager = ConnectionSessionManager::new();
        let connection_id = Uuid::new_v4();
        
        let session_id = manager.create_session(connection_id, Duration::from_secs(30)).await.unwrap();
        
        assert!(manager.update_session_stage(session_id, MigrationStage::Preparing).await.is_ok());
        
        let session = manager.get_session(session_id).await.unwrap();
        assert_eq!(session.migration_stage(), &MigrationStage::Preparing);
    }

    #[tokio::test]
    async fn test_increment_retry_count() {
        let manager = ConnectionSessionManager::new();
        let connection_id = Uuid::new_v4();
        
        let session_id = manager.create_session(connection_id, Duration::from_secs(30)).await.unwrap();
        
        let retry_count_1 = manager.increment_retry_count(session_id).await.unwrap();
        assert_eq!(retry_count_1, 1);
        
        let retry_count_2 = manager.increment_retry_count(session_id).await.unwrap();
        assert_eq!(retry_count_2, 2);
        
        let session = manager.get_session(session_id).await.unwrap();
        assert_eq!(session.retry_count(), 2);
    }

    #[tokio::test]
    async fn test_cleanup_completed_sessions() {
        let manager = ConnectionSessionManager::new();
        
        let session_id_1 = manager.create_session(Uuid::new_v4(), Duration::from_secs(30)).await.unwrap();
        let session_id_2 = manager.create_session(Uuid::new_v4(), Duration::from_secs(30)).await.unwrap();
        
        manager.update_session_state(session_id_1, ConnectionState::Migrated).await.unwrap();
        manager.update_session_state(session_id_2, ConnectionState::Active).await.unwrap();
        
        tokio::time::sleep(Duration::from_millis(100)).await;
        
        let removed_count = manager.cleanup_completed_sessions(Duration::from_millis(50)).await;
        assert_eq!(removed_count, 1);
        assert!(!manager.session_exists(session_id_1).await);
        assert!(manager.session_exists(session_id_2).await);
    }

    #[tokio::test]
    async fn test_get_session_metrics() {
        let manager = ConnectionSessionManager::new();
        let connection_id = Uuid::new_v4();
        
        let session_id = manager.create_session(connection_id, Duration::from_secs(30)).await.unwrap();
        
        let metrics = manager.get_session_metrics(session_id).await;
        assert!(metrics.is_some());
    }
}