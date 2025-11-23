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

        # Read JSON data containing handler source code
        data = await self.read_exact(sock, length)

        # Setup sys.path to include parent directories (for imports like benchmarkservice.types)
        import sys
        import os
        import json
        
        # Add current working directory and its subdirectories
        cwd = os.getcwd()
        if cwd not in sys.path:
            sys.path.insert(0, cwd)
        
        # Add generated directory if it exists
        generated_path = os.path.join(cwd, 'generated')
        if os.path.exists(generated_path) and generated_path not in sys.path:
            sys.path.insert(0, generated_path)

        # Parse JSON to get handler data (source + pickled handler)
        try:
            handlers_data = json.loads(data.decode('utf-8'))
        except Exception as e:
            print(f"Worker {self.worker_id}: ERROR: Failed to parse handler JSON: {e}", flush=True)
            sock.sendall(b'\x01')  # Error ack
            return

        # Unpickle the handler instance if available
        handler_instance = None
        first_method_data = next(iter(handlers_data.values()), None)
        print(f"Worker {self.worker_id}: First method data type: {type(first_method_data)}, keys: {list(first_method_data.keys()) if isinstance(first_method_data, dict) else 'not a dict'}", flush=True)
        if first_method_data and isinstance(first_method_data, dict) and 'handler' in first_method_data:
            try:
                import cloudpickle
                import base64
                pickled_b64 = first_method_data['handler']
                pickled_bytes = base64.b64decode(pickled_b64)
                handler_instance = cloudpickle.loads(pickled_bytes)
                print(f"Worker {self.worker_id}: Successfully unpickled handler instance: {type(handler_instance).__name__}", flush=True)
            except Exception as e:
                print(f"Worker {self.worker_id}: Warning: Failed to unpickle handler: {e}", flush=True)
                import traceback
                traceback.print_exc()
        
        # Create fake 'self' object once for all handlers if we have a handler instance
        fake_self = None
        if handler_instance is not None:
            class FakeSelf:
                pass
            fake_self = FakeSelf()
            fake_self.handler = handler_instance

        # Compile and register each handler
        for method_name, method_data in handlers_data.items():
            # Extract source code (handle both old string format and new dict format)
            if isinstance(method_data, dict):
                source_code = method_data.get('source', '')
            else:
                source_code = method_data
            try:
                # Create a namespace for the handler with common imports
                handler_globals = {
                    '__builtins__': __builtins__,
                    'asyncio': asyncio,
                }
                
                # Import rpcnet module
                try:
                    import rpcnet
                    handler_globals['rpcnet'] = rpcnet
                except ImportError:
                    pass
                
                # Try to infer the service module from method_name (e.g., "BenchmarkService.noop" -> "benchmarkservice")
                if '.' in method_name:
                    service_name = method_name.split('.')[0].lower()
                    try:
                        # Import the service types module (e.g., "benchmarkservice.types")
                        types_module = __import__(f"{service_name}.types", fromlist=['*'])
                        # Import all exported names from types module
                        for name in dir(types_module):
                            if not name.startswith('_'):
                                handler_globals[name] = getattr(types_module, name)
                    except ImportError as e:
                        print(f"Worker {self.worker_id}: Warning: Could not import {service_name}.types: {e}", flush=True)
                
                # If we have a fake_self, add it to globals
                if fake_self is not None:
                    handler_globals['self'] = fake_self
                
                # If we have a handler instance, add it to globals as 'handler'
                if handler_instance is not None:
                    handler_globals['handler'] = handler_instance
                
                # Execute the source code to define the function
                exec(source_code, handler_globals)
                
                # Extract the function name from the source code
                # Handler functions are typically named handle_xxx
                import re
                func_match = re.search(r'async def (\w+)\s*\(', source_code)
                if not func_match:
                    print(f"Worker {self.worker_id}: ERROR: Could not find function name in source for {method_name}", flush=True)
                    continue
                
                func_name = func_match.group(1)
                
                # Get the compiled function
                if func_name in handler_globals:
                    self.handlers[method_name] = handler_globals[func_name]
                    print(f"Worker {self.worker_id}: Registered handler '{method_name}' ({func_name})", flush=True)
                else:
                    print(f"Worker {self.worker_id}: ERROR: Function '{func_name}' not found in compiled code", flush=True)
                    
            except Exception as e:
                print(f"Worker {self.worker_id}: ERROR: Failed to compile handler for {method_name}: {e}", flush=True)
                print(f"Source code:\n{source_code}", flush=True)
                traceback.print_exc()

        print(f"Worker {self.worker_id}: Successfully registered {len(self.handlers)} handlers", flush=True)

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
        print("Usage: python -m rpcnet.worker <socket_path> <worker_id>", flush=True)
        sys.exit(1)

    socket_path = sys.argv[1]
    worker_id = int(sys.argv[2])

    worker = WorkerProcess(socket_path, worker_id)
    asyncio.run(worker.run())


if __name__ == "__main__":
    main()
