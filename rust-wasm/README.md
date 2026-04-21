# md-docrs-wasm

`wasm32-unknown-unknown` build of the `md_docrs_proxy` pure pipeline, exposing
the **exact same C ABI** as the Zig build (`zig/lib/wasm.zig`). Lets us drop
either `.wasm` into the same host and compare size and per-request latency
without any host-side code changes.

## Exports

| Symbol | Signature | Notes |
| --- | --- | --- |
| `alloc` | `(len: u32) -> *u8` | Backed by Rust's global allocator. Returns null on OOM or `len == 0`. |
| `free` | `(ptr: *u8, len: u32)` | Length must match the allocation. |
| `resolve_url` | `(spec_ptr, spec_len, target_ptr, target_len, out_ptr, out_cap) -> u32` | Same semantics as the Zig export. 0 on error. |
| `render_markdown` | `(json_ptr, json_len, spec_ptr, spec_len, target_ptr, target_len, len_out: *u32) -> *u8` | Takes already-decoded rustdoc JSON, returns a fresh allocation containing Markdown. Caller frees. Null on error. Only present in the `render` feature build. |

## Building

```sh
# Minimal parity build — matches the Zig wasm surface (resolve_url only).
cargo build --profile wasm-release --target wasm32-unknown-unknown \
  -p md-docrs-wasm --no-default-features
wasm-opt -Oz --strip-debug --strip-dwarf \
  -o target/wasm32-unknown-unknown/wasm-release/md_docrs_wasm.opt.wasm \
  target/wasm32-unknown-unknown/wasm-release/md_docrs_wasm.wasm

# Full pipeline — adds render_markdown (serde_json + rustdoc-types).
cargo build --profile wasm-release --target wasm32-unknown-unknown \
  -p md-docrs-wasm
wasm-opt -Oz --strip-debug --strip-dwarf \
  -o target/wasm32-unknown-unknown/wasm-release/md_docrs_wasm.opt.wasm \
  target/wasm32-unknown-unknown/wasm-release/md_docrs_wasm.wasm
```

Raw artifact lives at `target/wasm32-unknown-unknown/wasm-release/md_docrs_wasm.wasm`.

If you run `wasm-opt`, the optimized artifact can live alongside it, e.g.
`target/wasm32-unknown-unknown/wasm-release/md_docrs_wasm.opt.wasm`.

## Size snapshot

Measured on Rust 1.94 / Zig 0.16.

| Build | Bytes |
| --- | ---: |
| Zig 0.16 — `ReleaseSmall` + `strip`, exports `resolve_url` | **6,775** |
| Rust `wasm-release` — `resolve_url` only (`--no-default-features`) | **36,336** |
| Rust `wasm-release` + `wasm-opt -Oz` — `resolve_url` only | **28,523** |
| Rust `wasm-release` — `resolve_url` + `render_markdown` | **486,387** |

For the `resolve_url`-only Rust build, `wasm-opt -Oz` trims about **7,813 bytes**
from the raw `wasm-release` artifact, roughly a **21.5%** reduction.

The large jump for `render_markdown` is serde_json + `rustdoc-types`
deserialise impls. Expected; that's the cost of JSON→AST→Markdown.

## Feature gates

- `render` (default) — pulls `serde_json` + `rustdoc-types` and exposes
  `render_markdown`. Turn off for the minimal size-parity build.

## Tests

Host tests run through the same internal functions as the WASM exports
(the `no_mangle` attribute is gated to `target_arch = "wasm32"` so the test
binary doesn't shadow libc's `free`):

```sh
cargo test -p md-docrs-wasm
```

## Comparing with Zig

Both modules share this memory protocol:

1. Host calls `alloc(n)` to reserve input / output buffers in linear memory.
2. Host writes input bytes into those buffers via a fresh `Uint8Array(memory.buffer, ptr, len)`.
3. Host calls `resolve_url(...)` (or `render_markdown(...)`).
4. Host reads the output, then calls `free(ptr, len)` on each buffer.

Because the ABI matches byte-for-byte, the Worker at `zig/src/index.ts`
works as-is against either module — just point the `.wasm` import at the
Rust artifact.

## What's next

- Port `render_markdown` to Zig. That's where the real interesting size /
  speed comparison happens — today the Zig wasm doesn't carry serde_json
  or the rustdoc types.
- Benchmark instantiation + per-call latency side-by-side in a Worker
  (e.g. hyperfine-style loop from a test harness, or wrangler dev + `wrk`).
- Keep comparing raw vs `wasm-opt -Oz` output as the Rust WASM surface grows,
  especially once Zig gains the full render pipeline too.

Option A: keep `std`, but drastically reduce code size
This is the lowest-risk path.

For the minimal build:
- stop using `ItemSpec::parse`
- stop using `String`
- stop using `format!`
- implement a tiny local parser over `&[u8]`
- write URL bytes directly to `out_ptr`

This alone could cut a lot.

### Option B: create a dedicated `no_std` tiny crate
Example direction:
- `rust-wasm-tiny/`
- exports only `resolve_url`
- parser implemented over raw bytes
- no `std`
- no `serde`
- no `rustdoc-types`
- no dependency on main crate

This is the path most likely to get you materially closer to Zig.
