# md-docrs-rust-wasm

Rust `wasm32-unknown-unknown` crate for the workspace's WASM-facing ABI.

This crate is intentionally narrow:

- it exposes a small C-style ABI for hosts
- it reuses shared Rust logic from `md-docrs-core`
- it does not own the comparison harness
- it does not own the Zig implementation
- it does not own the Cloudflare Worker wrapper

That separation keeps boundaries clear:

- `crates/md-docrs-core` — shared Rust parsing / resolution / rendering logic
- `crates/md-docrs-rust-wasm` — Rust WASM export layer
- `crates/md-docrs-wasm-compare` — host-side comparison harness
- `zig/` — independent Zig implementation and Worker wrapper
- `wasm/` — staged artifacts and helper build script

## Purpose

`md-docrs-rust-wasm` builds a WebAssembly module that can be loaded by any host that understands its exported ABI.

Today it supports two scopes:

- **minimal**: `resolve_url` only
- **full**: `resolve_url` + `render_markdown`

The minimal build is the direct Rust counterpart to the Zig WASM module.  
The full build keeps the same base ABI and adds Markdown rendering.

## Exports

The module exports:

| Symbol | Signature | Notes |
| --- | --- | --- |
| `alloc` | `(len: u32) -> *mut u8` | Allocates a buffer in WASM linear memory. Returns null on failure or `len == 0`. |
| `free` | `(ptr: *mut u8, len: u32)` | Frees a buffer previously returned by `alloc`. Length must match. |
| `resolve_url` | `(spec_ptr, spec_len, target_ptr, target_len, out_ptr, out_cap) -> u32` | Resolves a docs.rs rustdoc JSON URL into the caller-provided output buffer. Returns bytes written, or `0` on error. |
| `render_markdown` | `(json_ptr, json_len, spec_ptr, spec_len, target_ptr, target_len, len_out: *mut u32) -> *mut u8` | Present in builds with the `render` feature. Returns a newly allocated Markdown buffer; caller must free it. Returns null on error. |

## Build modes

### Minimal build

This is the smallest Rust build and the one intended for direct parity with Zig.

It exposes:

- `alloc`
- `free`
- `resolve_url`

Build it with:

```sh
cargo build --profile wasm-release --target wasm32-unknown-unknown \
  -p md-docrs-rust-wasm --no-default-features
```

Output:

```sh
target/wasm32-unknown-unknown/wasm-release/md_docrs_rust_wasm.wasm
```

### Default build

The default feature set includes `render`.

It exposes:

- `alloc`
- `free`
- `resolve_url`
- `render_markdown`

Build it with:

```sh
cargo build --profile wasm-release --target wasm32-unknown-unknown \
  -p md-docrs-rust-wasm
```

### Full build

The crate also defines a convenience `full` feature:

- `render`
- `fetch`

Build it with:

```sh
cargo build --profile wasm-release --target wasm32-unknown-unknown \
  -p md-docrs-rust-wasm --no-default-features --features full
```

Use this when you want the full WASM-oriented surface used by the repo-level comparison flow.

## Features

| Feature | Default | Meaning |
| --- | --- | --- |
| `render` | yes | Enables JSON-to-Markdown rendering via `serde_json` and `rustdoc-types`, and exports `render_markdown`. |
| `fetch` | no | Enables fetch/decompression-related functionality needed by the full WASM pipeline. |
| `full` | no | Convenience alias for `render` + `fetch`. |

## Workspace boundaries

This crate should stay lean and focused on the ABI layer.

### It should contain

- exported WASM ABI functions
- memory handling for host/WASM interaction
- thin adapters into `md-docrs-core`
- feature-gated WASM-specific integration logic

### It should not contain

- CLI code
- server code
- Cloudflare Worker code
- comparison harness code
- Zig-specific code
- repo-level artifact staging logic

Those live elsewhere on purpose.

## Relationship to Zig

The Zig implementation lives under `zig/`.

The goal is to keep the **minimal ABI compatible** across both implementations so the same host-side logic can load either artifact with minimal or no changes.

That means the Rust minimal build should stay disciplined:

- small export surface
- stable memory protocol
- no unnecessary host assumptions

## Comparison workflow

This crate does not run comparisons itself.

For side-by-side Rust vs Zig comparison, use the repo-level flow:

- `wasm/build.sh` — builds and stages artifacts into `wasm/artifacts/`
- `crates/md-docrs-wasm-compare` — loads those artifacts and benchmarks / checks parity

Typical flow from the repo root:

```sh
./wasm/build.sh
cargo run -p md-docrs-wasm-compare -- --offline
```

## Optimization

If `wasm-opt` is installed, you can post-process the built artifact manually:

```sh
wasm-opt -Oz --strip-debug --strip-dwarf \
  -o target/wasm32-unknown-unknown/wasm-release/md_docrs_rust_wasm.opt.wasm \
  target/wasm32-unknown-unknown/wasm-release/md_docrs_rust_wasm.wasm
```

In normal repo usage, the top-level `wasm/build.sh` script handles staging optimized artifacts.

## Tests

Host tests can still exercise the crate logic:

```sh
cargo test -p md-docrs-rust-wasm
```

## Design guidance

To keep this crate lean over time:

- prefer pushing reusable logic down into `md-docrs-core`
- keep exported functions thin
- keep features explicit
- avoid mixing host/runtime concerns into the ABI layer
- treat code size as a product constraint for the minimal build

If a future change is only needed for:

- CLI behavior
- HTTP serving
- Worker deployment
- harness benchmarking

then it probably belongs outside this crate.