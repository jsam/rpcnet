use rpcnet::RpcRequest;
use serde::Serialize;

fn main() {
    let req = RpcRequest::new(
        1,
        "DirectorRegistry.get_worker".to_string(),
        vec![0x82, 0xa6],
    );

    // Test with struct_map (what we're using now)
    let mut buf = Vec::new();
    req.serialize(&mut rmp_serde::Serializer::new(&mut buf).with_struct_map())
        .unwrap();

    println!("✅ RpcRequest with struct_map:");
    println!("   Length: {} bytes", buf.len());
    println!("   First 20 bytes: {:?}", &buf[..buf.len().min(20)]);
    println!("   First byte: 0x{:02x}", buf[0]);

    if buf[0] == 0x83 {
        println!("   ✅ Correctly starts with 0x83 (3-element map for RpcRequest)");
    } else {
        println!("   ❌ Does NOT start with map marker!");
    }

    // Deserialize it back
    match rmp_serde::from_slice::<RpcRequest>(&buf) {
        Ok(decoded) => println!(
            "   ✅ Successfully deserialized: method={}",
            decoded.method()
        ),
        Err(e) => println!("   ❌ Deserialization failed: {:?}", e),
    }
}
