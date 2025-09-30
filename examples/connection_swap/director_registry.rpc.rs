use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerRegistration {
    pub worker_id: String,
    pub address: String,
    pub port: u16,
    pub capabilities: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistrationResponse {
    pub success: bool,
    pub worker_id: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionAssignment {
    pub connection_id: String,
    pub worker_id: String,
    pub stream_id: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssignmentResponse {
    pub success: bool,
    pub connection_id: String,
    pub worker_endpoint: String,
}

service DirectorRegistry {
    rpc RegisterWorker(WorkerRegistration) -> (RegistrationResponse);
    rpc AssignConnection(ConnectionAssignment) -> (AssignmentResponse);
}