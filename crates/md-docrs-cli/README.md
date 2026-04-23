# md-docrs-cli

`md-docrs-cli` provides the `md-docrs` command-line interface for rendering docs.rs Rust API documentation as Markdown.

It resolves a crate or item spec, fetches the corresponding rustdoc JSON from docs.rs, and prints Markdown to stdout.

## Install

From the workspace root:

```bash
cargo install --path crates/md-docrs-cli
```

Run without installing:

```bash
cargo run -p md-docrs-cli -- <SPEC>
```

## Spec format

```text
crate[@version][::path::to::item]
```

Examples:

```bash
md-docrs anyhow
md-docrs anyhow::Error
md-docrs tokio::sync::Mutex
md-docrs tokio@1.52.1::sync::Mutex
md-docrs --target x86_64-unknown-linux-gnu tokio::sync::Mutex
```

## Usage

```text
md-docrs [--target <TRIPLE>] <SPEC>
```

Arguments:

- `<SPEC>` — crate or item to render
- `--target <TRIPLE>` — optional target triple to resolve a target-specific rustdoc JSON build

## Examples

Render a crate landing page as Markdown:

```bash
cargo run -p md-docrs-cli -- anyhow
```

Render a specific item:

```bash
cargo run -p md-docrs-cli -- anyhow::Error
```

Render a version-pinned item:

```bash
cargo run -p md-docrs-cli -- tokio@1.52.1::sync::Mutex
```

Render using a specific target triple:

```bash
cargo run -p md-docrs-cli -- --target x86_64-unknown-linux-gnu tokio::sync::Mutex
```

Pipe the Markdown into a file:

```bash
cargo run -p md-docrs-cli -- serde::Serialize > serde-serialize.md
```

## Output

The command writes Markdown to standard output.

On invalid input or upstream failures, it exits with an error and prints a message to standard error.

## Related crates

- `md-docrs-core` — spec parsing, resolution, rendering, and cache abstractions
- `md-docrs-fetch-http` — native HTTP fetcher used by the CLI

## Workspace

This crate lives in the `md-docrs-proxy` workspace under `crates/md-docrs-cli`.