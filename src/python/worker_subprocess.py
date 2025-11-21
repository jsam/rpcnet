#!/usr/bin/env python3
"""
Python worker subprocess for RpcNet multi-process server.

This script runs as a separate OS process with its own Python interpreter and GIL.
It communicates with the master Rust process via Unix domain sockets.
"""

import asyncio
import socket
import struct
import sys
import os
import signal
import traceback


class WorkerProcess:
    """Worker process that handles RPC requests via Unix socket"""

    def __init__(self, socket_path: str, worker_id: int):
        self.socket_path = socket_path
        self.worker_id = worker_id
        self.handlers = {}
        self.running = True
        self.loop = None

        # Setup signal handlers
        signal.signal(signal.SIGTERM, self.handle_signal)
        signal.signal(signal.SIGINT, self.handle_signal)

    def handle_signal(self, signum, frame):
        """Handle shutdown signals gracefully"""
        print(f"Worker {self.worker_id}: Received signal {signum}, shutting down...", flush=True)
        self.running = False

    async def handle_init_message(self, sock):
        """Handle initialization message from master (handler registration)"""
        # Read length
        len_bytes = await self.read_exact(sock, 4)
        length = struct.unpack('!I', len_bytes)[0]

        # Read pickled handlers
        data = await self.read_exact(sock, length)

        # Unpickle handlers
        try:
            import cloudpickle as pickle
        except ImportError:
            try:
                import dill as pickle
            except ImportError:
                import pickle

        self.handlers = pickle.loads(data)

        print(f"Worker {self.worker_id}: Registered {len(self.handlers)} handlers", flush=True)

        # Send acknowledgment
        sock.sendall(b'\x00')

    async def handle_request(self, method_name: str, params: bytes) -> tuple[int, bytes]:
        """Handle a single RPC request"""
        handler = self.handlers.get(method_name)
        if not handler:
            error_msg = f"Method '{method_name}' not found"
            print(f"Worker {self.worker_id}: {error_msg}", flush=True)
            return (1, error_msg.encode())

        try:
            # Call the handler (should be async)
            if asyncio.iscoroutinefunction(handler):
                result = await handler(params)
            else:
                result = handler(params)

            return (0, result)
        except Exception as e:
            error_msg = f"Handler error: {str(e)}\n{traceback.format_exc()}"
            print(f"Worker {self.worker_id}: {error_msg}", flush=True)
            return (1, error_msg.encode())

    async def read_exact(self, sock, n: int) -> bytes:
        """Read exactly n bytes from socket"""
        data = b''
        while len(data) < n:
            chunk = await self.loop.sock_recv(sock, n - len(data))
            if not chunk:
                raise ConnectionError("Socket closed")
            data += chunk
        return data

    async def process_requests(self, sock):
        """Main request processing loop"""
        while self.running:
            try:
                # Read method name length (2 bytes)
                method_len_bytes = await self.read_exact(sock, 2)
                method_len = struct.unpack('!H', method_len_bytes)[0]

                # Read method name
                method_name = (await self.read_exact(sock, method_len)).decode('utf-8')

                # Read params length (4 bytes)
                params_len_bytes = await self.read_exact(sock, 4)
                params_len = struct.unpack('!I', params_len_bytes)[0]

                # Read params
                params = await self.read_exact(sock, params_len)

                # Handle request
                status, result = await self.handle_request(method_name, params)

                # Send response: [status: u8] [length: u32] [data]
                response = struct.pack('!B', status)
                response += struct.pack('!I', len(result))
                response += result

                await self.loop.sock_sendall(sock, response)

            except ConnectionError:
                print(f"Worker {self.worker_id}: Connection closed", flush=True)
                break
            except Exception as e:
                print(f"Worker {self.worker_id}: Error processing request: {e}", flush=True)
                print(traceback.format_exc(), flush=True)
                break

    async def run(self):
        """Main worker loop"""
        print(f"Worker {self.worker_id}: Starting on socket {self.socket_path}", flush=True)

        # Create event loop
        self.loop = asyncio.get_event_loop()

        # Create Unix socket
        server_sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        server_sock.bind(self.socket_path)
        server_sock.listen(1)
        server_sock.setblocking(False)

        print(f"Worker {self.worker_id}: Listening on {self.socket_path}", flush=True)

        # Accept connection from master
        conn, _ = await self.loop.sock_accept(server_sock)
        print(f"Worker {self.worker_id}: Master connected", flush=True)

        try:
            # Read first message - should be initialization (0xFF 0xFF header)
            header = await self.read_exact(conn, 2)
            if header == b'\xff\xff':
                print(f"Worker {self.worker_id}: Received init message", flush=True)
                await self.handle_init_message(conn)
            else:
                print(f"Worker {self.worker_id}: WARNING: No init message received", flush=True)

            # Process requests
            await self.process_requests(conn)

        except Exception as e:
            print(f"Worker {self.worker_id}: Fatal error: {e}", flush=True)
            print(traceback.format_exc(), flush=True)
        finally:
            conn.close()
            server_sock.close()
            # Clean up socket file
            try:
                os.unlink(self.socket_path)
            except:
                pass

        print(f"Worker {self.worker_id}: Exiting", flush=True)


def main():
    if len(sys.argv) != 3:
        print("Usage: worker_subprocess.py <socket_path> <worker_id>", flush=True)
        sys.exit(1)

    socket_path = sys.argv[1]
    worker_id = int(sys.argv[2])

    worker = WorkerProcess(socket_path, worker_id)
    asyncio.run(worker.run())


if __name__ == "__main__":
    main()
