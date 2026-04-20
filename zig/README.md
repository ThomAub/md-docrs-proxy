# md-docrs-zig

Zig 0.16 port of the spec-parsing / URL-building portion of `md-docrs-proxy`, compiled two ways:

- **WASM** (`wasm32-freestanding`, `ReleaseSmall`) — runs on Cloudflare Workers via `src/index.ts`. Layout and memory protocol mirror [zigflare](https://github.com/mattzcarey/zigflare).
- **Native CLI** — same core `resolve.resolveUrl`, wrapped with argv handling in `lib/cli.zig`. Useful for local iteration and for A/B testing against the Rust binary.

Scope is intentionally narrow so the WASM artifact is directly comparable to a same-scope Rust WASM build: no HTTP, no zstd, no rustdoc-JSON parsing, no Markdown renderer — those stay in the root Rust crate.

## Layout

```
zig/
├── lib/                     # Zig sources (build runs here)
│   ├── build.zig
│   ├── build.zig.zon
│   ├── spec.zig             # pure: crate[@version][::path] grammar
│   ├── url.zig              # pure: docs.rs URL builder
│   ├── resolve.zig          # pure: spec + url glue, native tests
│   ├── wasm.zig             # WASM entry: alloc / free / resolve_url
│   └── cli.zig              # native CLI entry
├── src/                     # Cloudflare Worker (TypeScript)
│   ├── index.ts
│   ├── md_docrs.wasm.d.ts
│   └── md_docrs.wasm        # produced by `npm run build:wasm`
├── package.json
├── tsconfig.json
└── wrangler.jsonc
```

## Build

```sh
# WASM only (default target)
cd zig/lib && zig build
# -> zig-out/bin/md-docrs.wasm

# Native CLI
cd zig/lib && zig build cli
./zig-out/bin/md-docrs-zig serde::de::Deserialize

# Tests (native, pull in spec.zig + url.zig + resolve.zig tests)
cd zig/lib && zig build test

# Run CLI through the build system
cd zig/lib && zig build run -- tokio@1.52.1::sync::Mutex --target x86_64-unknown-linux-gnu
```

## Worker

```sh
cd zig
npm install
npm run build:wasm           # builds lib/ and copies the wasm into src/
npm run dev                  # wrangler dev on localhost
npm run deploy               # wrangler deploy
```

Endpoints:

```sh
curl localhost:8787/serde                                    # latest
curl localhost:8787/tokio@1.52.1::sync::Mutex
curl 'localhost:8787/tokio::sync::Mutex?target=x86_64-unknown-linux-gnu'
curl 'localhost:8787/?spec=anyhow::Error'
```

All three print the fully resolved `https://docs.rs/crate/<crate>/<version>[/<target>]/json/57.zst` URL.

## WASM ABI

Exported from `lib/wasm.zig`:

| Export | Signature | Notes |
| --- | --- | --- |
| `alloc` | `(len: u32) -> *u8` | Backed by `std.heap.wasm_allocator`. Returns 0 on OOM. |
| `free` | `(ptr: *u8, len: u32)` | Caller must pass the exact length passed to `alloc`. |
| `resolve_url` | `(spec_ptr, spec_len, target_ptr, target_len, out_ptr, out_cap) -> u32` | Returns bytes written, or 0 on bad spec / out-of-space. `target_len == 0` means "no target override". |

Memory protocol notes in the zigflare [`doc/memory.md`](https://github.com/mattzcarey/zigflare/blob/main/doc/memory.md) apply verbatim: always recreate `Uint8Array` views *after* each `alloc`, since WASM memory growth detaches existing views.

## Comparing with Rust WASM

The Rust equivalent lives at [`../rust-wasm/`](../rust-wasm/README.md). It exports
the same `alloc` / `free` / `resolve_url` symbols with byte-for-byte identical
signatures, so the Worker at `src/index.ts` can swap between the two by changing
a single import path.

```sh
# Minimal parity build — matches this Zig wasm surface 1:1.
cargo build --profile wasm-release --target wasm32-unknown-unknown \
  -p md-docrs-wasm --no-default-features
cp ../target/wasm32-unknown-unknown/wasm-release/md_docrs_wasm.wasm \
   src/md_docrs.wasm   # drop-in replacement for the Zig artifact

# Full pipeline build — also exports `render_markdown` (JSON → Markdown).
cargo build --profile wasm-release --target wasm32-unknown-unknown \
  -p md-docrs-wasm
```

What we're comparing:

- `.wasm` size (Zig `ReleaseSmall` + `strip` vs. Rust `opt-level=z` + fat LTO + `strip`).
- Instantiation + per-call latency in a Worker.
- Cold-start cost (wrangler measures this).

The Rust README has the current byte counts. Porting `render_markdown` to
Zig is the interesting follow-up — that's where serde_json / rustdoc-types
vs. `std.json` + hand-written types becomes a real apples-to-apples test.
