#!/usr/bin/env bash
set -e

SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
cd "$SCRIPT_DIR"

# Dependency Checks
MISSING_DEPS=()
if ! command -v cargo &> /dev/null; then MISSING_DEPS+=("rust/cargo"); fi

if [[ "$OSTYPE" == "linux-gnu"* ]]; then
    if ! command -v pkg-config &> /dev/null; then MISSING_DEPS+=("pkg-config"); fi
    if ! command -v clang &> /dev/null; then MISSING_DEPS+=("clang"); fi
    if ! command -v lld &> /dev/null; then MISSING_DEPS+=("lld"); fi
    if command -v pkg-config &> /dev/null && ! pkg-config --exists openssl; then MISSING_DEPS+=("openssl-dev"); fi
fi

if [ ${#MISSING_DEPS[@]} -ne 0 ]; then
    echo "Error: Missing dependencies: ${MISSING_DEPS[*]}"
    exit 1
fi

echo -n "Building rmonitor... "
START_TIME=$(date +%s)
cargo build --release --quiet

OS_NAME=$(uname -s | tr '[:upper:]' '[:lower:]')
OUT_DIR="release/$OS_NAME"
mkdir -p "$OUT_DIR"

if [ -f "target/release/rmonitor" ]; then
    cp "target/release/rmonitor" "$OUT_DIR/rmonitor"
    sudo cp "target/release/rmonitor" "/usr/local/bin/rmonitor"
    DURATION=$(($(date +%s) - START_TIME))
    echo "Done! (${DURATION}s)"
    echo "Binary: /usr/local/bin/rmonitor"
else
    echo "Build failed."
    exit 1
fi
