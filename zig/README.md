# md-docrs-zig

Zig 0.16 port of the CLI-facing portion of `md-docrs-proxy`. Scope is deliberately narrow:

- Parse the `crate[@version][::path::to::item]` spec grammar (`src/spec.zig`).
- Build the docs.rs rustdoc JSON URL (`src/url.zig`).
- Drive both from a CLI (`src/main.zig`).

Network fetch, zstd decode, rustdoc-JSON deserialisation and Markdown rendering still live in the Rust crate at the repo root. Keeping the Zig binary this small lets us produce a WebAssembly module that is meaningful to compare against the Rust build.

## Build

```sh
cd zig
zig build                 # debug binary at zig-out/bin/md-docrs-zig
zig build -Doptimize=ReleaseSmall
zig build test
zig build run -- serde::de::Deserialize
```

## Example

```sh
$ ./zig-out/bin/md-docrs-zig tokio@1.52.1::sync::Mutex --target x86_64-unknown-linux-gnu
https://docs.rs/crate/tokio/1.52.1/x86_64-unknown-linux-gnu/json/57.zst
```

## WebAssembly

The WASM target will be wired up next. The intended invocation is:

```sh
zig build -Dtarget=wasm32-wasi -Doptimize=ReleaseSmall
```

At that point we'll add a matching `wasm32-wasip1` (or `wasm32-unknown-unknown`) target to the Rust crate and compare `.wasm` sizes for the same surface area (spec parsing + URL building).
