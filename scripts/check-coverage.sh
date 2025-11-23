#!/bin/bash
set -e

echo "🔍 RpcNet Coverage Analysis"
echo "=========================="

# Run coverage excluding python feature (PyO3 requires Python dev libraries)
echo "Running cargo-tarpaulin..."
echo "Note: Excluding 'python' feature (requires Python runtime for linking)"
cargo tarpaulin --no-default-features --features codegen,perf --out Json --output-dir target/coverage 2>/dev/null

# Parse results
COVERAGE=$(cat target/coverage/tarpaulin-report.json | jq -r '.coverage')
echo "Overall Coverage: ${COVERAGE}%"

# Check threshold (60% when Python bindings excluded, 65% with all features)
THRESHOLD=60
if (( $(echo "$COVERAGE < $THRESHOLD" | bc -l) )); then
    echo "❌ Coverage below ${THRESHOLD}% threshold (Python bindings excluded)"

    echo -e "\n📊 Feature Coverage:"
    echo "- Core RPC: $(cargo tarpaulin --lib --run-types Tests --out Stdout 2>/dev/null | grep 'Coverage' | awk '{print $2}' || echo 'N/A')"
    echo "- Examples: $(cargo tarpaulin --examples --out Stdout 2>/dev/null | grep 'Coverage' | awk '{print $2}' || echo 'N/A')"

    echo -e "\n⚠️  Gaps Found:"
    cargo tarpaulin --print-uncovered-lines --no-default-features --features codegen,perf 2>/dev/null | head -20

    exit 1
else
    echo "✅ Coverage meets ${THRESHOLD}% threshold"
fi

echo -e "\n📈 Detailed report: target/coverage/tarpaulin-report.html"
echo -e "\nNote: Python bindings (src/python/) are tested via Python integration tests"