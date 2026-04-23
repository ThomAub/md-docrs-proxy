# md-docrs-zig

Minimal Zig implementation of docs.rs rustdoc JSON URL resolution.

This subtree does three things:

- parses `crate[@version][::path::to::item]`
- builds the matching docs.rs rustdoc JSON URL
- exposes that logic as:
  - a native CLI
  - a small WASM module
  - a Cloudflare Worker wrapper

It does not fetch rustdoc JSON, decode zstd, or render Markdown. Those live on the Rust side.

## Scope

`zig/` is the minimal comparison target for the Rust WASM build.

It owns:

- spec parsing
- docs.rs URL construction
- `resolve_url` WASM export
- native Zig CLI
- Worker host wrapper

It does not own:

- HTTP fetching
- caching
- zstd decoding
- rustdoc JSON parsing
- Markdown rendering
- the main native server

## Layout

- `lib/build.zig` — Zig build definitions
- `lib/cli.zig` — native CLI
- `lib/resolve.zig` — shared resolver logic
- `lib/spec.zig` — spec parser
- `lib/url.zig` — docs.rs URL builder
- `lib/wasm.zig` — minimal WASM ABI
- `src/index.ts` — Cloudflare Worker wrapper
- `src/md_docrs.wasm` — staged WASM artifact used by the Worker

## Build

From `zig/`:

```bash
npm install
npm run build:wasm
```

From `zig/lib/`:

```bash
zig build
zig build cli
zig build test
```

From the repo root:

```bash
zig build test --build-file zig/lib/build.zig
```

## Native CLI

Build:

```bash
cd zig/lib
zig build cli
```

Run:

```bash
./zig-out/bin/md-docrs-zig serde
./zig-out/bin/md-docrs-zig 'tokio@1.52.1::sync::Mutex'
./zig-out/bin/md-docrs-zig 'anyhow::Error' --target x86_64-unknown-linux-gnu
zig build run -- 'tokio@1.52.1::sync::Mutex' --target x86_64-unknown-linux-gnu
```

Usage:

```text
md-docrs-zig &lt;SPEC&gt; [--target TRIPLE]
```

Spec grammar:

```text
crate[@version][::path::to::item]
```

Behavior:

- prints the resolved docs.rs rustdoc JSON URL to stdout
- exits `0` on success
- exits `2` for invalid input, missing `--target` value, or unexpected arguments

Examples of output:

```text
https://docs.rs/crate/serde/latest/json/57.zst
https://docs.rs/crate/tokio/1.52.1/json/57.zst
https://docs.rs/crate/anyhow/latest/x86_64-unknown-linux-gnu/json/57.zst
```

## Worker

The Worker is a thin host around the Zig WASM module.

Setup and run:

```bash
cd zig
npm install
npm run build:wasm
npm run dev
```

Deploy:

```bash
npm run deploy
```

Accepted request forms:

```/dev/null/zig-worker-routes.txt#L1-4
GET /<spec>
GET /<spec>?target=<triple>
GET /?spec=<spec>
GET /?spec=<spec>&target=<triple>
```

Examples:

```bash
curl localhost:8787/serde
curl localhost:8787/tokio@1.52.1::sync::Mutex
curl 'localhost:8787/tokio::sync::Mutex?target=x86_64-unknown-linux-gnu'
curl 'localhost:8787/?spec=anyhow::Error'
```

Responses:

- success: plain text docs.rs URL plus trailing newline
- failure: `400` with plain text error
- empty spec: `400` with a short usage message

## WASM ABI

The module exports a small C-style ABI:

| Export | Signature | Notes |
| --- | --- | --- |
| `alloc` | `(len: u32) -> *u8` | Allocates linear memory. Returns `0` on failure. |
| `free` | `(ptr: *u8, len: u32)` | Frees memory allocated by `alloc`. |
| `resolve_url` | `(spec_ptr, spec_len, target_ptr, target_len, out_ptr, out_cap) -> u32` | Writes the resolved URL into caller-provided memory. Returns bytes written, or `0` on error. |

Contract:

- `target_len == 0` means no explicit target
- caller owns input and output buffers
- output buffer must be large enough for the full URL
- return value `0` means invalid spec or insufficient output capacity

The Worker currently uses a fixed output buffer of `512` bytes.

## Relationship to Rust

This Zig module matches the minimal ABI surface of `crates/md-docrs-rust-wasm`:

- same exported function names
- same memory ownership model
- same `resolve_url` contract

That lets the comparison harness swap Rust and Zig artifacts with the same host-side calling convention.

Use the repo-level comparison flow from the repository root:

```bash
./wasm/build.sh
cargo run -p md-docrs-wasm-compare -- --offline
```

## Notes

- current format version is `57`
- default docs.rs base is `https://docs.rs`
- default `zig build` produces the WASM artifact
- `zig build cli` builds the native CLI separately
- this subtree is intentionally narrow so size and latency comparisons stay meaningful