# wasm/ — side-by-side comparison harness

Runs the Zig and Rust wasm builds of `resolve_url` through the exact same
sequence of specs and reports:

- artifact size
- resolved URL (parity check — every artifact must produce byte-identical output)
- median and p95 per-call latency

Default runtime is embedded **wasmtime** (crate). The `wasmer` cargo feature
swaps in the **wasmer** crate as an alternate host. Both are in-process
embeddings, not the `wasmtime` / `wasmer` CLI binaries.

## Layout

```
wasm/
├── Cargo.toml          # md-docrs-wasm-compare (workspace member)
├── src/main.rs         # harness: loads wasm, drives resolve_url, reports
├── build.sh            # builds zig + rust wasms and stages them in artifacts/
├── artifacts/          # .gitignored — populated by build.sh
│   ├── zig.wasm
│   ├── rust-minimal.wasm
│   └── rust-full.wasm
└── README.md
```

## Quick start

```sh
# From repo root.
./wasm/build.sh                          # produces artifacts/*.wasm
cargo run -p md-docrs-wasm-compare       # default: wasmtime, 200 iterations
```

Sample output:

```
artifact            bytes
-------------- ----------
zig                  6775
rust-minimal        36268
rust-full          404159

spec: tokio@1.52.1::sync::Mutex
artifact        output                                            median µs      p95 µs
--------------  ------------------------------------------------  ---------  ----------
zig             https://docs.rs/crate/tokio/1.52.1/json/57.zst            7           8
rust-minimal    https://docs.rs/crate/tokio/1.52.1/json/57.zst            9           9
rust-full       https://docs.rs/crate/tokio/1.52.1/json/57.zst            9           9
```

All three artifacts must return byte-identical URLs for every spec — that is
the ABI parity check. Per-call latency includes three `alloc`s, one
`resolve_url`, three `free`s, plus one `Memory::write` per input and one
`Memory::read` for the output.

## Flags

| Flag | Default | Meaning |
| --- | --- | --- |
| `--runtime wasmtime\|wasmer` | `wasmtime` | Embedded host. `wasmer` requires `--features wasmer`. |
| `--iterations N` | 200 | Hot-loop samples per (artifact, spec) cell. |
| `--artifacts-dir PATH` | `wasm/artifacts` | Where to look for `zig.wasm`, `rust-minimal.wasm`, `rust-full.wasm`. |

Any of the three `.wasm` files may be missing — the harness just skips that row.

## Wasmer (optional)

```sh
cargo run -p md-docrs-wasm-compare --features wasmer -- --runtime wasmer
```

Wasmer pulls in its own Cranelift fork; first build is ~20s. Both runtimes
agree on output, but wasmer's singlepass / cranelift defaults typically
give different per-call timings than wasmtime's cranelift — useful for
separating ABI cost from JIT cost.

## Running the raw `.wasm` without the harness

The CLI form of wasmtime / wasmer can't easily marshal strings across the
ABI boundary, but you can still inspect the modules:

```sh
wasmtime compile wasm/artifacts/zig.wasm -o /tmp/zig.cwasm
wasmer inspect wasm/artifacts/rust-minimal.wasm | head
```

For an end-to-end call you need host code that writes the spec into WASM
memory and reads the result back — that's exactly what `src/main.rs` does.

## Adding a new spec

Edit `DEFAULT_SPECS` in `src/main.rs`. A spec is `(spec_string, optional_target)`
and runs against every `.wasm` in the artifacts directory.
