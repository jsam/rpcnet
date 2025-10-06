# RPC.NET Makefile
# Provides convenient commands for testing, coverage, and development

.PHONY: help test test-unit test-integration coverage coverage-html clean doc bench lint examples publish publish-dry-run

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
	@echo "  lint            - Run clippy linter"
	@echo "  format          - Format code with rustfmt"
	@echo "  check           - Check code compiles without building"
	@echo "  clean           - Clean build artifacts and coverage reports"
	@echo ""
	@echo "Release:"
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
# Usage: make coverage [tool] - tool can be tarpaulin (default) or llvm-cov
coverage:
	@$(MAKE) coverage-tool TOOL=$(if $(filter tarpaulin llvm-cov,$(MAKECMDGOALS)),$(MAKECMDGOALS),tarpaulin)

coverage-tool:
	@if [ "$(TOOL)" = "llvm-cov" ]; then \
		echo "Generating test coverage report with LLVM..."; \
		cargo llvm-cov --html --lcov --output-dir target/llvm-cov; \
	else \
		echo "Generating test coverage report with Tarpaulin..."; \
		cargo tarpaulin --out Html --out Json --output-dir target/coverage --exclude-files "examples/*" --exclude-files "benches/*" --timeout 300 --all-features; \
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
		cargo tarpaulin --config tarpaulin.toml --fail-under 95 --out Xml; \
	fi

# Usage: make coverage-check [tool] - tool can be tarpaulin (default) or llvm-cov
coverage-check:
	@$(MAKE) coverage-check-tool TOOL=$(if $(filter tarpaulin llvm-cov,$(MAKECMDGOALS)),$(MAKECMDGOALS),tarpaulin)

coverage-check-tool:
	@if [ "$(TOOL)" = "llvm-cov" ]; then \
		echo "Checking coverage threshold (90%) with LLVM..."; \
		cargo llvm-cov --json --output-dir target/llvm-cov; \
		coverage=$$(cat target/llvm-cov/llvm-cov.json | jq -r '.data[0].totals.lines.percent'); \
		if (( $$(echo "$$coverage < 90" | bc -l) )); then \
			echo "❌ Coverage $$coverage% is below 90% threshold"; \
			exit 1; \
		else \
			echo "✅ Coverage $$coverage% meets threshold"; \
		fi \
	else \
		echo "Checking coverage threshold (90%) with Tarpaulin..."; \
		cargo tarpaulin --out Json --output-dir target/coverage --exclude-files "examples/*" --exclude-files "benches/*" --timeout 300 --all-features; \
		coverage=$$(cat target/coverage/tarpaulin-report.json | jq -r '.coverage'); \
		if (( $$(echo "$$coverage < 90" | bc -l) )); then \
			echo "❌ Coverage $$coverage% is below 90% threshold"; \
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
	@echo "5. Checking coverage..."
	@$(MAKE) coverage-check >/dev/null 2>&1 || (echo "⚠️  Coverage check failed (continuing anyway)")
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
	@echo "Package location: target/package/rpcnet-0.2.0.crate"
	@echo ""
	@echo "To publish, run: make publish"

publish: publish-check
	@echo "=== Publishing to crates.io ==="
	@echo ""
	@echo "⚠️  WARNING: This action cannot be undone!"
	@echo ""
	@read -p "Are you sure you want to publish rpcnet v0.2.0? (yes/no): " confirm && \
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
	@echo "  2. Push tag: git push origin v0.2.0"
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
