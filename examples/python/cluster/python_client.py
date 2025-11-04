#!/usr/bin/env python3
"""
Python client for RpcNet cluster example.

This demonstrates how to use the generated Python bindings to interact
with the Rust cluster (director).

NOTE: The worker uses streaming RPC which is not yet supported in Python codegen.
This example demonstrates connecting to the director and getting worker information.

Prerequisites:
1. Run the Rust cluster first (see examples/cluster/README.md)
2. Build Python bindings: maturin develop --features python --release
"""

import asyncio
import sys
import os

# Add generated code to path
sys.path.insert(0, os.path.join(os.path.dirname(__file__), 'generated'))

from directorregistry import DirectorRegistryClient, GetWorkerRequest, DirectorError


async def main():
    print("=" * 68)
    print("Python Client for RpcNet Cluster - Director Connection Demo")
    print("=" * 68)
    print()
    print("NOTE: This demonstrates Python generated bindings calling Rust services.")
    print("      The worker uses streaming RPC (not yet supported in Python codegen),")
    print("      so this example only shows connecting to the director.")
    print()

    # Configuration
    DIRECTOR_ADDR = os.getenv("DIRECTOR_ADDR", "127.0.0.1:61000")
    CERT_PATH = os.getenv("CERT_PATH", "../../../certs/test_cert.pem")

    # Resolve cert path relative to this file
    script_dir = os.path.dirname(os.path.abspath(__file__))
    cert_path = os.path.join(script_dir, CERT_PATH)

    if not os.path.exists(cert_path):
        print(f"❌ Certificate not found: {cert_path}")
        print(f"   Generate with:")
        print(f"   mkdir -p certs && cd certs")
        print(f"   openssl req -x509 -newkey rsa:4096 -keyout test_key.pem \\")
        print(f"     -out test_cert.pem -days 365 -nodes -subj '/CN=localhost'")
        return 1

    print(f"📁 Using certificate: {cert_path}")
    print(f"🎯 Director address: {DIRECTOR_ADDR}")
    print()

    try:
        # Step 1: Connect to director
        print("1️⃣  Connecting to director...")
        director = await DirectorRegistryClient.connect(
            DIRECTOR_ADDR,
            cert_path=cert_path,
            server_name="localhost",
            timeout_secs=5,
        )
        print(f"   ✅ Connected to director at {DIRECTOR_ADDR}")
        print()

        # Step 2: Request workers multiple times to test load balancing
        print("2️⃣  Requesting workers (testing load balancing)...")
        for i in range(5):
            try:
                worker_info = await director.get_worker(
                    GetWorkerRequest(
                        connection_id=None,
                        prompt=f"Request {i+1} from Python"
                    )
                )

                if worker_info.success and worker_info.worker_addr:
                    print(f"   Request {i+1}:")
                    print(f"      ✅ Worker: {worker_info.worker_label}")
                    print(f"      📍 Address: {worker_info.worker_addr}")
                    print(f"      🔗 Connection ID: {worker_info.connection_id}")
                else:
                    print(f"   Request {i+1}:")
                    print(f"      ⚠️  {worker_info.message}")

            except Exception as e:
                if "NoWorkersAvailable" in str(e):
                    print(f"   Request {i+1}: ❌ No workers available")
                    if i == 0:
                        print()
                        print("   💡 Start a worker with:")
                        print("      WORKER_ADDR=127.0.0.1:62001 DIRECTOR_ADDR=127.0.0.1:61000 \\")
                        print("        cargo run --manifest-path examples/cluster/Cargo.toml --bin worker")
                    break
                else:
                    print(f"   Request {i+1}: ❌ Error: {e}")

        print()
        print("=" * 68)
        print("✅ Python client completed successfully!")
        print()
        print("What was demonstrated:")
        print("  • Python client connected to Rust director via QUIC+TLS")
        print("  • Generated Python bindings used for type-safe RPC calls")
        print("  • Method name: 'DirectorRegistry.get_worker'")
        print("  • Serialization: MessagePack (Python ↔ Rust)")
        print("  • Transport: QUIC with TLS authentication")
        print()
        print("Generated files:")
        print("  • examples/python/cluster/generated/directorregistry/")
        print("    - types.py      (GetWorkerRequest, GetWorkerResponse, DirectorError)")
        print("    - client.py     (DirectorRegistryClient)")
        print("    - server.py     (DirectorRegistryServer)")
        print("=" * 68)
        return 0

    except ConnectionError as e:
        print()
        print(f"❌ Connection error: {e}")
        print()
        print("💡 Make sure the Rust director is running:")
        print("   DIRECTOR_ADDR=127.0.0.1:61000 RUST_LOG=info \\")
        print("     cargo run --manifest-path examples/cluster/Cargo.toml --bin director")
        return 1
    except Exception as e:
        print()
        print(f"❌ Unexpected error: {e}")
        import traceback
        traceback.print_exc()
        return 1


if __name__ == "__main__":
    exit_code = asyncio.run(main())
    sys.exit(exit_code)
