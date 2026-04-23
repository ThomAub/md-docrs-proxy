# md-docrs-rust-wasm

Rust `wasm32-unknown-unknown` export layer for `md-docrs`.

This crate packages the core `md-docrs` logic as a WebAssembly module so host environments can call a small, stable ABI. It is primarily used for WASM artifact comparison and for browser/edge-style hosts that want Rust-based docs.rs resolution and Markdown rendering.

## What this crate owns

- Rust WASM exports for the `md-docrs` pipeline
- ABI-compatible exports for comparison with the Zig WASM module
- optional render-only and fetch-enabled WASM builds
- the Rust side of the staged artifacts under `wasm/`

It does not own:

- the native CLI (`crates/md-docrs-cli`)
- the native HTTP server (`crates/md-docrs-server`)
- the Cloudflare Worker app (`crates/md-docrs-worker`)
- artifact staging orchestration (`wasm/build.sh`)
- the Zig comparison implementation (`zig/`)

## Features

This crate has three feature modes:

- default: `render`
- `fetch`
- `full`

Feature behavior:

- `render`
  - enables JSON-to-Markdown rendering
  - expects the host to provide rustdoc JSON bytes
- `fetch`
  - enables in-WASM fetching and zstd decoding support
- `full`
  - convenience alias for `render + fetch`

## Build variants

Minimal ABI-compatible build:

```bash
cargo build --profile wasm-release --target wasm32-unknown-unknown \
  -p md-docrs-rust-wasm --no-default-features
```

Default render build:

```bash
cargo build --profile wasm-release --target wasm32-unknown-unknown \
  -p md-docrs-rust-wasm
```

Full build with render + fetch support:

```bash
cargo build --profile wasm-release --target wasm32-unknown-unknown \
  -p md-docrs-rust-wasm --no-default-features --features full
```

## Relationship to the Zig WASM module

This crate is designed to match the minimal ABI surface used by the Zig implementation where possible so the host-side comparison harness can exercise both modules with the same calling convention.

That comparison flow is managed from the repository root:

```bash
./wasm/build.sh
cargo run -p md-docrs-wasm-compare -- --offline
```

## Typical role in the workspace

Common flows:

- build Rust WASM artifacts for size and behavior comparison
- export URL resolution in a minimal WASM-compatible ABI
- export Markdown rendering for host-driven integrations
- serve as the Rust-side module for staged WASM artifacts in `wasm/artifacts/`

## Related crates and directories

- `crates/md-docrs-core` — shared spec parsing, resolution, rendering, cache traits
- `crates/md-docrs-wasm-compare` — host-side comparison harness
- `wasm/` — staged artifact workflow
- `zig/` — Zig implementation with a matching minimal comparison target

## Notes

- target is `wasm32-unknown-unknown`
- the workspace defines a dedicated `wasm-release` profile for optimized builds
- this crate is not published independently
- use `wasm/build.sh` when you want the repo-level staged artifact workflow instead of a single direct build