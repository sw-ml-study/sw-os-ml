#!/usr/bin/env bash
# Boot the native ARM64 MLOS guest and enter K immediately.
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"
exec cargo run -q -p mlos-cli -- run --run k
