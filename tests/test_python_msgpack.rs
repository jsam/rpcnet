// Test to verify Python MessagePack serialization compatibility
use rpcnet::{RpcRequest, RpcResponse};

#[test]
fn test_python_rpc_request_serialization() {
    // Create the same params that Python would send (28 bytes)
    // This is GetWorkerRequest { connection_id: None, prompt: "Test" }
    let params = vec![
        0x82, 0xad, 0x63, 0x6f, 0x6e, 0x6e, 0x65, 0x63, 0x74, 0x69, 0x6f, 0x6e, 0x5f, 0x69, 0x64,
        0xc0, 0xa6, 0x70, 0x72, 0x6f, 0x6d, 0x70, 0x74, 0xa4, 0x54, 0x65, 0x73, 0x74,
    ];

    let req = RpcRequest::new(0, "DirectorRegistry.get_worker".to_string(), params);
    let serialized = rmp_serde::to_vec(&req).unwrap();

    println!("RpcRequest serialized to {} bytes", serialized.len());
    println!("Hex: {}", hex::encode(&serialized));

    // Try to deserialize it back
    let deser = rmp_serde::from_slice::<RpcRequest>(&serialized).unwrap();
    assert_eq!(deser.method(), "DirectorRegistry.get_worker");
    assert_eq!(deser.id(), 0);
    assert_eq!(deser.params().len(), 28);
}

#[test]
fn test_rpc_response_serialization() {
    let response = RpcResponse::new(0, Some(vec![1, 2, 3]), None);
    let serialized = rmp_serde::to_vec(&response).unwrap();

    println!("RpcResponse serialized to {} bytes", serialized.len());
    println!("Hex: {}", hex::encode(&serialized));

    let deser = rmp_serde::from_slice::<RpcResponse>(&serialized).unwrap();
    assert_eq!(deser.id(), 0);
    assert!(deser.result().is_some());
    assert!(deser.error().is_none());
}
