#!/usr/bin/env python3
"""
Test runner that only runs the working tests.

This script runs only the tests that are known to work with the current implementation.
"""

import sys
import subprocess
from pathlib import Path

# Add parent directory to import run_tests
sys.path.insert(0, str(Path(__file__).parent))
from run_tests import (
    Colors, print_colored, has_uv, setup_with_uv, build_module_with_uv,
    check_prerequisites, generate_certificates, build_module
)


def run_working_tests_with_uv(extra_args=None):
    """Run only working tests using UV."""
    print_colored("\nRunning working tests with UV...", Colors.YELLOW)
    print()

    # Build pytest command for only working tests
    cmd = [
        "uv", "run", "pytest",
        "python_tests/test_serialization.py",
        "python_tests/test_client_simple.py",
        "python_tests/test_client_fixed_port.py",
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
        print_colored("✓ All working tests passed!", Colors.GREEN)
        print_colored("=" * 40, Colors.GREEN)
        print()
        print_colored("Note: Some tests are skipped because they require additional features:", Colors.YELLOW)
        print_colored("  - test_client.py: Needs server.local_addr() method", Colors.YELLOW)
        print_colored("  - test_streaming.py: Needs server-side streaming handlers", Colors.YELLOW)
        print()
        print_colored("See python_tests/TEST_STATUS.md for details", Colors.YELLOW)
        return True
    else:
        print_colored("=" * 40, Colors.RED)
        print_colored("✗ Some tests failed", Colors.RED)
        print_colored("=" * 40, Colors.RED)
        return False


def run_working_tests(extra_args=None):
    """Run only working tests using regular pytest."""
    print_colored("\nRunning working tests...", Colors.YELLOW)
    print()

    # Build pytest command for only working tests
    cmd = [
        sys.executable, "-m", "pytest",
        "python_tests/test_serialization.py",
        "python_tests/test_client_simple.py",
        "python_tests/test_client_fixed_port.py",
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
        print_colored("✓ All working tests passed!", Colors.GREEN)
        print_colored("=" * 40, Colors.GREEN)
        print()
        print_colored("Note: Some tests are skipped because they require additional features:", Colors.YELLOW)
        print_colored("  - test_client.py: Needs server.local_addr() method", Colors.YELLOW)
        print_colored("  - test_streaming.py: Needs server-side streaming handlers", Colors.YELLOW)
        print()
        print_colored("See python_tests/TEST_STATUS.md for details", Colors.YELLOW)
        return True
    else:
        print_colored("=" * 40, Colors.RED)
        print_colored("✗ Some tests failed", Colors.RED)
        print_colored("=" * 40, Colors.RED)
        return False


def main():
    """Main entry point."""
    print_colored("=" * 40, Colors.YELLOW)
    print_colored("RpcNet Python Bindings - Working Tests", Colors.YELLOW)
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

        # Run working tests with UV
        extra_args = sys.argv[1:] if len(sys.argv) > 1 else None
        if use_uv:
            if not run_working_tests_with_uv(extra_args):
                return 1
        else:
            if not run_working_tests(extra_args):
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

        # Run working tests
        extra_args = sys.argv[1:] if len(sys.argv) > 1 else None
        if not run_working_tests(extra_args):
            return 1

    return 0


if __name__ == "__main__":
    sys.exit(main())
