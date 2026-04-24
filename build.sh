#!/usr/bin/env bash
set -e

SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
cd "$SCRIPT_DIR"

START_TIME=$(date +%s)
echo "Building rmonitor (Release)..."
cargo build --release --quiet

OS_NAME=$(uname -s | tr '[:upper:]' '[:lower:]')
OUT_DIR="release/$OS_NAME"

mkdir -p "$OUT_DIR"

if [ -f "target/release/rmonitor" ]; then
    cp "target/release/rmonitor" "$OUT_DIR/rmonitor"
    END_TIME=$(date +%s)
    DURATION=$((END_TIME - START_TIME))
    echo "Build complete! (Duration: ${DURATION}s) Executable located at: $OUT_DIR/rmonitor"
    
    read -p "Do you want to run the program now? (Y/N): " run
    if [[ "$run" =~ ^[Yy]$ ]]; then
        "./$OUT_DIR/rmonitor"
    fi

    # Path management
    if [[ ":$PATH:" != *":$PWD:"* ]]; then
        ALREADY_IN_CONFIG=false
        SHELL_CONFIG=""
        if [ -f "$HOME/.bashrc" ]; then
            SHELL_CONFIG="$HOME/.bashrc"
        elif [ -f "$HOME/.zshrc" ]; then
            SHELL_CONFIG="$HOME/.zshrc"
        elif [ -f "$HOME/.profile" ]; then
            SHELL_CONFIG="$HOME/.profile"
        fi

        if [ -n "$SHELL_CONFIG" ] && grep -q "export PATH=.*$PWD" "$SHELL_CONFIG"; then
            ALREADY_IN_CONFIG=true
        fi

        if [ "$ALREADY_IN_CONFIG" = false ]; then
            read -p "Do you want to add this directory to your PATH? (Y/N): " add_path
            if [[ "$add_path" =~ ^[Yy]$ ]]; then
                if [ -n "$SHELL_CONFIG" ]; then
                    echo "export PATH=\"\$PATH:$PWD\"" >> "$SHELL_CONFIG"
                    echo "Directory added to $SHELL_CONFIG. Please restart your shell or run 'source $SHELL_CONFIG'."
                else
                    echo "Could not find a shell config file (.bashrc, .zshrc, or .profile). Please add $PWD to your PATH manually."
                fi
            fi
        fi
    fi
else
    echo "Error: Could not find compiled executable at target/release/rmonitor"
    exit 1
fi
