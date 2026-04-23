# wasm/

Workspace-level WASM staging for artifact comparison.

This directory is not a Rust crate. It only exists to:

- build and stage Zig and Rust `.wasm` artifacts
- keep staged outputs under `wasm/artifacts/`
- document the comparison flow

The comparison binary lives in `crates/md-docrs-wasm-compare`.

## What it contains

- `build.sh` — builds and stages available artifacts
- `artifacts/` — staged `.wasm` files used by the comparison harness
- `README.md` — this file

## Artifact workflow

From the repo root:

```bash
./wasm/build.sh
cargo run -p md-docrs-wasm-compare -- --offline
```

`build.sh` does this:

- builds Zig minimal WASM
- attempts Zig full WASM and skips it cleanly if unsupported
- builds Rust minimal WASM from `crates/md-docrs-rust-wasm`
- builds Rust full WASM from `crates/md-docrs-rust-wasm`
- runs `wasm-opt -Oz` on Rust artifacts
- copies staged outputs into `wasm/artifacts/`

## Expected staged files

The harness looks for these filenames:

- `zig-minimal.wasm`
- `zig-full.wasm`
- `rust-minimal.wasm`
- `rust-minimal-opt.wasm`
- `rust-full.wasm`
- `rust-full-opt.wasm`

Missing files are skipped.

## Required tools

You need:

- Rust with `wasm32-unknown-unknown`
- Zig
- `wasm-opt`

## Related paths

- `crates/md-docrs-rust-wasm` — Rust WASM module
- `crates/md-docrs-wasm-compare` — host comparison harness
- `zig/` — Zig implementation
- `wasm/artifacts/` — staged outputs

## Notes

Keep `wasm/` boring:

- no Rust crate here
- no shared library logic here
- no comparison logic here

It is only the staging area for cross-language WASM artifacts.