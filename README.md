# md-docrs-proxy

Proxy that downloads rustdoc JSON from docs.rs and renders it as Markdown - built for LLM agents that waste tokens scraping docs.rs HTML.

## Build

```sh
cargo build --release
# binary at ./target/release/md-docrs
```

## CLI

Spec grammar: `crate[@version][::path::to::item]`. Version defaults to `latest`.

```sh
md-docrs anyhow                                       # crate index, latest
md-docrs anyhow::Error                                # item page
md-docrs tokio::sync::Mutex                           # follows pub use re-exports
md-docrs tokio@1.52.1::sync::Mutex                    # pinned version
md-docrs --target x86_64-unknown-linux-gnu tokio::sync::Mutex
```

Not every `@version` pin works: docs.rs has to have rebuilt rustdoc JSON at the supported format version (currently 57) for that exact release. Older releases predate the rebuild and return `502`; pin to a recent version or drop the pin to use `latest`.

Markdown goes to stdout. Pipe it into whatever consumes it.

## Server

Mirrors docs.rs URLs, always replies with `text/markdown`:

```sh
md-docrs serve --port 8080 --bind 127.0.0.1
```

```sh
curl -s localhost:8080/anyhow                                       # crate root
curl -s localhost:8080/anyhow/latest/anyhow/struct.Error.html       # item page
curl -s localhost:8080/tokio/latest/tokio/sync/struct.Mutex.html    # re-exported item
```

Response headers: `Content-Type: text/markdown; charset=utf-8`, `X-Markdown-Tokens` (byte-count/4 heuristic), `Vary: Accept`.

Status codes: 404 item not found, 400 bad spec, 502 upstream/decode error.

## Notes

- In-memory LRU cache (32 crates) per process. No disk cache.
- v0 does not render trait impls, blanket impls, or source links.
- Glob re-exports into external crates (e.g. `clap::Parser` from `clap_builder`) are not followed.

## WebAssembly builds

Two same-ABI WASM modules live alongside the Rust library:

- [`rust-wasm/`](rust-wasm/README.md) — `wasm32-unknown-unknown` build of
  the pure pipeline (spec parse + resolve + render). Exports `alloc`,
  `free`, `resolve_url`, and optionally `render_markdown`.
- [`zig/`](zig/README.md) — Zig 0.16 port of the same surface (`resolve_url`
  parity today; `render_markdown` is a follow-up). Ships a Cloudflare Worker
  wrapper that can load either artifact unchanged.

Build the Rust wasm:

```sh
# Minimal (resolve_url only — matches current Zig surface).
cargo build --profile wasm-release --target wasm32-unknown-unknown \
  -p md-docrs-wasm --no-default-features
# Full (adds render_markdown, brings in serde_json + rustdoc-types).
cargo build --profile wasm-release --target wasm32-unknown-unknown \
  -p md-docrs-wasm
```

The root crate's HTTP / server / CLI bits are gated behind `http`, `server`,
and `cli` features (all on by default), so the pure pipeline compiles for
`wasm32` without reqwest/tokio/axum/zstd.

To compare the two modules side by side (size, output parity, per-call
latency) under an embedded wasmtime or wasmer, see
[`wasm/`](wasm/README.md):

```sh
./wasm/build.sh                          # builds zig + rust wasm, stages them
cargo run -p md-docrs-wasm-compare       # runs the table
```

## Logging

```sh
RUST_LOG=md_docrs_proxy=debug md-docrs serve
```
