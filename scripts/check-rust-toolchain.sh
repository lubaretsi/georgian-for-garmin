#!/usr/bin/env bash
# Fails unless rust-toolchain.toml (release builds) names the same Rust as the pinned nixpkgs
# (dev shell and CI checks), so a release is built by the compiler the checks ran.
set -euo pipefail
cd "$(dirname "$0")/.."

pinned=$(sed -n 's/^channel = "\(.*\)"$/\1/p' rust-toolchain.toml)
nixpkgs=$(nix eval --raw --inputs-from . nixpkgs#rustc.version)
if [[ $pinned != "$nixpkgs" ]]; then
  echo "error: rust-toolchain.toml pins Rust ${pinned:-(none)}, but flake.lock's nixpkgs has $nixpkgs" >&2
  exit 1
fi
echo "Rust $pinned in rust-toolchain.toml and nixpkgs"
