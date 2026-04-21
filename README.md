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

```sh
cargo build --workspace
```

Run the Rust tests:

```sh
cargo test --workspace
```

Run the Zig tests from the repo root:

```sh
zig build test --build-file zig/lib/build.zig
```

## Release packaging

This repository is configured for [`cargo-dist`](https://axodotdev.github.io/cargo-dist/)
releases of the `md-docrs` CLI. With `cargo-dist` 0.31, the generated release
configuration lives in [dist-workspace.toml](/Users/thomas/dev/perso/github/md_docrs_proxy/dist-workspace.toml)
and the CI workflow lives in [.github/workflows/release.yml](/Users/thomas/dev/perso/github/md_docrs_proxy/.github/workflows/release.yml).

Install the release tooling locally:

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
This workspace distributes only the `md-docrs-cli` package with `cargo-dist`,
which ships the `md-docrs` binary.

Homebrew releases are published through the `ThomAub/homebrew-tap` tap managed
by `cargo-dist`. There is intentionally no checked-in `HomebrewFormula/`
directory in this repository.

This repository configures `cargo-release` to:

- update the shared Rust workspace version in one place
- create a single unified release tag as `v{{version}}`
- publish the crates needed by the CLI release
- keep the final remote push explicit

For this virtual workspace, preview and execute releases from the repository root:

```sh
# preview
cargo release 0.2.0

# version bump + release commit + tag + crates.io publish, but keep the push explicit
cargo release 0.2.0 --execute --no-push

# push the release commit and tag after validation
git push origin main
git push origin v0.2.0
```

Repository prerequisites:

- Create the tap repository `ThomAub/homebrew-tap`.
- Add a GitHub personal access token with `repo` scope as the
  `HOMEBREW_TAP_TOKEN` secret in this repository.

Once a release has been published, install with Homebrew:

```sh
brew install ThomAub/tap/md-docrs
```

## Native CLI

The CLI binary comes from `md-docrs-cli`.

Spec grammar:

```text
crate[@version][::path::to::item]
```

Examples:

```sh
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

```sh
cargo run -p md-docrs-server -- --port 8080 --bind 127.0.0.1
```

Example requests:

```sh
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

```sh
cargo build --profile wasm-release --target wasm32-unknown-unknown \
  -p md-docrs-rust-wasm --no-default-features
```

Default build adds `render_markdown`:

```sh
cargo build --profile wasm-release --target wasm32-unknown-unknown \
  -p md-docrs-rust-wasm
```

Full build adds `render_markdown` and `render_spec`:

```sh
cargo build --profile wasm-release --target wasm32-unknown-unknown \
  -p md-docrs-rust-wasm --no-default-features --features full
```

## Zig

The Zig subtree implements the minimal `resolve_url` path.

Common commands:

```sh
zig build --build-file zig/lib/build.zig
zig build cli --build-file zig/lib/build.zig
zig build test --build-file zig/lib/build.zig
```

See `zig/README.md` for details.

## WASM comparison

Stage artifacts, then run the comparison harness:

```sh
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
