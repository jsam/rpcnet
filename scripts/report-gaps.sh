#!/bin/bash
set -e

# RpcNet Coverage Gap Reporting Script
# Identifies and categorizes coverage gaps by priority

echo "⚠️  RpcNet Coverage Gap Analysis"
echo "================================"

# Check if coverage report exists
if [ ! -f "target/coverage/tarpaulin-report.json" ]; then
    echo "❌ No coverage report found. Run 'make coverage' first."
    exit 1
fi

# Check dependencies
if ! command -v jq &> /dev/null; then
    echo "❌ jq is required for JSON parsing. Please install it."
    exit 1
fi

if ! command -v bc &> /dev/null; then
    echo "❌ bc is required for calculations. Please install it."
    exit 1
fi

# Parse overall coverage
OVERALL_COVERAGE=$(cat target/coverage/tarpaulin-report.json | jq -r '.coverage')
echo "📊 Overall Coverage: ${OVERALL_COVERAGE}%"
echo ""

# Initialize counters
CRITICAL_GAPS=0
HIGH_GAPS=0
MEDIUM_GAPS=0
LOW_GAPS=0

echo "🔍 Coverage Gaps by Priority:"
echo "============================="

# CRITICAL: Security and Core RPC (must be > 65%)
echo "🚨 CRITICAL (Security & Core - Must be >65%):"
echo "----------------------------------------------"

# Check core files
for file in "src/lib.rs" "src/client.rs" "src/server.rs" "src/error.rs"; do
    if [ -f "$file" ]; then
        COVERAGE=$(cat target/coverage/tarpaulin-report.json | jq -r --arg file "$file" '.files[] | select(if .path | type == "array" then (.path | join("/") == $file) else (.path == $file) end) | .coverage // 0' 2>/dev/null)
        if [ -n "$COVERAGE" ] && (( $(echo "$COVERAGE < 65" | bc -l) )); then
            echo "  ❌ $file: ${COVERAGE}% (needs >65%)"
            CRITICAL_GAPS=$((CRITICAL_GAPS + 1))
        fi
    fi
done

# Check security files
for pattern in "tls" "cert" "auth"; do
    FOUND_FILES=$(cat target/coverage/tarpaulin-report.json | jq -r --arg pattern "$pattern" '.files[] | select(if .path | type == "array" then (.path | join("/") | test($pattern)) else (.path | test($pattern)) end) | if .path | type == "array" then "\(.path | join("/")):\(.coverage)" else "\(.path):\(.coverage)" end' 2>/dev/null)
    if [ -n "$FOUND_FILES" ]; then
        while IFS=: read -r filepath coverage; do
            if (( $(echo "$coverage < 65" | bc -l) )); then
                echo "  ❌ $filepath: ${coverage}% (needs >65%)"
                CRITICAL_GAPS=$((CRITICAL_GAPS + 1))
            fi
        done <<< "$FOUND_FILES"
    fi
done

if [ $CRITICAL_GAPS -eq 0 ]; then
    echo "  ✅ No critical gaps found"
fi

echo ""

# HIGH: Transport and Error Handling (must be > 65%)
echo "⚠️  HIGH (Transport & Error Handling - Must be >65%):"
echo "----------------------------------------------------"

# Check transport files
FOUND_FILES=$(cat target/coverage/tarpaulin-report.json | jq -r '.files[] | select(if .path | type == "array" then (.path | join("/") | test("transport|connection")) else (.path | test("transport|connection")) end) | if .path | type == "array" then "\(.path | join("/")):\(.coverage)" else "\(.path):\(.coverage)" end' 2>/dev/null)
if [ -n "$FOUND_FILES" ]; then
    while IFS=: read -r filepath coverage; do
        if (( $(echo "$coverage < 65" | bc -l) )); then
            echo "  ❌ $filepath: ${coverage}% (needs >65%)"
            HIGH_GAPS=$((HIGH_GAPS + 1))
        fi
    done <<< "$FOUND_FILES"
fi

if [ $HIGH_GAPS -eq 0 ]; then
    echo "  ✅ No high priority gaps found"
fi

echo ""

# MEDIUM: Code Generation and Streaming (must be > 65%)
echo "📋 MEDIUM (Codegen & Streaming - Must be >65%):"
echo "----------------------------------------------"

# Check codegen files
FOUND_FILES=$(cat target/coverage/tarpaulin-report.json | jq -r '.files[] | select(if .path | type == "array" then (.path | join("/") | test("codegen|rpcnet-gen")) else (.path | test("codegen|rpcnet-gen")) end) | if .path | type == "array" then "\(.path | join("/")):\(.coverage)" else "\(.path):\(.coverage)" end' 2>/dev/null)
if [ -n "$FOUND_FILES" ]; then
    while IFS=: read -r filepath coverage; do
        if (( $(echo "$coverage < 65" | bc -l) )); then
            echo "  ❌ $filepath: ${coverage}% (needs >65%)"
            MEDIUM_GAPS=$((MEDIUM_GAPS + 1))
        fi
    done <<< "$FOUND_FILES"
fi

# Check streaming files
FOUND_FILES=$(cat target/coverage/tarpaulin-report.json | jq -r '.files[] | select(if .path | type == "array" then (.path | join("/") | test("streaming|stream")) else (.path | test("streaming|stream")) end) | if .path | type == "array" then "\(.path | join("/")):\(.coverage)" else "\(.path):\(.coverage)" end' 2>/dev/null)
if [ -n "$FOUND_FILES" ]; then
    while IFS=: read -r filepath coverage; do
        if (( $(echo "$coverage < 65" | bc -l) )); then
            echo "  ❌ $filepath: ${coverage}% (needs >65%)"
            MEDIUM_GAPS=$((MEDIUM_GAPS + 1))
        fi
    done <<< "$FOUND_FILES"
fi

if [ $MEDIUM_GAPS -eq 0 ]; then
    echo "  ✅ No medium priority gaps found"
fi

echo ""

# LOW: Utilities and Helpers (must be > 65%)
echo "ℹ️  LOW (Utilities - Must be >65%):"
echo "-----------------------------------"

FOUND_FILES=$(cat target/coverage/tarpaulin-report.json | jq -r '.files[] | select(if .path | type == "array" then (.path | join("/") | test("util|helper|metrics")) else (.path | test("util|helper|metrics")) end) | if .path | type == "array" then "\(.path | join("/")):\(.coverage)" else "\(.path):\(.coverage)" end' 2>/dev/null)
if [ -n "$FOUND_FILES" ]; then
    while IFS=: read -r filepath coverage; do
        if (( $(echo "$coverage < 65" | bc -l) )); then
            echo "  ❌ $filepath: ${coverage}% (needs >65%)"
            LOW_GAPS=$((LOW_GAPS + 1))
        fi
    done <<< "$FOUND_FILES"
fi

if [ $LOW_GAPS -eq 0 ]; then
    echo "  ✅ No low priority gaps found"
fi

echo ""
echo "📈 Gap Summary:"
echo "==============="
echo "🚨 Critical gaps: $CRITICAL_GAPS"
echo "⚠️  High gaps: $HIGH_GAPS"  
echo "📋 Medium gaps: $MEDIUM_GAPS"
echo "ℹ️  Low gaps: $LOW_GAPS"

TOTAL_GAPS=$((CRITICAL_GAPS + HIGH_GAPS + MEDIUM_GAPS + LOW_GAPS))
echo "📊 Total gaps: $TOTAL_GAPS"

echo ""
echo "💡 Recommendations:"
echo "==================="

if [ $CRITICAL_GAPS -gt 0 ]; then
    echo "🚨 IMMEDIATE ACTION REQUIRED: Fix critical gaps in security and core RPC"
    echo "   Add unit tests for error paths and edge cases"
    echo "   Add integration tests for TLS certificate validation"
fi

if [ $HIGH_GAPS -gt 0 ]; then
    echo "⚠️  URGENT: Address transport layer coverage gaps"
    echo "   Add tests for connection failures and recovery"
    echo "   Test QUIC protocol edge cases"
fi

if [ $MEDIUM_GAPS -gt 0 ]; then
    echo "📋 MODERATE: Improve code generation and streaming tests"
    echo "   Add end-to-end tests for generated code"
    echo "   Test streaming error scenarios"
fi

if [ $LOW_GAPS -gt 0 ]; then
    echo "ℹ️  LOW: Add tests for utility functions when time permits"
fi

echo ""
echo "🔍 Uncovered Lines (Top 20):"
echo "============================="
cargo tarpaulin --print-uncovered-lines --exclude-files "examples/*" --exclude-files "benches/*" --all-features 2>/dev/null | head -20

# Exit with appropriate code
if [ $CRITICAL_GAPS -gt 0 ] || [ $HIGH_GAPS -gt 0 ]; then
    echo ""
    echo "❌ Coverage gaps require immediate attention"
    exit 1
elif [ $MEDIUM_GAPS -gt 0 ]; then
    echo ""
    echo "⚠️  Coverage gaps should be addressed soon"
    exit 1
else
    echo ""
    echo "✅ No significant coverage gaps found"
    exit 0
fi