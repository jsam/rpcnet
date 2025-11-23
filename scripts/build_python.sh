#!/bin/bash
# Robust Python extension build script for RpcNet
# This ensures a clean build every time and avoids stale module issues

set -e  # Exit on error

echo "🧹 Cleaning old Python builds..."

# 1. Remove all potential .so files
find . -name "_rpcnet*.so" -delete 2>/dev/null || true
find . -name "librpcnet*.so" -delete 2>/dev/null || true
find . -name "librpcnet*.dylib" -delete 2>/dev/null || true

# 2. Clean Python cache
find . -type d -name "__pycache__" -exec rm -rf {} + 2>/dev/null || true
find . -type f -name "*.pyc" -delete 2>/dev/null || true
find . -type f -name "*.pyo" -delete 2>/dev/null || true

# 3. Remove build artifacts
rm -rf target/wheels/ 2>/dev/null || true
rm -rf .pytest_cache/ 2>/dev/null || true
rm -rf build/ dist/ *.egg-info/ 2>/dev/null || true

# 4. Clean cargo build (only Python-related artifacts)
echo "🧹 Cleaning cargo build artifacts..."
cargo clean -p rpcnet 2>/dev/null || true

# 5. Ensure we're using the venv Python
if [ ! -d ".venv" ]; then
    echo "❌ No .venv directory found. Please create a virtual environment first:"
    echo "   python3 -m venv .venv"
    echo "   source .venv/bin/activate"
    exit 1
fi

# Activate venv if not already activated
if [ -z "$VIRTUAL_ENV" ]; then
    echo "🔧 Activating virtual environment..."
    source .venv/bin/activate
fi

echo "🐍 Using Python: $(which python3)"
echo "   Version: $(python3 --version)"

# 6. Build with maturin
echo ""
echo "🔨 Building Python extension with maturin..."
maturin develop --features extension-module

echo ""
echo "✅ Build complete!"
echo ""

# 7. Verify the module can be imported
echo "🔍 Verifying module import..."
python3 -c "import _rpcnet; print('✅ _rpcnet module imported successfully')" || {
    echo "❌ Failed to import _rpcnet module"
    exit 1
}

# 8. Show available classes
echo ""
echo "📦 Available _rpcnet classes:"
python3 -c "
import _rpcnet
classes = [x for x in dir(_rpcnet) if not x.startswith('_')]
for cls in sorted(classes):
    print(f'   - {cls}')
"

echo ""
echo "🎉 Python extension build successful!"
echo ""
echo "💡 To run tests:"
echo "   pytest tests/test_python_*.py -v"
