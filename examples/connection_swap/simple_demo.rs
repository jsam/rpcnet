#!/usr/bin/env rust-script
//! This is a simple demonstration of the migration infrastructure.
//! 
//! Run with: cargo run --example connection_swap/simple_demo

use rpcnet::migration::*;
use std::time::Duration;
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    println!("🚀 rpcnet Migration Infrastructure Demo\n");
    println!("========================================\n");

    // 1. Create a migration service
    println!("1️⃣  Creating migration service...");
    let server_id = Uuid::new_v4();
    let service = MigrationServiceImpl::new(server_id);
    println!("   ✅ Service created with server ID: {}\n", server_id);

    // 2. Set up source and target servers
    println!("2️⃣  Setting up source and target servers...");
    let source = ServerInstance::new("127.0.0.1".to_string(), 8081, 8080);
    let target = ServerInstance::new("127.0.0.2".to_string(), 8081, 8080);
    println!("   ✅ Source: {}:{} (management)", source.address, source.management_port);
    println!("   ✅ Target: {}:{} (management)\n", target.address, target.management_port);

    // 3. Create a migration request
    println!("3️⃣  Creating migration request...");
    let connection_id = Uuid::new_v4();
    let request = MigrationRequest::new(
        connection_id,
        source.clone(),
        target.clone(),
        MigrationReason::LoadBalancing,
        "demo-user".to_string(),
    );
    println!("   ✅ Request created for connection: {}", connection_id);
    println!("   ✅ Reason: LoadBalancing\n");

    // 4. Initiate migration (generates token)
    println!("4️⃣  Initiating migration...");
    let token = service.initiate_migration(request).await?;
    println!("   ✅ Migration token generated");
    println!("   ✅ Token (first 16 chars): {}...", &token.token_hex()[..16]);
    println!("   ✅ Token state: {:?}", token.state);
    println!("   ✅ Expires in: {:?}\n", token.time_until_expiry().unwrap());

    // 5. Demonstrate state machine
    println!("5️⃣  Demonstrating state machine...");
    let sm = MigrationStateMachine::with_default_timeout(token.session_id);
    println!("   Current state: {:?}", sm.get_state().await);
    
    sm.initiate().await?;
    println!("   → Transitioned to: {:?}", sm.get_state().await);
    
    sm.prepare().await?;
    println!("   → Transitioned to: {:?}", sm.get_state().await);
    
    sm.capture_state().await?;
    println!("   → Transitioned to: {:?}", sm.get_state().await);
    
    sm.transfer_state().await?;
    println!("   → Transitioned to: {:?}", sm.get_state().await);
    
    sm.pivot().await?;
    println!("   → Transitioned to: {:?}", sm.get_state().await);
    
    sm.verify().await?;
    println!("   → Transitioned to: {:?}", sm.get_state().await);
    
    sm.complete().await?;
    println!("   → Transitioned to: {:?}", sm.get_state().await);
    println!("   ✅ State machine completed full cycle\n");

    // 6. Confirm migration
    println!("6️⃣  Confirming migration...");
    let confirmation = service.confirm_migration(token.clone()).await?;
    println!("   ✅ Migration confirmed");
    println!("   ✅ Decision: {:?}", confirmation.decision);
    println!("   ✅ By: {:?}\n", confirmation.confirmed_by);

    // 7. Transfer state
    println!("7️⃣  Transferring connection state...");
    let snapshot = service.transfer_state(token).await?;
    let snapshot_size = bincode::serialize(&snapshot)?.len();
    println!("   ✅ State snapshot created");
    println!("   ✅ Snapshot size: {} bytes", snapshot_size);
    println!("   ✅ Session ID: {}\n", snapshot.session_id);

    // 8. Complete migration
    println!("8️⃣  Completing migration...");
    service.complete_migration(confirmation).await?;
    println!("   ✅ Migration completed successfully!\n");

    // 9. Demonstrate session manager
    println!("9️⃣  Demonstrating session manager...");
    let session_manager = ConnectionSessionManager::new();
    let session_id = session_manager
        .create_session(connection_id, Duration::from_secs(30))
        .await?;
    println!("   ✅ Session created: {}", session_id);
    
    session_manager
        .update_session_state(session_id, ConnectionState::Migrating)
        .await?;
    println!("   ✅ Session state updated to: Migrating");
    
    session_manager
        .update_session_state(session_id, ConnectionState::Migrated)
        .await?;
    println!("   ✅ Session state updated to: Migrated");
    
    let session_count = session_manager.count_sessions().await;
    println!("   ✅ Total sessions: {}\n", session_count);

    println!("========================================");
    println!("✨ Demo completed successfully!");
    println!("\nThe migration infrastructure is fully operational.");
    println!("All components tested:");
    println!("  • MigrationService ✓");
    println!("  • MigrationStateMachine ✓");
    println!("  • ConnectionSessionManager ✓");
    println!("  • Token generation & validation ✓");
    println!("  • State transfer ✓");
    println!("\nRun 'cargo test --lib migration' for comprehensive tests.");

    Ok(())
}
