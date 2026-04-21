#!/usr/bin/env bash
# Build the Zig and Rust wasm artifacts and stage them under artifacts/ so
# the comparison harness (cargo run -p md-docrs-wasm-compare) can load them
# without knowing where each toolchain drops its output.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "${HERE}/.." && pwd)"
ARTIFACTS="${HERE}/artifacts"

mkdir -p "${ARTIFACTS}"

echo ">> zig: ReleaseSmall, wasm32-freestanding"
(cd "${ROOT}/zig/lib" && zig build)
cp "${ROOT}/zig/lib/zig-out/bin/md-docrs.wasm" "${ARTIFACTS}/zig.wasm"

echo ">> rust-minimal: wasm-release, --no-default-features (resolve_url only)"
cargo build --manifest-path "${ROOT}/Cargo.toml" \
    --profile wasm-release --target wasm32-unknown-unknown \
    -p md-docrs-wasm --no-default-features
cp "${ROOT}/target/wasm32-unknown-unknown/wasm-release/md_docrs_wasm.wasm" \
   "${ARTIFACTS}/rust-minimal.wasm"

echo ">> rust-full: wasm-release, default features (+render_markdown)"
cargo build --manifest-path "${ROOT}/Cargo.toml" \
    --profile wasm-release --target wasm32-unknown-unknown \
    -p md-docrs-wasm
cp "${ROOT}/target/wasm32-unknown-unknown/wasm-release/md_docrs_wasm.wasm" \
   "${ARTIFACTS}/rust-full.wasm"

echo
echo "staged artifacts:"
ls -la "${ARTIFACTS}"
