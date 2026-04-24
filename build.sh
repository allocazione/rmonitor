#!/usr/bin/env bash
set -e

# Colors
GREEN='\033[0;32m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m' # No Color
BOLD='\033[1m'

# Build rmonitor and copy the executable into release/linux
SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
cd "$SCRIPT_DIR"

echo -e "${BLUE}${BOLD}╔══════════════════════════════════════════════════════╗${NC}"
echo -e "${BLUE}${BOLD}║             rmonitor Build System                    ║${NC}"
echo -e "${BLUE}${BOLD}╚══════════════════════════════════════════════════════╝${NC}"
echo ""

# Dependency Checks
echo -e "${CYAN}Checking dependencies...${NC}"
MISSING_DEPS=()

if ! command -v cargo &> /dev/null; then
    MISSING_DEPS+=("rust/cargo")
fi

if [[ "$OSTYPE" == "linux-gnu"* ]]; then
    if ! command -v pkg-config &> /dev/null; then
        MISSING_DEPS+=("pkg-config")
    fi
    if ! command -v clang &> /dev/null; then
        MISSING_DEPS+=("clang")
    fi
    if ! command -v lld &> /dev/null; then
        MISSING_DEPS+=("lld")
    fi
    # Check for openssl headers (rough check via pkg-config if it exists)
    if command -v pkg-config &> /dev/null; then
        if ! pkg-config --exists openssl; then
            MISSING_DEPS+=("openssl-dev")
        fi
    fi
fi

if [ ${#MISSING_DEPS[@]} -ne 0 ]; then
    echo -e "${RED}${BOLD}Error: Missing dependencies: ${MISSING_DEPS[*]}${NC}"
    echo -e "${YELLOW}Please refer to the Prerequisites section in README.md for installation instructions.${NC}"
    exit 1
fi

START_TIME=$(date +%s)
echo -e "${CYAN}Building rmonitor (Release mode)...${NC}"
cargo build --release --quiet

OS_NAME=$(uname -s | tr '[:upper:]' '[:lower:]')
OUT_DIR="release/$OS_NAME"

mkdir -p "$OUT_DIR"

if [ -f "target/release/rmonitor" ]; then
    cp "target/release/rmonitor" "$OUT_DIR/rmonitor"
    END_TIME=$(date +%s)
    DURATION=$((END_TIME - START_TIME))
    echo ""
    echo -e "${GREEN}${BOLD}Build complete!${NC} (Duration: ${DURATION}s)"
    echo -e "${GREEN}Executable located at: ${BOLD}$OUT_DIR/rmonitor${NC}"
    echo ""
    
    # Path management
    ABS_OUT_DIR="$PWD/$OUT_DIR"
    if [[ ":$PATH:" != *":$ABS_OUT_DIR:"* ]]; then
        ALREADY_IN_CONFIG=false
        SHELL_CONFIG=""
        if [ -f "$HOME/.bashrc" ]; then
            SHELL_CONFIG="$HOME/.bashrc"
        elif [ -f "$HOME/.zshrc" ]; then
            SHELL_CONFIG="$HOME/.zshrc"
        elif [ -f "$HOME/.profile" ]; then
            SHELL_CONFIG="$HOME/.profile"
        fi

        if [ -n "$SHELL_CONFIG" ] && grep -q "export PATH=.*$ABS_OUT_DIR" "$SHELL_CONFIG"; then
            ALREADY_IN_CONFIG=true
        fi

        if [ "$ALREADY_IN_CONFIG" = false ]; then
            echo -e "${YELLOW}To run 'rmonitor' from anywhere, you can add the release directory to your PATH.${NC}"
            read -p "Do you want to add $ABS_OUT_DIR to your PATH? (Y/N): " add_path
            if [[ "$add_path" =~ ^[Yy]$ ]]; then
                if [ -n "$SHELL_CONFIG" ]; then
                    echo "" >> "$SHELL_CONFIG"
                    echo "# rmonitor path" >> "$SHELL_CONFIG"
                    echo "export PATH=\"\$PATH:$ABS_OUT_DIR\"" >> "$SHELL_CONFIG"
                    echo -e "${GREEN}Directory added to $SHELL_CONFIG.${NC}"
                    echo -e "${YELLOW}Please restart your shell or run 'source $SHELL_CONFIG'.${NC}"
                else
                    echo -e "${RED}Could not find a shell config file (.bashrc, .zshrc, or .profile).${NC}"
                    echo -e "Please add ${BOLD}$ABS_OUT_DIR${NC} to your PATH manually."
                fi
            fi
            echo ""
        fi
    fi

    RUN_CMD="./$OUT_DIR/rmonitor"
    RUN_PROMPT="Do you want to run the program now? (Y/N): "

    if [[ "$OSTYPE" == "linux-gnu"* ]] && [ "$EUID" -ne 0 ]; then
        RUN_PROMPT="Do you want to run the program now (with sudo)? (Y/N): "
        RUN_CMD="sudo env \"PATH=$PATH\" ./$OUT_DIR/rmonitor"
    fi

    read -p "$RUN_PROMPT" run
    if [[ "$run" =~ ^[Yy]$ ]]; then
        if [[ "$RUN_CMD" == sudo* ]]; then
            echo -e "${CYAN}Elevating privileges for full security log access...${NC}"
        fi
        $RUN_CMD
    fi
else
    echo -e "${RED}${BOLD}Error: Could not find compiled executable at target/release/rmonitor${NC}"
    exit 1
fi
