# md-docrs-core

Core library for `md-docrs`.

This crate owns the shared pipeline used by the CLI, server, Worker, and WASM builds:

- parse `crate[@version][::path::to::item]`
- resolve the matching docs.rs rustdoc JSON URL
- validate and load rustdoc JSON
- resolve an item inside the rustdoc crate
- render the result as Markdown
- provide cache traits and in-memory cache support

## What this crate contains

Main areas of responsibility:

- `spec` — parse and validate item specs
- `resolve` — find items inside a loaded rustdoc JSON crate
- `render` — turn resolved rustdoc items into Markdown
- `fetch` — shared docs.rs URL building and format validation helpers
- `cache` — cache abstractions plus the default in-memory cache

This crate is intentionally runtime-agnostic. It does not perform platform-specific HTTP itself.

## Public API

Typical entry points:

- `ItemSpec` — parsed spec representation
- `render_spec` — high-level fetch + resolve + render flow
- `load_crate` — cache-aware crate loading
- `render_loaded_crate` — render from an already loaded rustdoc crate
- `RustdocFetcher` — fetcher trait implemented by host-specific crates

## Feature flags

- `hybrid-cache` — enables the optional foyer-backed hybrid cache support

Default features are empty.

## Used by

- `md-docrs-cli`
- `md-docrs-server`
- `md-docrs-worker`
- `md-docrs-rust-wasm`

## Notes

This crate focuses on shared logic only:

- no native CLI surface
- no HTTP server
- no Worker bindings
- no docs.rs HTTP client implementation

Those integrations live in sibling crates in the workspace.