#!/bin/bash
set -e

# RpcNet Coverage Analysis Script
# Runs cargo-tarpaulin and analyzes coverage by feature

echo "🔍 RpcNet Feature-Level Coverage Analysis"
echo "========================================"

# Check if jq is available
if ! command -v jq &> /dev/null; then
    echo "❌ jq is required for JSON parsing. Please install it."
    exit 1
fi

# Check if bc is available
if ! command -v bc &> /dev/null; then
    echo "❌ bc is required for calculations. Please install it."
    exit 1
fi

# Create output directory
mkdir -p target/coverage

# Run cargo-tarpaulin with comprehensive coverage
echo "📊 Running cargo-tarpaulin..."
echo "Note: Excluding 'python' feature (requires Python runtime for linking)"
cargo tarpaulin \
    --out Html \
    --out Json \
    --output-dir target/coverage \
    --exclude-files "examples/*" \
    --exclude-files "benches/*" \
    --exclude-files "specs/*" \
    --timeout 300 \
    --no-default-features \
    --features codegen,perf \
    --verbose

# Check if coverage report was generated
if [ ! -f "target/coverage/tarpaulin-report.json" ]; then
    echo "❌ Coverage report not generated. Check tarpaulin output."
    exit 1
fi

# Parse overall coverage
OVERALL_COVERAGE=$(cat target/coverage/tarpaulin-report.json | jq -r '.coverage')
echo "📈 Overall Coverage: ${OVERALL_COVERAGE}%"

# Analyze feature-specific coverage
echo ""
echo "🎯 Feature Coverage Analysis:"
echo "============================="

# Core RPC functionality
CORE_COVERAGE=$(cat target/coverage/tarpaulin-report.json | jq -r '.files[] | select(if .path | type == "array" then (.path | join("/") | test("src/(lib|client|server|error|config)\\.rs")) else (.path | test("src/(lib|client|server|error|config)\\.rs")) end) | .coverage' 2>/dev/null | awk '{sum+=$1; count++} END {if(count>0) printf "%.1f", sum/count; else print "0"}')
echo "✨ Core RPC: ${CORE_COVERAGE}%"

# QUIC Transport  
TRANSPORT_COVERAGE=$(cat target/coverage/tarpaulin-report.json | jq -r '.files[] | select(if .path | type == "array" then (.path | join("/") | test("src/(transport|connection)")) else (.path | test("src/(transport|connection)")) end) | .coverage' 2>/dev/null | awk '{sum+=$1; count++} END {if(count>0) printf "%.1f", sum/count; else print "0"}')
echo "🚀 QUIC Transport: ${TRANSPORT_COVERAGE}%"

# TLS Security
SECURITY_COVERAGE=$(cat target/coverage/tarpaulin-report.json | jq -r '.files[] | select(if .path | type == "array" then (.path | join("/") | test("src/(tls|cert|auth)")) else (.path | test("src/(tls|cert|auth)")) end) | .coverage' 2>/dev/null | awk '{sum+=$1; count++} END {if(count>0) printf "%.1f", sum/count; else print "0"}')
echo "🔒 TLS Security: ${SECURITY_COVERAGE}%"

# Code Generation
CODEGEN_COVERAGE=$(cat target/coverage/tarpaulin-report.json | jq -r '.files[] | select(if .path | type == "array" then (.path | join("/") | test("src/codegen|src/bin/rpcnet-gen")) else (.path | test("src/codegen|src/bin/rpcnet-gen")) end) | .coverage' 2>/dev/null | awk '{sum+=$1; count++} END {if(count>0) printf "%.1f", sum/count; else print "0"}')
echo "🛠️ Code Generation: ${CODEGEN_COVERAGE}%"

# Streaming
STREAMING_COVERAGE=$(cat target/coverage/tarpaulin-report.json | jq -r '.files[] | select(if .path | type == "array" then (.path | join("/") | test("src/(streaming|stream)")) else (.path | test("src/(streaming|stream)")) end) | .coverage' 2>/dev/null | awk '{sum+=$1; count++} END {if(count>0) printf "%.1f", sum/count; else print "0"}')
echo "📡 Streaming: ${STREAMING_COVERAGE}%"

# Set threshold (60% when Python bindings excluded)
THRESHOLD=60

echo ""
echo "📋 Summary:"
echo "==========="
echo "• Overall: ${OVERALL_COVERAGE}%"
echo "• Threshold: ${THRESHOLD}% (Python bindings excluded)"

# Check threshold
if (( $(echo "$OVERALL_COVERAGE < $THRESHOLD" | bc -l) )); then
    echo "❌ Coverage is below ${THRESHOLD}% threshold (Python bindings excluded)"
    
    echo ""
    echo "🔍 Files needing attention:"
    cat target/coverage/tarpaulin-report.json | jq -r '.files[] | select(.coverage < 65) | if .path | type == "array" then "  \(.path | join("/")): \(.coverage)%" else "  \(.path): \(.coverage)%" end' 2>/dev/null | head -10
    
    exit 1
else
    echo "✅ Coverage meets ${THRESHOLD}% threshold"
fi

echo ""
echo "Note: Threshold is ${THRESHOLD}% when Python bindings are excluded (tested separately)"
echo ""
echo "📄 Detailed reports:"
echo "  HTML: target/coverage/tarpaulin-report.html"
echo "  JSON: target/coverage/tarpaulin-report.json"