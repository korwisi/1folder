#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."
cargo build --lib
cd desktop
swift build
