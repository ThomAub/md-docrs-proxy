#!/usr/bin/env bash
# Build the Zig and Rust wasm artifacts and stage them under artifacts/ so
# the comparison harness (cargo run -p md-docrs-wasm-compare) can load them
# without knowing where each toolchain drops its output.
#
# Produces up to six artifacts:
#   zig-minimal.wasm         Zig ReleaseSmall, resolve_url only
#   zig-full.wasm            Zig ReleaseSmall, full pipeline (if -Dfull supported)
#   rust-minimal.wasm        Rust wasm-release, --no-default-features
#   rust-minimal-opt.wasm    Rust wasm-release + wasm-opt -Oz, --no-default-features
#   rust-full.wasm           Rust wasm-release, --features full (fetch + render)
#   rust-full-opt.wasm       Rust wasm-release + wasm-opt -Oz, --features full
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "${HERE}/.." && pwd)"
ARTIFACTS="${HERE}/artifacts"

mkdir -p "${ARTIFACTS}"

if command -v wasm-opt >/dev/null 2>&1; then
    WASM_OPT="$(command -v wasm-opt)"
    echo ">> wasm-opt: ${WASM_OPT}"
else
    echo "wasm-opt not found in PATH; install Binaryen to produce optimized Rust artifacts" >&2
    exit 1
fi

optimize_wasm() {
    local src="$1"
    local dest="$2"

    "${WASM_OPT}" -Oz --enable-bulk-memory --strip-debug --strip-dwarf -o "${dest}" "${src}"
}

echo ">> zig-minimal: ReleaseSmall, wasm32-freestanding"
(cd "${ROOT}/zig/lib" && zig build)
cp "${ROOT}/zig/lib/zig-out/bin/md-docrs.wasm" "${ARTIFACTS}/zig-minimal.wasm"

echo ">> zig-full: ReleaseSmall + full pipeline (-Dfull)"
if (cd "${ROOT}/zig/lib" && zig build -Dfull 2>/dev/null); then
    if [[ -f "${ROOT}/zig/lib/zig-out/bin/md-docrs-full.wasm" ]]; then
        cp "${ROOT}/zig/lib/zig-out/bin/md-docrs-full.wasm" \
           "${ARTIFACTS}/zig-full.wasm"
    else
        echo "   (skipping: -Dfull accepted but produced no md-docrs-full.wasm)"
    fi
else
    echo "   (skipping: zig -Dfull not supported yet; implement render_spec in zig/lib/)"
fi

echo ">> rust-minimal: wasm-release, --no-default-features (resolve_url only)"
cargo build --manifest-path "${ROOT}/Cargo.toml" \
    --profile wasm-release --target wasm32-unknown-unknown \
    -p md-docrs-wasm --no-default-features
cp "${ROOT}/target/wasm32-unknown-unknown/wasm-release/md_docrs_wasm.wasm" \
   "${ARTIFACTS}/rust-minimal.wasm"
optimize_wasm "${ARTIFACTS}/rust-minimal.wasm" "${ARTIFACTS}/rust-minimal-opt.wasm"

echo ">> rust-full: wasm-release, --features full (fetch + render)"
cargo build --manifest-path "${ROOT}/Cargo.toml" \
    --profile wasm-release --target wasm32-unknown-unknown \
    -p md-docrs-wasm --no-default-features --features full
cp "${ROOT}/target/wasm32-unknown-unknown/wasm-release/md_docrs_wasm.wasm" \
   "${ARTIFACTS}/rust-full.wasm"
optimize_wasm "${ARTIFACTS}/rust-full.wasm" "${ARTIFACTS}/rust-full-opt.wasm"

echo
echo "staged artifacts:"
ls -la "${ARTIFACTS}"
