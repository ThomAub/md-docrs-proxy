# md-docrs-rust-wasm

Rust `wasm32-unknown-unknown` export layer for this workspace.

This crate wraps the shared Rust logic from `md-docrs-core` behind a small C-style ABI for host environments.

## What it exports

### Always available

- `alloc(len: u32) -> *mut u8`
- `free(ptr: *mut u8, len: u32)`
- `resolve_url(spec_ptr, spec_len, target_ptr, target_len, out_ptr, out_cap) -> u32`

These three exports match the Zig minimal WASM surface.

### With `render`

- `render_markdown(json_ptr, json_len, spec_ptr, spec_len, target_ptr, target_len, len_out) -> *mut u8`

This lets a host pass rustdoc JSON into the module and receive rendered Markdown back.

### With `render` + `fetch`

- `render_spec(spec_ptr, spec_len, target_ptr, target_len, buf_ptr_out, buf_len_out) -> i32`

This is the full in-module pipeline:

1. parse the spec
2. build the docs.rs rustdoc JSON URL
3. call the host-provided `fetch_bytes`
4. zstd-decode the response
5. parse rustdoc JSON
6. resolve the requested item
7. render Markdown

## Features

| Feature | Default | Purpose |
| --- | --- | --- |
| `render` | yes | Enables JSON-to-Markdown rendering and exports `render_markdown` |
| `fetch` | no | Enables host-imported fetch + in-WASM zstd decode used by `render_spec` |
| `full` | no | Convenience alias for `render` + `fetch` |

## Build modes

### Minimal

Exports only:

- `alloc`
- `free`
- `resolve_url`

Build:

```/dev/null/minimal.sh#L1-2
cargo build --profile wasm-release --target wasm32-unknown-unknown \
  -p md-docrs-rust-wasm --no-default-features
```

### Default

Exports:

- `alloc`
- `free`
- `resolve_url`
- `render_markdown`

Build:

```/dev/null/default.sh#L1-2
cargo build --profile wasm-release --target wasm32-unknown-unknown \
  -p md-docrs-rust-wasm
```

### Full

Exports:

- `alloc`
- `free`
- `resolve_url`
- `render_markdown`
- `render_spec`

Build:

```/dev/null/full.sh#L1-2
cargo build --profile wasm-release --target wasm32-unknown-unknown \
  -p md-docrs-rust-wasm --no-default-features --features full
```

Output path:

```/dev/null/output.txt#L1-1
target/wasm32-unknown-unknown/wasm-release/md_docrs_rust_wasm.wasm
```

## ABI notes

### Memory

- `alloc` returns a pointer in WASM linear memory
- `free` must be called with the exact pointer and length originally allocated
- `alloc(0)` returns null
- most failures are reported as `0`, null, or a negative status code depending on the export

### `resolve_url`

`resolve_url` parses:

```/dev/null/spec.txt#L1-1
crate[@version][::path::to::item]
```

If `target_len == 0`, no explicit target triple is used.

On success it writes the docs.rs rustdoc JSON URL into the caller-provided output buffer and returns the number of bytes written.

It returns `0` on failure, including:

- invalid UTF-8
- invalid spec
- output buffer too small

### `render_markdown`

`render_markdown` expects the host to provide:

- rustdoc JSON bytes
- a spec
- an optional target triple
- a writable `len_out`

On success it returns a newly allocated Markdown buffer and writes its size to `*len_out`.

The caller owns the returned buffer and must release it with `free(ptr, len)`.

It returns null on failure.

### `render_spec`

`render_spec` requires a host import:

```/dev/null/fetch-bytes.txt#L1-5
fetch_bytes(
  url_ptr: *const u8,
  url_len: u32,
  buf_ptr_out: *mut u32,
  buf_len_out: *mut u32,
) -> i32
```

The host is expected to:

1. fetch the URL
2. allocate a buffer inside WASM memory using exported `alloc`
3. write the response body into that buffer
4. store the pointer and length into the provided out-slots

Return `0` for success and non-zero for failure.

On success, `render_spec` writes an allocated Markdown buffer to `*buf_ptr_out` and `*buf_len_out`, then returns `0`.

### `render_spec` status codes

| Code | Meaning |
| --- | --- |
| `0` | Success |
| `-1` | Allocation failure |
| `-2` | Host fetch failed |
| `-3` | zstd decode failed |
| `-4` | JSON parse failed |
| `-5` | Spec parse failure, resolve miss, or URL too long |
| `-6` | Output pointer or length could not be written |

## Relationship to the rest of the repo

- `crates/md-docrs-core` contains the shared Rust parsing, resolution, and rendering logic
- `crates/md-docrs-wasm-compare` contains the host-side comparison harness
- `zig/` contains the independent Zig implementation of the minimal ABI
- `wasm/` contains the repo-level staging script and staged artifacts

This crate should stay focused on the Rust WASM ABI layer.

## Typical workflow

Build and stage artifacts from the repo root:

```/dev/null/workflow.sh#L1-2
./wasm/build.sh
cargo run -p md-docrs-wasm-compare -- --offline
```

## Tests

```/dev/null/tests.sh#L1-1
cargo test -p md-docrs-rust-wasm
```
