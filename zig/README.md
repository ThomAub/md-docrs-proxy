# md-docrs-zig

Zig implementation of the **minimal URL-resolution surface** of this repository.

This subtree is intentionally small and separate from the Rust workspace. Its job is to answer:

- can Zig produce a smaller `.wasm` for the same ABI?
- can Zig match Rust's `resolve_url` behavior exactly?
- can the same host code load either module unchanged?

It is **not** the full docs.rs proxy. It does not fetch rustdoc JSON, decompress zstd, or render Markdown.

## Boundaries

### What lives here

`zig/` owns the minimal, self-contained path:

- parse `crate[@version][::path]`
- build the corresponding docs.rs rustdoc JSON URL
- expose that logic through:
  - a Zig native CLI
  - a tiny WASM module
  - a Cloudflare Worker wrapper in TypeScript

### What does not live here

The following stay on the Rust side:

- HTTP fetching
- caching
- zstd decoding
- rustdoc JSON parsing
- Markdown rendering
- the main CLI/server application

That split is deliberate. It keeps the Zig implementation lean and makes size/perf comparisons fair.

## Relationship to the Rust workspace

The repository has three distinct layers:

1. `crates/md-docrs-core`
   - shared Rust logic for the full pipeline

2. `crates/md-docrs-rust-wasm`
   - Rust WASM module with the same low-level ABI as the Zig WASM module
   - can be built in:
     - minimal mode: `resolve_url` only
     - fuller mode: adds render support

3. `zig/`
   - independent Zig implementation of the minimal ABI surface

At the top level, `wasm/` is just a harness area:

- `wasm/build.sh` stages Zig and Rust artifacts into `wasm/artifacts/`
- `crates/md-docrs-wasm-compare` loads those artifacts and compares size, parity, and latency

So the conceptual split is:

- **Rust workspace** = production pipeline and Rust WASM
- **Zig subtree** = minimal alternative implementation
- **wasm/** = comparison/staging glue

## Layout

```/dev/null/zig-layout.txt#L1-17
zig/
├── lib/
│   ├── build.zig
│   ├── build.zig.zon
│   ├── spec.zig
│   ├── url.zig
│   ├── resolve.zig
│   ├── wasm.zig
│   └── cli.zig
├── src/
│   ├── index.ts
│   ├── md_docrs.wasm.d.ts
│   └── md_docrs.wasm
├── package.json
├── tsconfig.json
└── wrangler.jsonc
```

## Components

### `lib/spec.zig`
Parses the spec grammar:

- `crate`
- `crate@version`
- `crate::path::to::item`
- `crate@version::path::to::item`

### `lib/url.zig`
Builds the docs.rs JSON URL from parsed pieces.

### `lib/resolve.zig`
Pure glue between parsing and URL building. This is the logic shared by the CLI and WASM entrypoints.

### `lib/wasm.zig`
Exports the minimal ABI used for host-neutral comparisons.

### `lib/cli.zig`
Wraps the same core resolver as a native command-line tool.

### `src/index.ts`
Cloudflare Worker host for the WASM module. This is host glue only; the actual URL resolution lives in Zig WASM.

## Build

Most Zig work happens from `zig/lib/`.

```/dev/null/zig-build.sh#L1-11
cd zig/lib

# Build the WASM artifact.
zig build

# Build the native CLI.
zig build cli

# Run unit tests.
zig build test
```

If you want to run the test step from the repository root, point Zig at the build file explicitly:

```/dev/null/zig-build-root.sh#L1-1
zig build test --build-file zig/lib/build.zig
```

## Native CLI

The CLI is the fastest way to sanity-check the minimal resolver behavior.

```/dev/null/zig-cli.sh#L1-13
cd zig/lib
zig build cli

./zig-out/bin/md-docrs-zig serde
./zig-out/bin/md-docrs-zig 'tokio@1.52.1::sync::Mutex'
./zig-out/bin/md-docrs-zig 'anyhow::Error' --target x86_64-unknown-linux-gnu

# Or via the build runner:
zig build run -- 'tokio@1.52.1::sync::Mutex' --target x86_64-unknown-linux-gnu
```

Expected output is always a fully resolved docs.rs rustdoc JSON URL, for example:

```/dev/null/zig-cli-output.txt#L1-3
https://docs.rs/crate/serde/latest/json/57.zst
https://docs.rs/crate/tokio/1.52.1/json/57.zst
https://docs.rs/crate/anyhow/latest/x86_64-unknown-linux-gnu/json/57.zst
```

Exit codes:

| Code | Meaning |
| --- | --- |
| 0 | URL printed to stdout |
| 2 | Invalid spec, missing `--target` value, or unknown argument |

## Worker

The Worker is a thin host around the Zig WASM module.

```/dev/null/zig-worker.sh#L1-6
cd zig
npm install
npm run build:wasm
npm run dev
npm run deploy
```

Example requests:

```/dev/null/zig-worker-curl.sh#L1-4
curl localhost:8787/serde
curl localhost:8787/tokio@1.52.1::sync::Mutex
curl 'localhost:8787/tokio::sync::Mutex?target=x86_64-unknown-linux-gnu'
curl 'localhost:8787/?spec=anyhow::Error'
```

Each returns a resolved docs.rs URL string.

## WASM ABI

The Zig module exports a deliberately tiny ABI:

| Export | Signature | Notes |
| --- | --- | --- |
| `alloc` | `(len: u32) -> *u8` | Allocates in linear memory. Returns `0` on failure. |
| `free` | `(ptr: *u8, len: u32)` | Caller must free with the same length used for allocation. |
| `resolve_url` | `(spec_ptr, spec_len, target_ptr, target_len, out_ptr, out_cap) -> u32` | Writes the resolved URL into caller-provided output memory. Returns bytes written, or `0` on error. |

This ABI is intentionally matched by the Rust WASM crate so the same host can swap implementations without changing its calling convention.

## Integration with Rust WASM

The Rust equivalent is `crates/md-docrs-rust-wasm`.

Both modules are meant to be interchangeable for the minimal path:

- same exported function names
- same memory ownership model
- same `resolve_url` contract
- same expected output bytes for the same input

Build the Rust minimal module like this:

```/dev/null/rust-wasm-build.sh#L1-3
cargo build --profile wasm-release --target wasm32-unknown-unknown \
  -p md-docrs-rust-wasm --no-default-features
```

You can then compare the Zig and Rust artifacts through the top-level harness:

```/dev/null/wasm-compare.sh#L1-2
./wasm/build.sh
cargo run -p md-docrs-wasm-compare -- --offline
```

## Why this split exists

This subtree is intentionally narrow for two reasons:

1. **clear ownership**
   - Zig owns only the minimal resolver path
   - Rust owns the full product pipeline

2. **fair comparison**
   - if both Zig and Rust expose only `resolve_url`, size and latency comparisons mean something
   - if one side includes fetch/decompress/render and the other does not, the comparison becomes noisy

## Current status

Today, Zig covers:

- spec parsing
- URL resolution
- native CLI
- minimal WASM export
- Worker hosting

It does **not** yet cover:

- JSON-to-Markdown rendering
- in-WASM fetching
- zstd decompression

That is intentional. The minimal boundary is the stable comparison target.

## Summary

If you're deciding where code should go:

- put **full docs.rs proxy behavior** in Rust workspace crates
- put **minimal ABI-compatible URL resolution** in `zig/`
- put **artifact staging and cross-runtime comparison** in top-level `wasm/`

That keeps the repository lean and the boundaries clear.