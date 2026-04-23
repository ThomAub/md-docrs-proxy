# md-docrs-worker

`md-docrs-worker` is the hosted Markdown rendering entrypoint for the `md-docrs` workspace.

It runs as a Cloudflare Worker and serves Markdown generated from docs.rs rustdoc JSON. It is the hosted counterpart to the native `md-docrs` CLI and `md-docrs-server`.

## What it does

This crate:

- accepts crate specs and docs.rs-style paths over HTTP
- fetches rustdoc JSON from docs.rs
- decompresses and parses the payload inside the Worker
- renders the requested crate or item as Markdown
- caches decoded rustdoc crates in Cloudflare KV

This makes it suitable for a hosted MCP-friendly deployment where the Worker provides a simple HTTP interface and Cloudflare handles the edge runtime.

## Routes

The Worker supports these request forms:

- `GET /?spec=<spec>`
- `GET /?spec=<spec>&target=<triple>`
- `GET /<crate>`
- `GET /<crate>/<version>`
- `GET /<crate>/<version>/<crate>/<module>/.../struct.Name.html`
- `GET /healthz`
- `GET /kv`

The `target` query parameter is optional and overrides the docs.rs target triple.

## Examples

Render a crate root:

```/dev/null/worker-readme-curl-1.sh#L1-1
curl 'http://127.0.0.1:8787/?spec=anyhow'
```

Render a specific item from a query spec:

```/dev/null/worker-readme-curl-2.sh#L1-1
curl 'http://127.0.0.1:8787/?spec=tokio::sync::Mutex'
```

Render using a docs.rs-style path:

```/dev/null/worker-readme-curl-3.sh#L1-1
curl 'http://127.0.0.1:8787/tokio/latest/tokio/sync/struct.Mutex.html'
```

Render for a specific target:

```/dev/null/worker-readme-curl-4.sh#L1-1
curl 'http://127.0.0.1:8787/?spec=anyhow::Error&target=x86_64-unknown-linux-gnu'
```

Health check:

```/dev/null/worker-readme-curl-5.sh#L1-1
curl 'http://127.0.0.1:8787/healthz'
```

## Responses

Successful render responses return:

- status `200`
- `Content-Type: text/markdown; charset=utf-8`

Error behavior:

- `400` for invalid specs
- `404` for missing items
- `502` for upstream fetch, decode, format, or JSON errors

The root route `/` returns a short plain-text usage message.

## Cache

The Worker uses Cloudflare KV for crate-level caching.

Cache keys are derived from:

- crate name
- version
- optional target triple

Cached values contain the decoded rustdoc crate payload, which avoids repeating fetch and decode work for hot entries.

## Local development

This crate is intended to run in the Cloudflare Worker environment.

Typical local flow from the repository root:

```/dev/null/worker-readme-local.sh#L1-2
cargo test --workspace
cargo build --workspace
```

Then run the Worker with your usual Cloudflare local workflow for this repository.

## Relationship to other crates

- `md-docrs-core` provides spec parsing, URL resolution, rendering, and shared error types
- `md-docrs-worker` provides the hosted HTTP interface and KV-backed cache
- `md-docrs-cli` is the native CLI for local use
- `md-docrs-server` is the native server alternative

## Notes

- this crate is not published independently
- it is designed for Cloudflare Worker deployment
- it is the main hosted path to mention when describing a remotely deployed MCP-compatible setup