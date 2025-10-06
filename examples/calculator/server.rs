#![allow(dead_code)]
#![allow(unused_imports)]
//! Calculator server using generated code.

#[path = "generated/calculator/mod.rs"]
mod calculator;

use calculator::server::{CalculatorHandler, CalculatorServer};
use calculator::{
    AddRequest, AddResponse, CalculatorError, DivideRequest, DivideResponse, MultiplyRequest,
    MultiplyResponse, SubtractRequest, SubtractResponse,
};
use rpcnet::RpcConfig;

struct MyCalculator;

#[async_trait::async_trait]
impl CalculatorHandler for MyCalculator {
    async fn add(&self, request: AddRequest) -> Result<AddResponse, CalculatorError> {
        match request.a.checked_add(request.b) {
            Some(result) => Ok(AddResponse { result }),
            None => Err(CalculatorError::Overflow),
        }
    }

    async fn subtract(
        &self,
        request: SubtractRequest,
    ) -> Result<SubtractResponse, CalculatorError> {
        match request.a.checked_sub(request.b) {
            Some(result) => Ok(SubtractResponse { result }),
            None => Err(CalculatorError::Overflow),
        }
    }

    async fn multiply(
        &self,
        request: MultiplyRequest,
    ) -> Result<MultiplyResponse, CalculatorError> {
        match request.a.checked_mul(request.b) {
            Some(result) => Ok(MultiplyResponse { result }),
            None => Err(CalculatorError::Overflow),
        }
    }

    async fn divide(&self, request: DivideRequest) -> Result<DivideResponse, CalculatorError> {
        if request.divisor == 0.0 {
            return Err(CalculatorError::DivisionByZero);
        }

        let result = request.dividend / request.divisor;
        Ok(DivideResponse { result })
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Calculator Server (Generated Code) ===");

    let config = RpcConfig::new("certs/test_cert.pem", "127.0.0.1:8090")
        .with_key_path("certs/test_key.pem")
        .with_server_name("localhost");

    let server = CalculatorServer::new(MyCalculator, config);
    println!("Starting calculator server on port 8090...");

    server.serve().await?;
    Ok(())
}
