# md-docrs-proxy

Proxy that downloads rustdoc JSON from docs.rs and renders it as Markdown - built for LLM agents that waste tokens scraping docs.rs HTML.

## Build

```sh
cargo build --release
# binary at ./target/release/md-docrs
```

## CLI

Spec grammar: `crate[@version][::path::to::item]`. Version defaults to `latest`.

```sh
md-docrs anyhow                                       # crate index, latest
md-docrs anyhow::Error                                # item page
md-docrs tokio::sync::Mutex                           # follows pub use re-exports
md-docrs tokio@1.52.1::sync::Mutex                    # pinned version
md-docrs --target x86_64-unknown-linux-gnu tokio::sync::Mutex
```

Not every `@version` pin works: docs.rs has to have rebuilt rustdoc JSON at the supported format version (currently 57) for that exact release. Older releases predate the rebuild and return `502`; pin to a recent version or drop the pin to use `latest`.

Markdown goes to stdout. Pipe it into whatever consumes it.

## Server

Mirrors docs.rs URLs, always replies with `text/markdown`:

```sh
md-docrs serve --port 8080 --bind 127.0.0.1
```

```sh
curl -s localhost:8080/anyhow                                       # crate root
curl -s localhost:8080/anyhow/latest/anyhow/struct.Error.html       # item page
curl -s localhost:8080/tokio/latest/tokio/sync/struct.Mutex.html    # re-exported item
```

Response headers: `Content-Type: text/markdown; charset=utf-8`, `X-Markdown-Tokens` (byte-count/4 heuristic), `Vary: Accept`.

Status codes: 404 item not found, 400 bad spec, 502 upstream/decode error.

## Notes

- In-memory LRU cache (32 crates) per process. No disk cache.
- v0 does not render trait impls, blanket impls, or source links.
- Glob re-exports into external crates (e.g. `clap::Parser` from `clap_builder`) are not followed.

## Logging

```sh
RUST_LOG=md_docrs_proxy=debug md-docrs serve
```
