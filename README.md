# md-docrs-proxy

`md-docrs-proxy` resolves docs.rs rustdoc JSON URLs and renders rustdoc JSON as Markdown.

## Workspace

Rust crates under `crates/`:

- `md-docrs-core` — shared spec parsing, docs.rs resolution, rustdoc JSON rendering, cache traits
- `md-docrs-fetch-http` — native HTTP fetcher for docs.rs
- `md-docrs-cli` — native CLI that prints Markdown to stdout
- `md-docrs-server` — native HTTP server
- `md-docrs-worker` — Cloudflare Worker crate
- `md-docrs-rust-wasm` — Rust `wasm32-unknown-unknown` export layer
- `md-docrs-wasm-compare` — host-side WASM comparison harness

Other top-level directories:

- `zig/` — Zig implementation of the minimal `resolve_url` ABI, plus its Worker wrapper
- `wasm/` — staged WASM artifacts and the repo-level build script

## What each path owns

- `crates/` owns the Rust implementation
- `zig/` owns the minimal Zig implementation
- `wasm/` owns artifact staging for Rust/Zig WASM comparison

The top-level `wasm/` directory is not a Cargo crate.

## Build and test

Build the Rust workspace:

```/dev/null/build.sh#L1-1
cargo build --workspace
```

Run the Rust tests:

```/dev/null/test.sh#L1-1
cargo test --workspace
```

Run the Zig tests from the repo root:

```/dev/null/zig-test.sh#L1-1
zig build test --build-file zig/lib/build.zig
```

## Native CLI

The CLI binary comes from `md-docrs-cli`.

Spec grammar:

```/dev/null/spec.txt#L1-1
crate[@version][::path::to::item]
```

Examples:

```/dev/null/cli-examples.sh#L1-5
cargo run -p md-docrs-cli -- anyhow
cargo run -p md-docrs-cli -- anyhow::Error
cargo run -p md-docrs-cli -- tokio::sync::Mutex
cargo run -p md-docrs-cli -- tokio@1.52.1::sync::Mutex
cargo run -p md-docrs-cli -- --target x86_64-unknown-linux-gnu tokio::sync::Mutex
```

Output is Markdown on stdout.

## Native server

The HTTP server binary comes from `md-docrs-server`.

Start it locally:

```/dev/null/server.sh#L1-1
cargo run -p md-docrs-server -- --port 8080 --bind 127.0.0.1
```

Example requests:

```/dev/null/server-curl.sh#L1-4
curl -sS http://127.0.0.1:8080/anyhow
curl -sS http://127.0.0.1:8080/anyhow/latest/anyhow/struct.Error.html
curl -sS http://127.0.0.1:8080/tokio/latest/tokio/sync/struct.Mutex.html
curl -sS http://127.0.0.1:8080/healthz
```

Response behavior:

- `200` with `Content-Type: text/markdown; charset=utf-8`
- `400` for invalid specs
- `404` for missing items
- `502` for upstream, decode, or JSON errors

Optional disk-backed cache support is available behind the `hybrid-cache` feature on `md-docrs-server`.

## Rust WASM

The Rust WASM crate lives at `crates/md-docrs-rust-wasm`.

Minimal build, ABI-compatible with the Zig module:

```/dev/null/rust-wasm-min.sh#L1-2
cargo build --profile wasm-release --target wasm32-unknown-unknown \
  -p md-docrs-rust-wasm --no-default-features
```

Default build adds `render_markdown`:

```/dev/null/rust-wasm-default.sh#L1-2
cargo build --profile wasm-release --target wasm32-unknown-unknown \
  -p md-docrs-rust-wasm
```

Full build adds `render_markdown` and `render_spec`:

```/dev/null/rust-wasm-full.sh#L1-2
cargo build --profile wasm-release --target wasm32-unknown-unknown \
  -p md-docrs-rust-wasm --no-default-features --features full
```

## Zig

The Zig subtree implements the minimal `resolve_url` path.

Common commands:

```/dev/null/zig-commands.sh#L1-3
zig build --build-file zig/lib/build.zig
zig build cli --build-file zig/lib/build.zig
zig build test --build-file zig/lib/build.zig
```

See `zig/README.md` for details.

## WASM comparison

Stage artifacts, then run the comparison harness:

```/dev/null/wasm-compare.sh#L1-2
./wasm/build.sh
cargo run -p md-docrs-wasm-compare -- --offline
```

See `wasm/README.md` for the workflow and supported flags.

## Notes

Current limits:

- in-memory cache by default for native paths
- no disk cache unless `md-docrs-server` is built with `hybrid-cache`
- partial rendering coverage; not all rustdoc surfaces are rendered yet
- Zig currently covers URL resolution only, not fetch/decompress/render