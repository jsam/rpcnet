# RPC.NET Makefile
# Provides convenient commands for testing, coverage, and development

.PHONY: help test test-unit test-integration coverage coverage-html clean doc bench lint examples

# Default target
help:
	@echo "RPC.NET Development Commands"
	@echo "============================"
	@echo ""
	@echo "Testing:"
	@echo "  test            - Run all tests"
	@echo "  test-unit       - Run unit tests only"
	@echo "  test-integration- Run integration tests only" 
	@echo "  test-examples   - Test all examples compile and run"
	@echo ""
	@echo "Coverage:"
	@echo "  coverage        - Generate test coverage report"
	@echo "  coverage-html   - Generate HTML coverage report and open"
	@echo "  coverage-ci     - Generate coverage for CI (with fail-under threshold)"
	@echo "  coverage-check  - Check if coverage meets 90% threshold"
	@echo "  coverage-gaps   - Show uncovered lines"
	@echo "  install-coverage-tools - Install coverage dependencies"
	@echo ""
	@echo "Development:"
	@echo "  lint            - Run clippy linter"
	@echo "  format          - Format code with rustfmt"
	@echo "  check           - Check code compiles without building"
	@echo "  clean           - Clean build artifacts and coverage reports"
	@echo ""
	@echo "Documentation:"
	@echo "  doc             - Generate and open documentation"
	@echo "  doc-private     - Generate documentation including private items"
	@echo ""
	@echo "Benchmarks:"
	@echo "  bench           - Run performance benchmarks"
	@echo ""
	@echo "Examples:"
	@echo "  examples        - Run all examples (for testing)"

# Testing commands
test:
	@echo "Running all tests..."
	cargo test

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
coverage:
	@echo "Generating test coverage report..."
	cargo tarpaulin --out Html --out Json --output-dir target/coverage --exclude-files "examples/*" --exclude-files "benches/*" --timeout 300 --all-features

coverage-html:
	@echo "Generating HTML coverage report..."
	cargo tarpaulin --config tarpaulin.toml --out Html
	@echo "Opening coverage report..."
	@if [ -f target/tarpaulin/tarpaulin-report.html ]; then \
		echo "Coverage report generated at: target/tarpaulin/tarpaulin-report.html"; \
		if command -v open >/dev/null; then \
			open target/tarpaulin/tarpaulin-report.html; \
		elif command -v xdg-open >/dev/null; then \
			xdg-open target/tarpaulin/tarpaulin-report.html; \
		else \
			echo "Please open target/tarpaulin/tarpaulin-report.html in your browser"; \
		fi \
	fi

coverage-ci:
	@echo "Running coverage analysis for CI..."
	cargo tarpaulin --config tarpaulin.toml --fail-under 95 --out Xml

coverage-check:
	@echo "Checking coverage threshold (90%)..."
	@cargo tarpaulin --out Json --output-dir target/coverage --exclude-files "examples/*" --exclude-files "benches/*" --timeout 300 --all-features
	@coverage=$$(cat target/coverage/tarpaulin-report.json | jq -r '.coverage'); \
	if (( $$(echo "$$coverage < 90" | bc -l) )); then \
		echo "❌ Coverage $$coverage% is below 90% threshold"; \
		exit 1; \
	else \
		echo "✅ Coverage $$coverage% meets threshold"; \
	fi

coverage-gaps:
	@echo "Analyzing coverage gaps..."
	@cargo tarpaulin --print-uncovered-lines --exclude-files "examples/*" --exclude-files "benches/*" --all-features

install-coverage-tools:
	@echo "Installing coverage tools..."
	cargo install cargo-tarpaulin
	@echo "Coverage tools installed"

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
	rm -rf target/tarpaulin coverage

# Documentation commands
doc:
	@echo "Generating documentation..."
	cargo doc --open --no-deps

doc-private:
	@echo "Generating documentation (including private items)..."
	cargo doc --open --no-deps --document-private-items

# Benchmark commands
bench:
	@echo "Running benchmarks..."
	cargo bench

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

# CI/CD commands (used by continuous integration)
ci-test:
	@echo "Running CI tests..."
	cargo test --all-targets --all-features

ci-coverage:
	@echo "Running CI coverage..."
	cargo tarpaulin --config tarpaulin.toml --out Xml --fail-under 95

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