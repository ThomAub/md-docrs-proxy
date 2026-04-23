# md-docrs

`md-docrs` is a CLI that renders docs.rs rustdoc JSON as Markdown.

It lets you query a crate, module, type, trait, function, or other documented item with a compact spec:

```text
crate[@version][::path::to::item]
```

## Quick start

Run the CLI directly from the workspace:

```bash
cargo run -p md-docrs-cli -- anyhow
cargo run -p md-docrs-cli -- anyhow::Error
cargo run -p md-docrs-cli -- tokio::sync::Mutex
cargo run -p md-docrs-cli -- tokio@1.52.1::sync::Mutex
cargo run -p md-docrs-cli -- --target x86_64-unknown-linux-gnu tokio::sync::Mutex
```

Output is written to stdout as Markdown.

## What this repository contains

This repository is a Rust workspace centered on the `md-docrs` CLI, plus related server, worker, and WASM targets for some explorative work on wasm and zig. 

### Crates

- `crates/md-docrs-cli` — the `md-docrs` command-line tool
- `crates/md-docrs-core` — spec parsing, docs.rs resolution, rendering, and cache abstractions
- `crates/md-docrs-fetch-http` — native HTTP fetcher for docs.rs rustdoc JSON
- `crates/md-docrs-server` — native HTTP server that serves Markdown
- `crates/md-docrs-worker` — Cloudflare Worker for a hosted HTTP version
- `crates/md-docrs-rust-wasm` — Rust WASM export layer
- `crates/md-docrs-wasm-compare` — host-side comparison harness for WASM builds

### Other directories

- `zig/` — minimal Zig URL-resolution implementation for fun and compare WASM size
- `wasm/` — WASM artifacts and comparison build flow

## Hosted MCP

There is also a Cloudflare Worker-based hosted path in `crates/md-docrs-worker`.
Ongoing work!

## Release packaging

This repository is configured thanks to [`cargo-dist`](https://axodotdev.github.io/cargo-dist/) and [`cargo-release`](https://github.com/crate-ci/cargo-release)
