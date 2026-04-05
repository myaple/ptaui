#!/bin/bash
set -euo pipefail

if [ "${CLAUDE_CODE_REMOTE:-}" != "true" ]; then
  exit 0
fi

# Install Rust toolchain components
rustup component add clippy rustfmt 2>/dev/null || true

# Fetch dependencies (uses cached registry when available)
cargo fetch
