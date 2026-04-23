# md-docrs-wasm-compare

Host-side comparison harness for the staged Rust and Zig WASM artifacts.

This crate is used to compare behavior and artifact characteristics across the WASM modules built elsewhere in the workspace. It does not build the `.wasm` files itself; it consumes artifacts staged under the repo-level `wasm/` directory.

## What it does

- loads staged WASM artifacts from `wasm/artifacts/`
- runs the same host-side checks against each available artifact
- compares outputs across implementations
- supports offline runs against locally staged artifacts

This crate is intended for verification and comparison work, not for serving production traffic.

## Scope

This crate owns:

- the native comparison binary
- host-side loading and execution of staged WASM modules
- result comparison across Rust and Zig artifacts

This crate does not own:

- WASM artifact building
- artifact staging scripts
- the Rust WASM implementation itself
- the Zig implementation itself

## Binary

The package exposes the `wasm-compare` binary.

Run it from the repository root after staging artifacts:

```bash
./wasm/build.sh
cargo run -p md-docrs-wasm-compare -- --offline
```

## Inputs

The harness looks for staged files in `wasm/artifacts/`.

Expected filenames include:

- `zig-minimal.wasm`
- `zig-full.wasm`
- `rust-minimal.wasm`
- `rust-minimal-opt.wasm`
- `rust-full.wasm`
- `rust-full-opt.wasm`

Missing artifacts are skipped.

## Typical workflow

From the repository root:

```bash
./wasm/build.sh
cargo run -p md-docrs-wasm-compare -- --offline
```

## Related paths

- `wasm/README.md` — repo-level staging workflow
- `wasm/build.sh` — artifact build and staging script
- `crates/md-docrs-rust-wasm` — Rust WASM module under test
- `zig/README.md` — Zig implementation and Worker wrapper

## Notes

- this crate is host-side only
- it is not published
- it is mainly useful when validating parity and size/runtime tradeoffs across WASM variants