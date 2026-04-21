#!/usr/bin/env bash
set -euo pipefail

# Build and stage the WASM artifacts used by the comparison harness.
#
# Responsibilities:
# - build Zig minimal wasm
# - optionally build Zig full wasm if supported
# - build Rust minimal/full wasm from the workspace
# - run wasm-opt on Rust artifacts
# - copy everything into wasm/artifacts/
#
# This directory is only a staging area. The actual Rust crates live under:
# - crates/md-docrs-rust-wasm
# - crates/md-docrs-wasm-compare

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "${HERE}/.." && pwd)"
ARTIFACTS_DIR="${HERE}/artifacts"
RUST_WASM_PKG="md-docrs-rust-wasm"
RUST_WASM_OUT="${ROOT}/target/wasm32-unknown-unknown/wasm-release/md_docrs_rust_wasm.wasm"
ZIG_DIR="${ROOT}/zig/lib"
STAGED_ARTIFACTS=(
    "zig-minimal.wasm"
    "zig-full.wasm"
    "rust-minimal.wasm"
    "rust-minimal-opt.wasm"
    "rust-full.wasm"
    "rust-full-opt.wasm"
)

mkdir -p "${ARTIFACTS_DIR}"

require_cmd() {
    command -v "$1" >/dev/null 2>&1 || {
        echo "missing required command: $1" >&2
        exit 1
    }
}

copy_if_exists() {
    local src="$1"
    local dest="$2"

    if [[ -f "${src}" ]]; then
        cp "${src}" "${dest}"
        return 0
    fi

    return 1
}

optimize_wasm() {
    local src="$1"
    local dest="$2"

    wasm-opt -Oz \
        --enable-bulk-memory \
        --strip-debug \
        --strip-dwarf \
        -o "${dest}" \
        "${src}"
}

build_zig_minimal() {
    echo ">> zig-minimal"
    (
        cd "${ZIG_DIR}"
        zig build
    )
    copy_if_exists \
        "${ZIG_DIR}/zig-out/bin/md-docrs.wasm" \
        "${ARTIFACTS_DIR}/zig-minimal.wasm"
}

build_zig_full() {
    echo ">> zig-full"
    if (
        cd "${ZIG_DIR}"
        zig build -Dfull >/dev/null 2>&1
    ); then
        if copy_if_exists \
            "${ZIG_DIR}/zig-out/bin/md-docrs-full.wasm" \
            "${ARTIFACTS_DIR}/zig-full.wasm"; then
            :
        else
            echo "   skipped: build accepted -Dfull but produced no md-docrs-full.wasm"
        fi
    else
        echo "   skipped: Zig full wasm is not implemented yet"
    fi
}

build_rust() {
    local label="$1"
    shift

    echo ">> ${label}"
    cargo build \
        --manifest-path "${ROOT}/Cargo.toml" \
        --profile wasm-release \
        --target wasm32-unknown-unknown \
        -p "${RUST_WASM_PKG}" \
        "$@"
}

stage_rust_artifact() {
    local raw_name="$1"
    local opt_name="$2"

    copy_if_exists "${RUST_WASM_OUT}" "${ARTIFACTS_DIR}/${raw_name}"
    optimize_wasm "${ARTIFACTS_DIR}/${raw_name}" "${ARTIFACTS_DIR}/${opt_name}"
}

main() {
    require_cmd cargo
    require_cmd zig
    require_cmd wasm-opt

    rm -f "${STAGED_ARTIFACTS[@]/#/${ARTIFACTS_DIR}/}"

    build_zig_minimal
    build_zig_full

    build_rust "rust-minimal" --no-default-features
    stage_rust_artifact "rust-minimal.wasm" "rust-minimal-opt.wasm"

    build_rust "rust-full" --no-default-features --features full
    stage_rust_artifact "rust-full.wasm" "rust-full-opt.wasm"

    echo
    echo "staged artifacts:"
    ls -la "${ARTIFACTS_DIR}"
}

main "$@"
