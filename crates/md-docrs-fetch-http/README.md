# md-docrs-fetch-http

HTTP fetcher for `md-docrs`.

This crate is the native transport layer used to download rustdoc JSON from docs.rs, decompress it, deserialize it, and hand a validated `rustdoc_types::Crate` back to the rest of the system.

## Purpose

`md-docrs-fetch-http` owns the network-facing part of the pipeline for native Rust targets:

- build the docs.rs rustdoc JSON URL
- fetch the `.zst` payload over HTTPS
- decompress the response
- deserialize rustdoc JSON
- validate the rustdoc format version

It is used by:

- `md-docrs-cli`
- `md-docrs-server`

It is not used by the Cloudflare Worker path, which has its own worker-specific fetch implementation.

## Scope

This crate does:

- HTTP fetching from docs.rs
- zstd decompression
- rustdoc JSON decoding
- format-version validation

This crate does not do:

- spec parsing
- item resolution
- Markdown rendering
- caching policy
- HTTP server routing
- CLI argument parsing

Those responsibilities live in other crates, primarily `md-docrs-core`, `md-docrs-cli`, and `md-docrs-server`.

## Dependency relationship

`md-docrs-fetch-http` depends on `md-docrs-core` for shared types and validation helpers.

Typical flow:

1. another crate parses a spec into `ItemSpec`
2. core/cache logic decides whether a fetch is needed
3. this crate downloads and decodes the rustdoc JSON crate
4. `md-docrs-core` resolves the requested item and renders Markdown

## Native-only role

This crate is intended for native environments.

It currently uses `ureq` for HTTP and `zstd` for decompression, which makes it a good fit for the CLI and server binaries but not for the Worker/WASM deployment targets.

## Build

From the repository root:

```bash
cargo build -p md-docrs-fetch-http
```

## Test

From the repository root:

```bash
cargo test -p md-docrs-fetch-http
```

## Related crates

- `md-docrs-core` — shared spec parsing, URL building helpers, resolution, rendering, cache traits
- `md-docrs-cli` — native CLI that prints Markdown to stdout
- `md-docrs-server` — native HTTP server
- `md-docrs-worker` — Cloudflare Worker implementation with its own fetch path

## Notes

This crate is intentionally small and focused.

If you need to change how docs.rs payloads are fetched on native targets, this is the crate to update.