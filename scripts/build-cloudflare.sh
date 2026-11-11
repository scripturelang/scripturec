#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

# Workers Builds does not preinstall Rust. Keep its toolchain aligned with
# the version used to validate this deployment.
export RUSTUP_TOOLCHAIN=1.96.0
if ! command -v rustup >/dev/null 2>&1; then
  installer="$(mktemp)"
  trap 'rm -f "$installer"' EXIT
  curl --proto '=https' --tlsv1.2 -fsSL https://sh.rustup.rs -o "$installer"
  sh "$installer" -y --profile minimal --default-toolchain none
  source "${CARGO_HOME:-$HOME/.cargo}/env"
fi

rustup toolchain install "$RUSTUP_TOOLCHAIN" --profile minimal --target wasm32-unknown-unknown
cargo web
