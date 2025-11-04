#!/usr/bin/env python3
"""
Python-based test runner for RpcNet Python bindings.

This script:
1. Checks prerequisites (pytest, certificates)
2. Builds the Python module
3. Runs the test suite
4. Reports results
"""

import sys
import subprocess
import os
import shutil
from pathlib import Path


class Colors:
    """ANSI color codes for terminal output."""
    RED = '\033[0;31m'
    GREEN = '\033[0;32m'
    YELLOW = '\033[1;33m'
    NC = '\033[0m'  # No Color


def print_colored(message, color):
    """Print colored message to terminal."""
    print(f"{color}{message}{Colors.NC}")


def has_uv():
    """Check if uv is available."""
    return shutil.which("uv") is not None


def setup_with_uv():
    """Setup environment using UV."""
    print_colored("Using UV for faster package management ⚡", Colors.YELLOW)

    # Create venv if it doesn't exist
    if not Path(".venv").exists():
        print_colored("Creating virtual environment with UV...", Colors.YELLOW)
        try:
            subprocess.run(["uv", "venv"], check=True)
            print(f"{Colors.GREEN}✓ Virtual environment created{Colors.NC}")
        except subprocess.CalledProcessError as e:
            print_colored(f"✗ Failed to create venv: {e}", Colors.RED)
            return False
    else:
        print(f"{Colors.GREEN}✓ Virtual environment exists{Colors.NC}")

    # Install dependencies
    print_colored("Installing dependencies with UV...", Colors.YELLOW)
    try:
        subprocess.run([
            "uv", "pip", "install",
            "-r", "python_tests/requirements.txt"
        ], check=True)
        print(f"{Colors.GREEN}✓ Dependencies installed{Colors.NC}")
    except subprocess.CalledProcessError as e:
        print_colored(f"✗ Failed to install dependencies: {e}", Colors.RED)
        return False

    return True


def build_module_with_uv():
    """Build the Python module using UV."""
    print_colored("\nBuilding Python module with UV...", Colors.YELLOW)

    try:
        result = subprocess.run([
            "uv", "run", "maturin", "develop",
            "--features", "python"
        ], capture_output=True, text=True)

        if result.returncode == 0:
            print(f"{Colors.GREEN}✓ Module built with maturin (via UV){Colors.NC}")
            return True
        else:
            print_colored(f"Maturin build failed: {result.stderr}", Colors.RED)
            return False
    except FileNotFoundError:
        print_colored("✗ Maturin not found in UV environment", Colors.RED)
        return False


def run_tests_with_uv(extra_args=None):
    """Run tests using UV."""
    print_colored("\nRunning tests with UV...", Colors.YELLOW)
    print()

    # Build pytest command
    cmd = [
        "uv", "run", "pytest",
        "python_tests/",
        "-v",
        "--tb=short",
        "--asyncio-mode=auto",
    ]

    # Add any extra arguments
    if extra_args:
        cmd.extend(extra_args)

    # Run tests
    result = subprocess.run(cmd)

    print()
    if result.returncode == 0:
        print_colored("=" * 40, Colors.GREEN)
        print_colored("✓ All tests passed!", Colors.GREEN)
        print_colored("=" * 40, Colors.GREEN)
        return True
    else:
        print_colored("=" * 40, Colors.RED)
        print_colored("✗ Some tests failed", Colors.RED)
        print_colored("=" * 40, Colors.RED)
        return False


def check_prerequisites():
    """Check that all prerequisites are installed."""
    print_colored("Checking prerequisites...", Colors.YELLOW)

    # Check pytest
    try:
        import pytest
        print(f"{Colors.GREEN}✓ pytest is installed (version {pytest.__version__}){Colors.NC}")
    except ImportError:
        print_colored("✗ pytest is not installed", Colors.RED)
        print("Install with: pip install pytest pytest-asyncio")
        return False

    # Check pytest-asyncio
    try:
        import pytest_asyncio
        print(f"{Colors.GREEN}✓ pytest-asyncio is installed{Colors.NC}")
    except ImportError:
        print_colored("✗ pytest-asyncio is not installed", Colors.RED)
        print("Install with: pip install pytest-asyncio")
        return False

    return True


def generate_certificates():
    """Generate test certificates if they don't exist."""
    cert_path = Path("certs/test_cert.pem")
    key_path = Path("certs/test_key.pem")

    if cert_path.exists() and key_path.exists():
        print(f"{Colors.GREEN}✓ Test certificates exist{Colors.NC}")
        return True

    print_colored("Generating test certificates...", Colors.YELLOW)

    # Create certs directory
    cert_path.parent.mkdir(exist_ok=True)

    # Generate self-signed certificate
    try:
        subprocess.run([
            "openssl", "req", "-x509", "-newkey", "rsa:4096",
            "-keyout", str(key_path),
            "-out", str(cert_path),
            "-days", "365",
            "-nodes",
            "-subj", "/CN=localhost"
        ], check=True, capture_output=True)

        print(f"{Colors.GREEN}✓ Certificates generated{Colors.NC}")
        return True
    except subprocess.CalledProcessError as e:
        print_colored(f"✗ Failed to generate certificates: {e}", Colors.RED)
        return False
    except FileNotFoundError:
        print_colored("✗ OpenSSL not found. Please install OpenSSL.", Colors.RED)
        return False


def build_module():
    """Build the Python module."""
    print_colored("\nBuilding Python module...", Colors.YELLOW)

    # Try maturin first
    try:
        result = subprocess.run(
            ["maturin", "develop", "--features", "python"],
            capture_output=True,
            text=True
        )

        if result.returncode == 0:
            print(f"{Colors.GREEN}✓ Module built with maturin{Colors.NC}")
            return True
        else:
            print_colored(f"Maturin build failed: {result.stderr}", Colors.RED)
            return False

    except FileNotFoundError:
        # Maturin not installed, try cargo
        print_colored("Maturin not found, trying cargo build...", Colors.YELLOW)
        try:
            result = subprocess.run(
                ["cargo", "build", "--release", "--features", "python"],
                capture_output=True,
                text=True
            )

            if result.returncode == 0:
                print(f"{Colors.GREEN}✓ Module built with cargo{Colors.NC}")
                print(f"{Colors.YELLOW}Note: Install maturin for better integration: pip install maturin{Colors.NC}")
                return True
            else:
                print_colored(f"Cargo build failed: {result.stderr}", Colors.RED)
                return False

        except FileNotFoundError:
            print_colored("✗ Neither maturin nor cargo found", Colors.RED)
            return False


def run_tests(extra_args=None):
    """Run the test suite."""
    print_colored("\nRunning tests...", Colors.YELLOW)
    print()

    # Build pytest command
    cmd = [
        sys.executable, "-m", "pytest",
        "python_tests/",
        "-v",
        "--tb=short",
        "--asyncio-mode=auto",
    ]

    # Add any extra arguments passed to this script
    if extra_args:
        cmd.extend(extra_args)

    # Run tests
    result = subprocess.run(cmd)

    print()
    if result.returncode == 0:
        print_colored("=" * 40, Colors.GREEN)
        print_colored("✓ All tests passed!", Colors.GREEN)
        print_colored("=" * 40, Colors.GREEN)
        return True
    else:
        print_colored("=" * 40, Colors.RED)
        print_colored("✗ Some tests failed", Colors.RED)
        print_colored("=" * 40, Colors.RED)
        return False


def main():
    """Main entry point."""
    print_colored("=" * 40, Colors.YELLOW)
    print_colored("RpcNet Python Bindings Test Runner", Colors.YELLOW)
    print_colored("=" * 40, Colors.YELLOW)
    print()

    # Check we're in the right directory
    if not Path("Cargo.toml").exists():
        print_colored("Error: Must be run from the rpcnet root directory", Colors.RED)
        return 1

    # Check if UV is available and use it if so
    use_uv = has_uv()

    if use_uv:
        print_colored("✓ UV detected - using UV for faster operations", Colors.GREEN)
        print()

        # Setup with UV
        if not setup_with_uv():
            print_colored("Falling back to traditional pip...", Colors.YELLOW)
            use_uv = False

        # Generate certificates
        if not generate_certificates():
            return 1

        # Build with UV
        if use_uv:
            if not build_module_with_uv():
                return 1
        else:
            if not build_module():
                return 1

        # Run tests with UV
        extra_args = sys.argv[1:] if len(sys.argv) > 1 else None
        if use_uv:
            if not run_tests_with_uv(extra_args):
                return 1
        else:
            if not run_tests(extra_args):
                return 1
    else:
        print_colored("UV not found - using traditional pip/maturin", Colors.YELLOW)
        print_colored("Install UV for 10-100x faster package management:", Colors.YELLOW)
        print_colored("  curl -LsSf https://astral.sh/uv/install.sh | sh", Colors.YELLOW)
        print()

        # Check prerequisites
        if not check_prerequisites():
            return 1

        # Generate certificates if needed
        if not generate_certificates():
            return 1

        # Build module
        if not build_module():
            return 1

        # Run tests
        extra_args = sys.argv[1:] if len(sys.argv) > 1 else None
        if not run_tests(extra_args):
            return 1

    return 0


if __name__ == "__main__":
    sys.exit(main())
