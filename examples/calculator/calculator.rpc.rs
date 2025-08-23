//! Calculator service definition.

use serde::{Serialize, Deserialize};

/// Request for addition operation.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AddRequest {
    pub a: i64,
    pub b: i64,
}

/// Response from addition operation.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AddResponse {
    pub result: i64,
}

/// Request for subtraction operation.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SubtractRequest {
    pub a: i64,
    pub b: i64,
}

/// Response from subtraction operation.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SubtractResponse {
    pub result: i64,
}

/// Request for multiplication operation.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MultiplyRequest {
    pub a: i64,
    pub b: i64,
}

/// Response from multiplication operation.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MultiplyResponse {
    pub result: i64,
}

/// Request for division operation.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DivideRequest {
    pub dividend: f64,
    pub divisor: f64,
}

/// Response from division operation.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DivideResponse {
    pub result: f64,
}

/// Errors that can occur in calculator operations.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum CalculatorError {
    /// Division by zero attempted.
    DivisionByZero,
    /// Integer overflow occurred.
    Overflow,
    /// Invalid input provided.
    InvalidInput(String),
}

/// Calculator service with basic arithmetic operations.
#[rpcnet::service]
pub trait Calculator {
    async fn add(&self, request: AddRequest) -> Result<AddResponse, CalculatorError>;
    async fn subtract(&self, request: SubtractRequest) -> Result<SubtractResponse, CalculatorError>;
    async fn multiply(&self, request: MultiplyRequest) -> Result<MultiplyResponse, CalculatorError>;
    async fn divide(&self, request: DivideRequest) -> Result<DivideResponse, CalculatorError>;
}