#!/bin/bash
set -e

echo "🔍 RpcNet Coverage Analysis"
echo "=========================="

# Run coverage
echo "Running cargo-tarpaulin..."
cargo tarpaulin --all-features --out Json --output-dir target/coverage 2>/dev/null

# Parse results
COVERAGE=$(cat target/coverage/tarpaulin-report.json | jq -r '.coverage')
echo "Overall Coverage: ${COVERAGE}%"

# Check threshold
if (( $(echo "$COVERAGE < 65" | bc -l) )); then
    echo "❌ Coverage below 65% threshold"
    
    echo -e "\n📊 Feature Coverage:"
    echo "- Core RPC: $(cargo tarpaulin --lib --run-types Tests --out Stdout 2>/dev/null | grep 'Coverage' | awk '{print $2}' || echo 'N/A')"
    echo "- Examples: $(cargo tarpaulin --examples --out Stdout 2>/dev/null | grep 'Coverage' | awk '{print $2}' || echo 'N/A')"
    
    echo -e "\n⚠️  Gaps Found:"
    cargo tarpaulin --print-uncovered-lines --all-features 2>/dev/null | head -20
    
    exit 1
else
    echo "✅ Coverage meets 65% threshold"
fi

echo -e "\n📈 Detailed report: target/coverage/tarpaulin-report.html"