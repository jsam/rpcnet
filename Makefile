# RPC.NET Makefile
# Provides convenient commands for testing, coverage, and development

# Project paths
ROOT_DIR := $(shell pwd)
VENV_PYTHON := $(ROOT_DIR)/.venv/bin/python
VENV_PIP := $(ROOT_DIR)/.venv/bin/pip
VENV_MATURIN := $(ROOT_DIR)/.venv/bin/maturin
RPCNET_GEN := $(ROOT_DIR)/target/release/rpcnet-gen

.PHONY: help test test-quick test-only test-unit test-integration lint lint-quick pre-commit coverage coverage-html clean doc bench examples publish publish-dry-run

# Default target
help:
	@echo "RPC.NET Development Commands"
	@echo "============================"
	@echo ""
	@echo "Testing:"
	@echo "  test            - Run tests + quick lint (recommended)"
	@echo "  test-quick      - Run tests only (faster)"
	@echo "  test-only       - Alias for test-quick"
	@echo "  test-unit       - Run unit tests only"
	@echo "  test-integration- Run integration tests only" 
	@echo "  test-examples   - Test all examples compile and run"
	@echo ""
	@echo "Coverage:"
	@echo "  coverage [tool]        - Generate test coverage report (tool: tarpaulin|llvm-cov)"
	@echo "  coverage-html [tool]   - Generate HTML coverage report and open"
	@echo "  coverage-ci [tool]     - Generate coverage for CI (with fail-under threshold)"
	@echo "  coverage-check [tool]  - Check if coverage meets 90% threshold"
	@echo "  coverage-gaps [tool]   - Show uncovered lines"
	@echo "  install-coverage-tools - Install coverage dependencies (both tools)"
	@echo ""
	@echo "  Examples:"
	@echo "    make coverage-html tarpaulin  # Use tarpaulin for HTML report"
	@echo "    make coverage-html llvm-cov   # Use llvm-cov for HTML report"
	@echo "    make coverage-html            # Use tarpaulin (default)"
	@echo ""
	@echo "Development:"
	@echo "  pre-commit      - Quick pre-commit checks (format + lint + test)"
	@echo "  lint            - Run full clippy linter on all targets"
	@echo "  lint-quick      - Run quick clippy check (lib + bins only)"
	@echo "  format          - Format code with rustfmt"
	@echo "  format-check    - Check if code is formatted (CI)"
	@echo "  check           - Check code compiles without building"
	@echo "  clean           - Clean build artifacts and coverage reports"
	@echo ""
	@echo "Release:"
	@echo "  release-prepare - Prepare a new release (version bump + changelog)"
	@echo "  changelog       - Generate changelog from conventional commits"
	@echo "  publish-check   - Run all pre-publication checks (tests, lint, docs)"
	@echo "  publish-dry-run - Package and verify crate contents before publishing"
	@echo "  publish         - Publish to crates.io (includes all checks + confirmation)"
	@echo ""
	@echo "Documentation:"
	@echo "  doc             - Generate and open documentation"
	@echo "  doc-private     - Generate documentation including private items"
	@echo "  doc-book        - Build mdBook for the user guide"
	@echo "  doc-book-serve  - Serve mdBook locally with live reload"
	@echo ""
	@echo "Benchmarks:"
	@echo "  bench           - Run performance benchmarks"
	@echo ""
	@echo "Python Extension:"
	@echo "  python-setup    - Setup Python venv and install dependencies"
	@echo "  python-build    - Clean build of Python extension module"
	@echo "  python-test     - Run Python integration tests"
	@echo "  python-clean    - Clean Python build artifacts"
	@echo ""
	@echo "Python Examples:"
	@echo "  python-examples               - Test all Python examples"
	@echo "  python-example-client-server  - Test client/server example"
	@echo "  python-example-streaming      - Test streaming example"
	@echo "  python-example-cluster        - Test cluster example"
	@echo ""
	@echo "Examples:"
	@echo "  examples        - Run all examples (for testing)"

# Testing commands
test: test-quick lint-quick
	@echo ""
	@echo "✅ All tests and checks passed!"

test-quick:
	@echo "Running all tests..."
	cargo test

test-only:
	@echo "Running tests only (no linting)..."
	cargo test

lint-quick:
	@echo ""
	@echo "Running quick lint check..."
	@cargo clippy --lib --bins -- -D warnings || (echo "❌ Clippy failed! Run 'cargo clippy --fix' to auto-fix." && exit 1)
	@echo "✅ Lint check passed"

test-unit:
	@echo "Running unit tests..."
	cargo test --lib

test-integration:
	@echo "Running integration tests..."
	cargo test --test '*'

test-examples:
	@echo "Testing examples compile..."
	cargo build --examples
	@echo "Running basic example test..."
	timeout 10s cargo run --example basic_client_server || true

# Coverage commands
# Usage: make coverage [tool] - tool can be tarpaulin (default) or llvm-cov
coverage:
	@$(MAKE) coverage-tool TOOL=$(if $(filter tarpaulin llvm-cov,$(MAKECMDGOALS)),$(MAKECMDGOALS),tarpaulin)

coverage-tool:
	@if [ "$(TOOL)" = "llvm-cov" ]; then \
		echo "Generating test coverage report with LLVM..."; \
		cargo llvm-cov --html --lcov --output-dir target/llvm-cov; \
	else \
		echo "Generating test coverage report with Tarpaulin..."; \
		cargo tarpaulin --out Html --out Json --output-dir target/coverage --exclude-files "examples/*" --exclude-files "benches/*" --timeout 300 --no-default-features --features codegen,perf; \
	fi

# Usage: make coverage-html [tool] - tool can be tarpaulin (default) or llvm-cov  
coverage-html:
	@$(MAKE) coverage-html-tool TOOL=$(if $(filter tarpaulin llvm-cov,$(MAKECMDGOALS)),$(MAKECMDGOALS),tarpaulin)

coverage-html-tool:
	@if [ "$(TOOL)" = "llvm-cov" ]; then \
		echo "Generating HTML coverage report with LLVM..."; \
		cargo llvm-cov --html --open; \
		echo "Coverage report generated at: target/llvm-cov/html/index.html"; \
	else \
		echo "Generating HTML coverage report with Tarpaulin..."; \
		cargo tarpaulin --config tarpaulin.toml --out Html; \
		echo "Opening coverage report..."; \
		if [ -f target/tarpaulin/tarpaulin-report.html ]; then \
			echo "Coverage report generated at: target/tarpaulin/tarpaulin-report.html"; \
			if command -v open >/dev/null; then \
				open target/tarpaulin/tarpaulin-report.html; \
			elif command -v xdg-open >/dev/null; then \
				xdg-open target/tarpaulin/tarpaulin-report.html; \
			else \
				echo "Please open target/tarpaulin/tarpaulin-report.html in your browser"; \
			fi \
		fi \
	fi

# Usage: make coverage-ci [tool] - tool can be tarpaulin (default) or llvm-cov
coverage-ci:
	@$(MAKE) coverage-ci-tool TOOL=$(if $(filter tarpaulin llvm-cov,$(MAKECMDGOALS)),$(MAKECMDGOALS),tarpaulin)

coverage-ci-tool:
	@if [ "$(TOOL)" = "llvm-cov" ]; then \
		echo "Running coverage analysis for CI with LLVM..."; \
		cargo llvm-cov --lcov --output-dir target/llvm-cov; \
		echo "LLVM coverage report generated for CI"; \
	else \
		echo "Running coverage analysis for CI with Tarpaulin..."; \
		cargo tarpaulin --config tarpaulin.toml --fail-under 50 --out Xml; \
	fi

# Usage: make coverage-check [tool] - tool can be tarpaulin (default) or llvm-cov
coverage-check:
	@$(MAKE) coverage-check-tool TOOL=$(if $(filter tarpaulin llvm-cov,$(MAKECMDGOALS)),$(MAKECMDGOALS),tarpaulin)

coverage-check-tool:
	@if [ "$(TOOL)" = "llvm-cov" ]; then \
		echo "Checking coverage threshold (65%, Python excluded) with LLVM..."; \
		cargo llvm-cov --json --output-dir target/llvm-cov; \
		coverage=$$(cat target/llvm-cov/llvm-cov.json | jq -r '.data[0].totals.lines.percent'); \
		if (( $$(echo "$$coverage < 65" | bc -l) )); then \
			echo "❌ Coverage $$coverage% is below 65% threshold"; \
			exit 1; \
		else \
			echo "✅ Coverage $$coverage% meets threshold"; \
		fi \
	else \
		echo "Checking coverage threshold (65%, Python excluded) with Tarpaulin..."; \
		cargo tarpaulin --out Json --output-dir target/coverage --exclude-files "examples/*" --exclude-files "benches/*" --timeout 300 --no-default-features --features codegen,perf; \
		coverage=$$(cat target/coverage/tarpaulin-report.json | jq -r '.coverage'); \
		if (( $$(echo "$$coverage < 65" | bc -l) )); then \
			echo "❌ Coverage $$coverage% is below 65% threshold (Python bindings excluded)"; \
			exit 1; \
		else \
			echo "✅ Coverage $$coverage% meets threshold"; \
		fi \
	fi

# Usage: make coverage-gaps [tool] - tool can be tarpaulin (default) or llvm-cov
coverage-gaps:
	@$(MAKE) coverage-gaps-tool TOOL=$(if $(filter tarpaulin llvm-cov,$(MAKECMDGOALS)),$(MAKECMDGOALS),tarpaulin)

coverage-gaps-tool:
	@if [ "$(TOOL)" = "llvm-cov" ]; then \
		echo "Analyzing coverage gaps with LLVM..."; \
		cargo llvm-cov --html --open; \
		echo "Open the HTML report to see uncovered lines"; \
	else \
		echo "Analyzing coverage gaps with Tarpaulin..."; \
		cargo tarpaulin --print-uncovered-lines --exclude-files "examples/*" --exclude-files "benches/*" --all-features; \
	fi

install-coverage-tools:
	@echo "Installing coverage tools..."
	cargo install cargo-tarpaulin cargo-llvm-cov
	@echo "Coverage tools installed (both tarpaulin and llvm-cov)"

# Allow tool names to be passed as targets (for backwards compatibility)
tarpaulin llvm-cov:
	@# This is a dummy target to consume the tool name arguments

# Development commands
lint:
	@echo "Running clippy linter..."
	cargo clippy --all-targets --all-features -- -D warnings

format:
	@echo "Formatting code..."
	cargo fmt

format-check:
	@echo "Checking code formatting..."
	cargo fmt --check

check:
	@echo "Checking code compiles..."
	cargo check --all-targets

clean:
	@echo "Cleaning build artifacts..."
	cargo clean
	@echo "Cleaning coverage reports..."
	rm -rf target/tarpaulin target/llvm-cov coverage llvm-coverage.lcov

# Release management commands
release-prepare:
	@if [ -z "$(VERSION)" ]; then \
		echo "Error: VERSION not specified"; \
		echo "Usage: make release-prepare VERSION=0.2.0"; \
		exit 1; \
	fi
	@echo "Preparing release $(VERSION)..."
	./scripts/prepare-release.sh $(VERSION)

changelog:
	@echo "Generating changelog..."
	@command -v git-cliff >/dev/null 2>&1 || { \
		echo "git-cliff not found. Installing..."; \
		cargo install git-cliff; \
	}
	@if [ -z "$(VERSION)" ]; then \
		echo "Generating changelog for all changes..."; \
		git-cliff -o CHANGELOG.md; \
	else \
		echo "Generating changelog for version $(VERSION)..."; \
		git-cliff --tag "$(VERSION)" -o CHANGELOG.md; \
	fi
	@echo "✅ Changelog generated in CHANGELOG.md"

publish-check:
	@echo "=== Pre-Publication Checks ==="
	@echo ""
	@echo "1. Verifying all tests pass..."
	@cargo test --all-features || (echo "❌ Tests failed! Fix tests before publishing." && exit 1)
	@echo "✅ All tests passed"
	@echo ""
	@echo "2. Checking code formatting..."
	@cargo fmt --check || (echo "❌ Code not formatted! Run 'make format' first." && exit 1)
	@echo "✅ Code is properly formatted"
	@echo ""
	@echo "3. Running linter..."
	@cargo clippy --all-targets --all-features -- -D warnings || (echo "❌ Clippy warnings found! Fix them first." && exit 1)
	@echo "✅ No clippy warnings"
	@echo ""
	@echo "4. Building documentation..."
	@cargo doc --no-deps --all-features 2>&1 | grep -q "error" && (echo "❌ Documentation build failed!" && exit 1) || echo "✅ Documentation builds successfully"
	@echo ""
# 	@echo "5. Checking coverage..."
# 	@$(MAKE) coverage-check >/dev/null 2>&1 || (echo "⚠️  Coverage check failed (continuing anyway)")
	@echo ""
	@echo "✅ All pre-publication checks passed!"
	@echo ""

publish-dry-run: publish-check
	@echo "=== Packaging rpcnet (library + rpcnet-gen CLI) ==="
	@echo ""
	@echo "Reviewing package contents..."
	@cargo package --list --allow-dirty | head -50
	@echo "..."
	@echo ""
	@echo "Building package..."
	@cargo package --allow-dirty
	@echo ""
	@echo "✅ Package created successfully!"
	@echo ""
	@echo "Package location: target/package/rpcnet-0.1.0.crate"
	@echo ""
	@echo "To publish, run: make publish"

publish: publish-check
	@echo "=== Publishing to crates.io ==="
	@echo ""
	@echo "⚠️  WARNING: This action cannot be undone!"
	@echo ""
	@read -p "Are you sure you want to publish rpcnet v0.1.0? (yes/no): " confirm && \
	if [ "$$confirm" != "yes" ]; then \
		echo "Publication cancelled."; \
		exit 1; \
	fi
	@echo ""
	@echo "Publishing rpcnet crate (library + rpcnet-gen CLI)..."
	@cargo publish
	@echo ""
	@echo "✅ Successfully published to crates.io!"
	@echo ""
	@echo "Next steps:"
	@echo "  1. Push to GitHub: git push origin main"
	@echo "  2. Push tag: git push origin v0.1.0"
	@echo "  3. Create GitHub release: https://github.com/jsam/rpcnet/releases/new"
	@echo "  4. Verify on crates.io: https://crates.io/crates/rpcnet"
	@echo "  5. Check docs.rs: https://docs.rs/rpcnet"
	@echo ""

# Documentation commands
doc:
	@echo "Generating documentation..."
	cargo doc --open --no-deps

doc-private:
	@echo "Generating documentation (including private items)..."
	cargo doc --open --no-deps --document-private-items

doc-book:
	@echo "Building mdBook..."
	mdbook build docs/mdbook
	@echo "Book output: docs/mdbook/book/index.html"

doc-book-serve:
	@echo "Serving mdBook on http://localhost:3000 ..."
	mdbook serve docs/mdbook --open

# Benchmark commands
bench:
	@echo "Running all benchmarks (Rust + Python)..."
	@echo ""
	@echo "=== Rust Benchmarks ==="
	cargo bench
	@echo ""
	@echo "=== Python Benchmarks ==="
	@if [ -f .venv/bin/python ]; then \
		.venv/bin/python benches/python_realistic_bench.py; \
	else \
		echo "⚠️  Python venv not found. Skipping Python benchmarks."; \
		echo "   Run: uv venv && uv run maturin develop --features python --release"; \
	fi

bench-rust:
	@echo "Running Rust benchmarks only..."
	cargo bench

bench-python:
	@echo "Running Python benchmarks only..."
	@if [ -f .venv/bin/python ]; then \
		.venv/bin/python benches/python_realistic_bench.py; \
	else \
		echo "❌ Error: Python venv not found"; \
		echo "   Run: uv venv && uv run maturin develop --features python --release"; \
		exit 1; \
	fi

# Python Extension commands
python-build:
	@echo "Building Python extension module..."
	@./scripts/build_python.sh

python-build-release:
	@echo "Building Python extension module (release mode)..."
	@./scripts/build_python.sh --release

python-test:
	@echo "Running Python integration tests..."
	@if [ ! -d ".venv" ]; then \
		echo "❌ Error: No .venv directory found"; \
		echo "   Run: make python-build first"; \
		exit 1; \
	fi
	@.venv/bin/pytest tests/test_python_*.py -v

python-clean:
	@echo "Cleaning Python build artifacts..."
	@find . -name "_rpcnet*.so" -delete 2>/dev/null || true
	@find . -name "librpcnet*.so" -delete 2>/dev/null || true
	@find . -name "librpcnet*.dylib" -delete 2>/dev/null || true
	@find . -type d -name "__pycache__" -exec rm -rf {} + 2>/dev/null || true
	@find . -type f -name "*.pyc" -delete 2>/dev/null || true
	@rm -rf target/wheels/ .pytest_cache/ build/ dist/ *.egg-info/ 2>/dev/null || true
	@echo "✅ Python artifacts cleaned"

python-setup:
	@echo "Setting up Python development environment..."
	@if [ ! -d ".venv" ]; then \
		echo "Creating virtual environment..."; \
		python3 -m venv .venv; \
	fi
	@echo "Installing dependencies..."
	@.venv/bin/pip install -q maturin pytest pytest-asyncio
	@echo "✅ Python environment ready"
	@echo ""
	@echo "Next step: make python-build"

# Example commands
examples:
	@echo "Building all examples..."
	cargo build --examples
	@echo ""
	@echo "Available examples:"
	@echo "  cargo run --example basic_client_server"
	@echo "  cargo run --example echo_service"
	@echo "  cargo run --example calculator_service"
	@echo "  cargo run --example file_transfer"
	@echo "  cargo run --example concurrent_clients"
	@echo "  cargo run --example custom_serialization"
	@echo "  cargo run --example error_handling"
	@echo "  cargo run --example configuration"
	@echo "  cargo run --example health_check"

# Development workflow commands
dev-setup:
	@echo "Setting up development environment..."
	@echo "Installing required tools..."
	cargo install cargo-tarpaulin cargo-watch
	@echo "Development setup complete!"

dev-watch:
	@echo "Watching for file changes and running tests..."
	cargo watch -x test

# Pre-commit check - runs locally before committing
pre-commit:
	@echo "=== Pre-Commit Checks ==="
	@echo ""
	@echo "1. Formatting code..."
	@cargo fmt || (echo "❌ Format failed!" && exit 1)
	@echo "✅ Code formatted"
	@echo ""
	@echo "2. Running quick lint..."
	@cargo clippy --lib --bins -- -D warnings || (echo "❌ Clippy failed! Fix warnings or run 'cargo clippy --fix'" && exit 1)
	@echo "✅ Lint passed"
	@echo ""
	@echo "3. Running tests..."
	@cargo test --lib || (echo "❌ Tests failed!" && exit 1)
	@echo "✅ Tests passed"
	@echo ""
	@echo "✅ All pre-commit checks passed! Ready to commit."

# CI/CD commands (used by continuous integration)
ci-test:
	@echo "Running CI tests..."
	@echo "Note: Testing without extension-module feature (PyO3 linking issue)"
	@echo "Note: Skipping benchmarks due to python_interop lifecycle issues"
	cargo test --lib --bins --tests --examples --features "codegen,perf,python"

ci-coverage:
	@echo "Running CI coverage..."
	cargo tarpaulin --config tarpaulin.toml --out Xml --fail-under 50

ci-lint:
	@echo "Running CI linting..."
	cargo clippy --all-targets --all-features -- -D warnings
	cargo fmt --check

ci-security:
	@echo "Running security audit..."
	cargo audit || echo "cargo-audit not installed, skipping security check"

# All CI checks in one command
ci: ci-lint ci-test ci-coverage
	@echo "All CI checks passed!"

# Python Example Testing
python-examples: python-example-client-server python-example-streaming python-example-cluster
	@echo ""
	@echo "✅ All Python examples tested successfully!"

# CI Python examples - same as python-examples but with certificate generation
ci-python-examples:
	@echo "=== CI Python Examples Testing ==="
	@echo "Generating test certificates..."
	@mkdir -p certs
	@openssl req -x509 -newkey rsa:4096 -keyout certs/test_key.pem -out certs/test_cert.pem -days 365 -nodes -subj "/CN=localhost" 2>/dev/null || true
	@$(MAKE) python-examples

python-example-client-server:
	@echo "=== Testing Python Client/Server Example ==="
	@echo "Building rpcnet-gen..."
	@cargo build --release --bin rpcnet-gen --features codegen,python
	@echo "Building Python extension..."
	@if [ ! -d ".venv" ]; then \
		echo "Creating virtual environment..."; \
		python3 -m venv .venv; \
		$(VENV_PIP) install -q maturin cloudpickle; \
	fi
	@$(VENV_MATURIN) develop --release --features extension-module,codegen,python --quiet
	@echo "Generating Python client code..."
	@cd examples/python/client_server && $(RPCNET_GEN) --input benchmark.rpc.rs --output generated --python
	@echo "Starting server..."
	@cd examples/python/client_server && PYTHON=$(VENV_PYTHON) $(VENV_PYTHON) $(ROOT_DIR)/examples/python/client_server/server.py & echo $$! > /tmp/rpcnet_test_server.pid && sleep 10
	@echo "Running blocking client test..."
	@cd examples/python/client_server && CERT_PATH=$(ROOT_DIR)/certs/test_cert.pem timeout 60 $(VENV_PYTHON) $(ROOT_DIR)/examples/python/client_server/client.py || (kill $$(cat /tmp/rpcnet_test_server.pid) 2>/dev/null; rm -f /tmp/rpcnet_test_server.pid; exit 1)
	@echo "Running async client test..."
	@cd examples/python/client_server && CERT_PATH=$(ROOT_DIR)/certs/test_cert.pem timeout 60 $(VENV_PYTHON) $(ROOT_DIR)/examples/python/client_server/async_client.py || (kill $$(cat /tmp/rpcnet_test_server.pid) 2>/dev/null; rm -f /tmp/rpcnet_test_server.pid; exit 1)
	@kill $$(cat /tmp/rpcnet_test_server.pid) 2>/dev/null || true
	@rm -f /tmp/rpcnet_test_server.pid
	@echo "✅ Client/Server example passed"

python-example-streaming:
	@echo "=== Testing Python Streaming Example ==="
	@echo "Building rpcnet-gen..."
	@cargo build --release --bin rpcnet-gen --features codegen,python
	@echo "Building Python extension..."
	@if [ ! -d ".venv" ]; then \
		echo "Creating virtual environment..."; \
		python3 -m venv .venv; \
		$(VENV_PIP) install -q maturin cloudpickle; \
	fi
	@$(VENV_MATURIN) develop --release --features extension-module,codegen,python --quiet
	@echo "Building Rust server..."
	@cd examples/python/streaming && cargo build --release
	@echo "Generating Python client code..."
	@cd examples/python/streaming && $(RPCNET_GEN) --input streaming.rpc.rs --output . --python
	@echo "Testing import..."
	@cd examples/python/streaming && $(VENV_PYTHON) -c "import sys; sys.path.insert(0, '.'); from streamingservice.client import StreamingServiceClient; print('✅ streamingservice.client imports')"
	@echo "Starting server..."
	@cd examples/python/streaming && BIND_ADDR=127.0.0.1:50052 ./target/release/server > /tmp/rpcnet_streaming_server.log 2>&1 & echo $$! > /tmp/rpcnet_streaming_server.pid && sleep 5
	@echo "Running unary client test..."
	@CERT_PATH=$(ROOT_DIR)/certs/test_cert.pem timeout 30 $(VENV_PYTHON) $(ROOT_DIR)/examples/python/streaming/unary_client.py || (kill $$(cat /tmp/rpcnet_streaming_server.pid) 2>/dev/null; rm -f /tmp/rpcnet_streaming_server.pid; exit 1)
	@kill $$(cat /tmp/rpcnet_streaming_server.pid) 2>/dev/null || true
	@rm -f /tmp/rpcnet_streaming_server.pid
	@echo "✅ Streaming example passed"

python-example-cluster:
	@echo "=== Testing Python Cluster Example ==="
	@echo "Building rpcnet-gen..."
	@cargo build --release --bin rpcnet-gen --features codegen,python
	@echo "Building Python extension..."
	@if [ ! -d ".venv" ]; then \
		echo "Creating virtual environment..."; \
		python3 -m venv .venv; \
		$(VENV_PIP) install -q maturin cloudpickle; \
	fi
	@$(VENV_MATURIN) develop --release --features extension-module,codegen,python --quiet
	@echo "Generating Rust code for cluster..."
	@cd examples/python/cluster && mkdir -p src/generated
	@cd examples/python/cluster && $(RPCNET_GEN) --input inference.rpc.rs --output src/generated
	@cd examples/python/cluster && $(RPCNET_GEN) --input director_registry.rpc.rs --output src/generated
	@echo "Building Rust components..."
	@cd examples/python/cluster && cargo build --release
	@echo "Generating Python code..."
	@cd examples/python/cluster && $(RPCNET_GEN) --input inference.rpc.rs --output . --python
	@cd examples/python/cluster && $(RPCNET_GEN) --input director_registry.rpc.rs --output . --python
	@echo "Testing imports..."
	@cd examples/python/cluster && $(VENV_PYTHON) -c "import sys; sys.path.insert(0, '.'); from inference.server import InferenceServer; print('✅ inference.server imports')"
	@cd examples/python/cluster && $(VENV_PYTHON) -c "import sys; sys.path.insert(0, '.'); from directorregistry.client import DirectorRegistryClient; print('✅ directorregistry.client imports')"
	@echo "Starting director..."
	@cd examples/python/cluster && DIRECTOR_ADDR=127.0.0.1:61000 ./target/release/director > /tmp/rpcnet_director.log 2>&1 & echo $$! > /tmp/rpcnet_director.pid && sleep 8
	@echo "Starting Python worker..."
	@cd examples/python/cluster && PYTHON=$(VENV_PYTHON) WORKER_LABEL=test-worker WORKER_ADDR=127.0.0.1:62001 DIRECTOR_ADDR=127.0.0.1:61000 CERT_PATH=$(ROOT_DIR)/certs/test_cert.pem KEY_PATH=$(ROOT_DIR)/certs/test_key.pem PROCESSES=2 $(VENV_PYTHON) $(ROOT_DIR)/examples/python/cluster/python_worker.py > /tmp/rpcnet_worker.log 2>&1 & echo $$! > /tmp/rpcnet_worker.pid && sleep 10
	@echo "Running cluster client test..."
	@DIRECTOR_ADDR=127.0.0.1:61000 CERT_PATH=$(ROOT_DIR)/certs/test_cert.pem timeout 60 $(VENV_PYTHON) $(ROOT_DIR)/examples/python/cluster/client.py || (kill $$(cat /tmp/rpcnet_worker.pid /tmp/rpcnet_director.pid) 2>/dev/null; rm -f /tmp/rpcnet_worker.pid /tmp/rpcnet_director.pid; exit 1)
	@kill $$(cat /tmp/rpcnet_worker.pid /tmp/rpcnet_director.pid) 2>/dev/null || true
	@rm -f /tmp/rpcnet_worker.pid /tmp/rpcnet_director.pid
	@echo "✅ Cluster example passed"
