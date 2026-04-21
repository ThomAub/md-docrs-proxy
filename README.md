# md-docrs-proxy

`md-docrs-proxy` resolves docs.rs rustdoc JSON URLs and renders rustdoc JSON as Markdown.

This repository is organized with clear boundaries between:

- **Rust workspace crates** for the real application and shared logic
- **Zig** for a minimal `resolve_url` implementation and Worker wrapper
- **Top-level `wasm/`** for cross-language artifact staging and comparison

## Repository boundaries

### Rust workspace

The Rust implementation lives under `crates/`:

- `crates/md-docrs-core` — pure shared logic
  - spec parsing
  - docs.rs URL resolution
  - rustdoc JSON rendering
  - cache abstractions and shared types
- `crates/md-docrs-cli` — native CLI and local HTTP server
- `crates/md-docrs-worker` — Cloudflare Worker crate for the Rust side
- `crates/md-docrs-rust-wasm` — Rust `wasm32-unknown-unknown` build exposing the WASM ABI
- `crates/md-docrs-wasm-compare` — host-side comparison harness for staged `.wasm` artifacts

### Zig

The Zig implementation lives under `zig/`:

- `zig/lib` — Zig source for:
  - spec parsing
  - docs.rs URL building
  - minimal WASM ABI
  - native Zig CLI
- `zig/src` — TypeScript Cloudflare Worker wrapper for the Zig wasm module

Zig is intentionally narrow in scope today: it is the minimal `resolve_url` implementation, not the full Markdown rendering pipeline.

### Top-level wasm harness

The top-level `wasm/` directory is **not** a Cargo crate anymore.

It exists only for repo-level WASM workflow:

- `wasm/build.sh` — builds/stages Zig and Rust wasm artifacts into `wasm/artifacts/`
- `wasm/artifacts/` — generated staged artifacts used by the comparison harness
- `wasm/README.md` — docs for the comparison flow

The actual comparison binary lives in:

- `crates/md-docrs-wasm-compare`

## Build

Build the Rust workspace:

```sh
cargo build --workspace
```

## Native CLI

The main native binary is provided by `md-docrs-cli`.

Spec grammar:

```text
crate[@version][::path::to::item]
```

Version defaults to `latest`.

Examples:

```sh
cargo run -p md-docrs-cli -- anyhow
cargo run -p md-docrs-cli -- anyhow::Error
cargo run -p md-docrs-cli -- tokio::sync::Mutex
cargo run -p md-docrs-cli -- tokio@1.52.1::sync::Mutex
cargo run -p md-docrs-cli -- --target x86_64-unknown-linux-gnu tokio::sync::Mutex
```

Not every `@version` pin works: docs.rs must have rebuilt rustdoc JSON for the supported format version for that exact release. Older releases may return `502`; in that case use a newer version or `latest`.

Markdown goes to stdout.

## Local server

The native server also comes from `md-docrs-cli`.

```sh
cargo run -p md-docrs-cli -- serve --port 8080 --bind 127.0.0.1
```

Examples:

```sh
curl -s localhost:8080/anyhow
curl -s localhost:8080/anyhow/latest/anyhow/struct.Error.html
curl -s localhost:8080/tokio/latest/tokio/sync/struct.Mutex.html
```

Response shape:

- `Content-Type: text/markdown; charset=utf-8`
- `X-Markdown-Tokens`
- `Vary: Accept`

Status codes:

- `400` bad spec
- `404` item not found
- `502` upstream/decode error

## Rust WASM

The Rust WASM module lives in:

- `crates/md-docrs-rust-wasm`

It exposes the shared ABI used for side-by-side comparison with Zig:

- `alloc`
- `free`
- `resolve_url`
- optionally `render_markdown`

### Minimal Rust WASM build

This is the closest match to the current Zig WASM surface.

```sh
cargo build --profile wasm-release --target wasm32-unknown-unknown \
  -p md-docrs-rust-wasm --no-default-features
```

### Full Rust WASM build

This adds `render_markdown`.

```sh
cargo build --profile wasm-release --target wasm32-unknown-unknown \
  -p md-docrs-rust-wasm --no-default-features --features full
```

## Zig

See:

- [`zig/README.md`](zig/README.md)

Typical Zig commands:

```sh
zig build --build-file zig/lib/build.zig
zig build cli --build-file zig/lib/build.zig
zig build test --build-file zig/lib/build.zig
```

## WASM comparison harness

Use the top-level `wasm/` directory to stage artifacts, then run the Rust comparison harness.

```sh
./wasm/build.sh
cargo run -p md-docrs-wasm-compare -- --offline
```

For full docs, see:

- [`wasm/README.md`](wasm/README.md)

## Notes

- In-memory LRU cache only for the native process path
- No disk cache by default
- v0 does not render trait impls, blanket impls, or source links
- Glob re-exports into external crates are not fully followed

## Logging

```sh
RUST_LOG=debug cargo run -p md-docrs-cli -- serve
```
