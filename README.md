# md-docrs-proxy

Proxy that downloads rustdoc JSON from docs.rs and renders it as Markdown - built for LLM agents that waste tokens scraping docs.rs HTML.

## Build

```sh
cargo build --release
# binary at ./target/release/md-docrs
```

## Release packaging

This repository is configured for [`cargo-dist`](https://axodotdev.github.io/cargo-dist/)
releases of the `md-docrs` CLI. With `cargo-dist` 0.31, the generated release
configuration lives in [`dist-workspace.toml`](dist-workspace.toml) and the CI
workflow lives in [`.github/workflows/release.yml`](.github/workflows/release.yml).

Install `dist` locally:

```sh
cargo install cargo-dist --locked
cargo install cargo-release --locked
```

Validate what the release workflow will build:

```sh
dist plan --tag vX.Y.Z
```

Build the current platform's release artifacts and installers locally:

```sh
dist build --tag vX.Y.Z
```

Release tags should use the unified workspace form (`v0.1.0`, `v0.2.3`, ...).
This workspace explicitly distributes only the root `md_docrs_proxy` package,
which ships the `md-docrs` binary.

Homebrew releases are published through the `ThomAub/homebrew-tap` tap managed
by `cargo-dist`. There is intentionally no checked-in `HomebrewFormula/`
directory in this repository.

Before the first tagged release:

```sh
# preview the release mechanics without side effects
cargo release 0.1.0

# after the tree is clean, create the release commit/tag locally
cargo release 0.1.0 --execute --no-publish --no-push

# trigger the GitHub release workflow
git push origin HEAD
git push origin v0.1.0
```

This repository configures `cargo-release` to:

- update the shared Rust workspace version in one place
- tag releases as `v{{version}}`
- skip remote pushes by default

For this non-virtual workspace, use `--workspace` so `cargo-release` updates the
shared version for all Rust workspace members while only publishing the root
crate that is actually publishable:

```sh
# preview
cargo release 0.2.0 --workspace

# release commit + tag + crates.io publish, but keep the remote push explicit
cargo release 0.2.0 --workspace --execute --no-push
```

That keeps the final push explicit while still letting `cargo-release` handle
the version bump, release commit, tag creation, and crates.io publish.

Repository prerequisites:

- Create the tap repository `ThomAub/homebrew-tap`.
- Add a GitHub personal access token with `repo` scope as the
  `HOMEBREW_TAP_TOKEN` secret in this repository.

Once a release has been published, install with Homebrew:

```sh
brew install ThomAub/tap/md-docrs
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
# Optional shipped-size pass for Rust artifacts.
wasm-opt -Oz --strip-debug --strip-dwarf \
  -o wasm/artifacts/rust-minimal-opt.wasm \
  target/wasm32-unknown-unknown/wasm-release/md_docrs_wasm.wasm
```

The root crate's HTTP / server / CLI bits are gated behind `http`, `server`,
and `cli` features (all on by default), so the pure pipeline compiles for
`wasm32` without reqwest/tokio/axum/zstd.

To compare the two modules side by side (size, output parity, per-call
latency) under an embedded wasmtime or wasmer, see
[`wasm/`](wasm/README.md):

```sh
./wasm/build.sh                          # builds zig + rust wasm, runs wasm-opt, stages them
cargo run -p md-docrs-wasm-compare       # runs the table
```

## Logging

```sh
RUST_LOG=md_docrs_proxy=debug md-docrs serve
```
