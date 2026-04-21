# wasm/ — workspace-level WASM harness

This directory is **not** a Rust crate.

It exists to keep the cross-language WASM comparison workflow in one simple place:

- `build.sh` builds and stages WASM artifacts from the Rust and Zig implementations
- `artifacts/` holds the staged `.wasm` files
- this `README.md` explains how to run the comparison harness

The actual Rust comparison binary lives in:

- `crates/md-docrs-wasm-compare`

## Boundaries

Keep the repo split like this:

- `crates/md-docrs-core` — shared Rust library logic
- `crates/md-docrs-rust-wasm` — Rust WASM module
- `crates/md-docrs-wasm-compare` — Rust host-side comparison harness
- `zig/` — Zig implementation and its Worker wrapper
- `wasm/` — staging area and glue docs/scripts only

That separation keeps responsibilities lean:

- Zig owns the Zig implementation
- Rust owns the Rust implementation and host harness
- `wasm/` owns only the artifact workflow

## Layout

```/dev/null/layout.txt#L1-11
wasm/
├── README.md               # this file
├── build.sh                # stages Rust + Zig wasm outputs into artifacts/
└── artifacts/              # .gitignored staged outputs
    ├── zig-minimal.wasm
    ├── zig-full.wasm       # optional, only if Zig full build exists
    ├── rust-minimal.wasm
    ├── rust-minimal-opt.wasm
    ├── rust-full.wasm
    └── rust-full-opt.wasm
```

Related workspace locations:

```/dev/null/workspace-layout.txt#L1-8
crates/
├── md-docrs-rust-wasm/
├── md-docrs-wasm-compare/
└── ...
zig/
└── ...
wasm/
└── ...
```

## What gets compared

The harness compares compatible WASM artifacts that share the same low-level ABI.

Today that means:

- **Zig minimal**
  - exports `alloc`, `free`, `resolve_url`
  - implements spec parsing + docs.rs URL resolution
- **Rust minimal**
  - exports the same minimal ABI
  - meant to match the Zig surface
- **Rust full**
  - extends the surface with rendering functionality
- **Zig full**
  - optional future/experimental target if implemented

The comparison harness reports:

- artifact size
- output parity for `resolve_url`
- median and p95 latency
- raw Rust size vs `wasm-opt -Oz` size

## Quick start

From the repo root:

```/dev/null/quickstart.sh#L1-4
./wasm/build.sh
cargo run -p md-docrs-wasm-compare
```

That does two things:

1. builds/stages available `.wasm` artifacts into `wasm/artifacts/`
2. runs the host-side comparison binary from `crates/md-docrs-wasm-compare`

## What `build.sh` does

`wasm/build.sh` is the single entry point for artifact staging.

It is responsible for:

- building Zig minimal
- attempting Zig full, but skipping it cleanly if unsupported
- building Rust minimal from `crates/md-docrs-rust-wasm`
- building Rust full from `crates/md-docrs-rust-wasm`
- producing optimized Rust copies with `wasm-opt`
- copying all generated outputs into `wasm/artifacts/`

It should not contain harness logic.
It should not become a second build system.
Its job is only to stage comparable artifacts in one place.

## Required tools

You need these available on your machine:

- Rust toolchain with `wasm32-unknown-unknown`
- Zig
- `wasm-opt` from Binaryen

If `wasm-opt` is missing, `build.sh` should fail early because optimized Rust artifacts are part of the comparison output.

## Artifact names

The harness looks for these filenames in `wasm/artifacts/`:

- `zig-minimal.wasm`
- `zig-full.wasm`
- `rust-minimal.wasm`
- `rust-minimal-opt.wasm`
- `rust-full.wasm`
- `rust-full-opt.wasm`

Any subset may be present.
Missing files are skipped.

That makes the flow flexible:

- minimal-only comparison works
- Rust-only comparison works
- future Zig full comparison can slot in without redesign

## Rust commands

The Rust WASM module comes from `crates/md-docrs-rust-wasm`.

Minimal build:

```/dev/null/rust-minimal.sh#L1-3
cargo build --profile wasm-release --target wasm32-unknown-unknown \
  -p md-docrs-rust-wasm --no-default-features
```

Full build:

```/dev/null/rust-full.sh#L1-3
cargo build --profile wasm-release --target wasm32-unknown-unknown \
  -p md-docrs-rust-wasm --no-default-features --features full
```

Comparison harness:

```/dev/null/harness.sh#L1-2
cargo run -p md-docrs-wasm-compare
```

Optional Wasmer runtime:

```/dev/null/harness-wasmer.sh#L1-2
cargo run -p md-docrs-wasm-compare --features wasmer -- --runtime wasmer
```

## Zig commands

The Zig implementation lives under `zig/`.

Minimal WASM build:

```/dev/null/zig-build.sh#L1-3
cd zig/lib
zig build
```

Native Zig tests:

```/dev/null/zig-test.sh#L1-3
zig build test --build-file zig/lib/build.zig
```

If Zig full is not implemented yet, `build.sh` should print a skip message and continue.

## Flags

The harness supports these main flags:

| Flag | Default | Meaning |
| --- | --- | --- |
| `--runtime wasmtime\|wasmer` | `wasmtime` | Embedded runtime used by the Rust host harness |
| `--iterations N` | `200` | Hot-loop samples per artifact/spec pair |
| `--artifacts-dir PATH` | `wasm/artifacts` | Directory containing staged `.wasm` files |

If supported by the harness version you are running, other flags such as offline or render-specific controls follow the same rule: they belong to the host harness crate, not to `wasm/build.sh`.

## Running raw modules manually

The `.wasm` files can be inspected directly, but real calls require host code that:

- allocates memory in the module
- writes input bytes into WASM memory
- calls exported functions
- reads the output bytes
- frees buffers correctly

That host logic lives in the Rust comparison harness, not in this directory.

## Design rule for this directory

Keep `wasm/` boring.

Good uses:

- stage artifacts
- document the comparison workflow
- hold generated outputs

Bad uses:

- adding a second Rust crate here
- duplicating logic from `crates/md-docrs-wasm-compare`
- mixing Zig source code into this directory
- mixing Rust library code into this directory

## Summary

If you are looking for:

- the Rust WASM implementation: see `crates/md-docrs-rust-wasm`
- the Rust host comparison program: see `crates/md-docrs-wasm-compare`
- the Zig implementation: see `zig/`
- the staged outputs and helper script: stay in `wasm/`

The goal is simple: one place to stage artifacts, one Rust crate to compare them, and clear boundaries between Rust, Zig, and the shared WASM workflow.