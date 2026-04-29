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
    echo "Error: Missing dependencies:"
    for dep in "${MISSING_DEPS[@]}"; do
        echo "  - $dep"
    done
    echo ""

    # Detect OS and suggest install commands
    RUSTUP_CMD="  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"

    if [[ "$OSTYPE" == "darwin"* ]]; then
        echo "To install the missing dependencies on macOS (Homebrew):"
        for dep in "${MISSING_DEPS[@]}"; do
            case "$dep" in
                rust/cargo)  echo "$RUSTUP_CMD" ;;
                pkg-config)  echo "  brew install pkg-config" ;;
                clang)       echo "  brew install llvm" ;;
                # lld is bundled inside the Homebrew llvm package
                lld)         echo "  brew install llvm  # lld is included in the llvm package" ;;
                openssl-dev) echo "  brew install openssl" ;;
            esac
        done
    elif [[ "$OSTYPE" == "linux-gnu"* ]]; then
        if command -v apt-get &> /dev/null; then
            echo "To install the missing dependencies on Debian/Ubuntu:"
            for dep in "${MISSING_DEPS[@]}"; do
                case "$dep" in
                    rust/cargo)  echo "$RUSTUP_CMD" ;;
                    pkg-config)  echo "  sudo apt-get install -y pkg-config" ;;
                    clang)       echo "  sudo apt-get install -y clang" ;;
                    lld)         echo "  sudo apt-get install -y lld" ;;
                    openssl-dev) echo "  sudo apt-get install -y libssl-dev" ;;
                esac
            done
        elif command -v dnf &> /dev/null; then
            echo "To install the missing dependencies on Fedora/RHEL:"
            for dep in "${MISSING_DEPS[@]}"; do
                case "$dep" in
                    rust/cargo)  echo "$RUSTUP_CMD" ;;
                    pkg-config)  echo "  sudo dnf install -y pkg-config" ;;
                    clang)       echo "  sudo dnf install -y clang" ;;
                    lld)         echo "  sudo dnf install -y lld" ;;
                    openssl-dev) echo "  sudo dnf install -y openssl-devel" ;;
                esac
            done
        elif command -v pacman &> /dev/null; then
            echo "To install the missing dependencies on Arch Linux:"
            for dep in "${MISSING_DEPS[@]}"; do
                case "$dep" in
                    rust/cargo)  echo "$RUSTUP_CMD" ;;
                    pkg-config)  echo "  sudo pacman -S pkg-config" ;;
                    clang)       echo "  sudo pacman -S clang" ;;
                    lld)         echo "  sudo pacman -S lld" ;;
                    openssl-dev) echo "  sudo pacman -S openssl" ;;
                esac
            done
        else
            echo "Please install the missing dependencies using your system's package manager."
        fi
    else
        echo "Please install the missing dependencies for your operating system."
    fi

    exit 1
fi

echo -n "Building rmonitor..."
echo ""
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
