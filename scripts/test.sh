#!/usr/bin/env bash
set -euo pipefail

cargo build --release

./target/release/fireshot diagnose

if [[ "${FIRESHOT_PORTAL_PING:-}" == "1" ]]; then
  ./target/release/fireshot diagnose --ping
fi
