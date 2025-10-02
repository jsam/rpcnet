//! Zero-downtime QUIC connection migration
//!
//! This module provides functionality for migrating established QUIC connections
//! between server instances without dropping client connections or requiring
//! re-authentication.

pub mod models;
pub mod types;
pub mod state;
pub mod services;
pub mod health;

mod state_machine;
mod session_manager;
mod request_handler;
mod state_transfer;
mod migration_service_impl;

pub use models::*;
pub use types::*;
pub use state::*;
pub use services::*;
pub use health::*;

pub use state_machine::MigrationStateMachine;
pub use session_manager::ConnectionSessionManager;
pub use request_handler::MigrationRequestHandler;
pub use state_transfer::StateTransferService;
pub use migration_service_impl::MigrationServiceImpl;