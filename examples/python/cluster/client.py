#!/usr/bin/env python3
"""
Proper cluster test client that connects through the director.

This demonstrates the correct cluster architecture:
1. Client connects to Director
2. Director assigns a worker using SWIM cluster membership
3. Client connects directly to the assigned worker
4. Client uses that worker for inference requests

This matches the Rust cluster example architecture.
"""

import asyncio
import sys
import os

# Add generated code to path
sys.path.insert(0, os.path.dirname(__file__))

from directorregistry.client import DirectorRegistryClient
from directorregistry.types import GetWorkerRequest
from inference.client import InferenceClient
from inference.types import InferenceRequest


async def main():
    """Test proper cluster flow through director"""
    # Configuration
    director_addr = os.getenv("DIRECTOR_ADDR", "127.0.0.1:61000")
    cert_path = os.getenv("CERT_PATH", "certs/test_cert.pem")

    print("=" * 70)
    print("🧪 Cluster Test Client (via Director)")
    print("=" * 70)
    print(f"Director Address: {director_addr}")
    print(f"Certificate: {cert_path}")
    print("=" * 70)
    print()

    connection_id = None

    for request_num in range(1, 6):
        print(f"🔍 Request #{request_num}: Asking director for worker assignment...")
        
        # Step 1: Connect to director
        director_client = await DirectorRegistryClient.connect(
            addr=director_addr,
            cert_path=cert_path,
            server_name="localhost",
            timeout_secs=5
        )
        print(f"✅ Connected to director")

        # Step 2: Request worker assignment
        get_worker_req = GetWorkerRequest(
            connection_id=connection_id,
            prompt=f"test-prompt-{request_num}"
        )

        try:
            response = await director_client.get_worker(get_worker_req)
            
            if not response.success:
                print(f"❌ Director error: {response.message}")
                print(f"⏳ Waiting 2 seconds before retry...")
                await asyncio.sleep(2)
                continue

            worker_addr = response.worker_addr
            worker_label = response.worker_label
            connection_id = response.connection_id

            print(f"🔀 Director assigned worker:")
            print(f"   Worker: {worker_label}")
            print(f"   Address: {worker_addr}")
            print(f"   Connection ID: {connection_id}")

            # Step 3: Connect directly to assigned worker
            print(f"🔌 Establishing direct connection to worker...")
            worker_client = await InferenceClient.connect(
                addr=worker_addr,
                cert_path=cert_path,
                server_name="localhost",
                timeout_secs=5
            )
            print(f"✅ Direct connection established to worker")

            # Step 4: Make inference request
            inference_request = InferenceRequest(
                connection_id=connection_id,
                prompt=f"Hello from cluster client, request #{request_num}!"
            )
            
            print(f"📤 Sending inference request to worker...")
            inference_response = await worker_client.infer(inference_request)
            
            print(f"📥 Response from worker:")
            print(f"   {inference_response.text}")
            print(f"   (Sequence: {inference_response.sequence})")
            print()

        except Exception as e:
            print(f"❌ Error: {e}")
            import traceback
            traceback.print_exc()
            print()
            await asyncio.sleep(2)
            continue

        # Small delay between requests
        await asyncio.sleep(1)

    print("=" * 70)
    print("✅ Cluster test completed!")
    print("=" * 70)
    print()
    print("🎯 Architecture demonstrated:")
    print("   1. Client → Director (get worker assignment via SWIM)")
    print("   2. Director → Worker selection (load balancing)")
    print("   3. Client → Worker (direct connection for inference)")
    print("=" * 70)


if __name__ == "__main__":
    asyncio.run(main())
