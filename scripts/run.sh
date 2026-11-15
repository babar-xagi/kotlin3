#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BASE_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

# Ensure release binary of k3 is built
if [[ ! -f "$BASE_DIR/target/release/k3" ]]; then
    echo "--> Compiling Kotlin3 release binary..."
    cargo build --release --manifest-path "$BASE_DIR/Cargo.toml"
fi

# Execute benchmark runner with environment parameters
python3 "$SCRIPT_DIR/benchmark_runner.py"
