# md-docrs-server

`md-docrs-server` is the native HTTP server for `md-docrs`.

It fetches rustdoc JSON from docs.rs, resolves requested items, renders them as Markdown, and serves the result over HTTP.

## What it does

- accepts crate and item requests over HTTP
- fetches rustdoc JSON from docs.rs
- renders Markdown responses
- exposes a simple health endpoint
- supports in-memory caching by default
- supports an optional memory + disk hybrid cache via the `hybrid-cache` feature

## Run locally

From the workspace root:

```bash
cargo run -p md-docrs-server -- --port 8080 --bind 127.0.0.1
```

Then query it with `curl`:

```bash
curl -sS http://127.0.0.1:8080/anyhow
curl -sS http://127.0.0.1:8080/anyhow/latest/anyhow/struct.Error.html
curl -sS http://127.0.0.1:8080/tokio/latest/tokio/sync/struct.Mutex.html
curl -sS http://127.0.0.1:8080/healthz
```

## Routes

Supported request forms:

- `GET /<crate>`
- `GET /<crate>/<version>`
- `GET /<crate>/<version>/<crate>/<path>/struct.Name.html`
- `GET /?spec=<crate[@version][::path::to::item]>`
- `GET /?spec=<crate[@version][::path::to::item]>&target=<triple>`
- `GET /healthz`

The server also accepts `?target=<triple>` on path-based requests.

Examples:

```bash
curl -sS http://127.0.0.1:8080/serde
curl -sS http://127.0.0.1:8080/tokio/1.52.1/tokio/sync/struct.Mutex.html
curl -sS 'http://127.0.0.1:8080/?spec=anyhow::Error'
curl -sS 'http://127.0.0.1:8080/tokio/latest/tokio/sync/struct.Mutex.html?target=x86_64-unknown-linux-gnu'
```

## CLI options

```bash
md-docrs-server --port <PORT> --bind <ADDR> [--cache-dir <DIR>]
                [--cache-disk-bytes <BYTES>]
                [--cache-memory-bytes <BYTES>]
```

Options:

- `--port` — listen port, defaults to `8080`
- `--bind` — bind address, defaults to `127.0.0.1`
- `--cache-dir` — enable the foyer-backed hybrid cache and store the disk tier in this directory
- `--cache-disk-bytes` — disk tier capacity in bytes
- `--cache-memory-bytes` — memory tier weight budget in bytes

## Response behavior

- `200` with `Content-Type: text/markdown; charset=utf-8` for successful item renders
- `200` with plain text for `/healthz`
- `400` for invalid specs
- `404` for missing items
- `502` for upstream fetch, decode, format, or JSON errors

Successful Markdown responses also include:

- `Vary: Accept`
- `X-Markdown-Tokens` — a rough token estimate derived from output size

## Caching

By default, the server uses an in-memory cache.

With the default crate features, you can enable a foyer-backed hybrid cache by passing `--cache-dir`:

```bash
cargo run -p md-docrs-server -- --cache-dir .cache/md-docrs
```

If the binary is built without the `hybrid-cache` feature, `--cache-dir` is rejected.

Build without that feature:

```bash
cargo build -p md-docrs-server --no-default-features
```

Build explicitly with the feature:

```bash
cargo build -p md-docrs-server --features hybrid-cache
```

## Relationship to the workspace

This crate is the native HTTP-serving entry point.

Related crates:

- `md-docrs-core` — spec parsing, resolution, rendering, cache traits
- `md-docrs-fetch-http` — native docs.rs fetcher
- `md-docrs-cli` — native CLI that prints Markdown to stdout
- `md-docrs-worker` — Cloudflare Worker version

## Notes

- this crate is not published
- it is intended for native server deployments
- item path parsing strips rustdoc kind prefixes like `struct.` and the `.html` suffix from the final path segment