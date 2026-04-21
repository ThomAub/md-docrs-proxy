# `md-docrs` usage reference

Use this reference for quick, correct `md-docrs` lookups against docs.rs.

## Spec format

`crate[@version][::path::to::item]`

Examples:

- crate root: `anyhow`
- item: `anyhow::Error`
- nested item: `tokio::sync::Mutex`
- versioned item: `tokio@1.52.1::sync::Mutex`

## Command forms

Installed binary:

`md-docrs <spec>`

Target-specific lookup:

`md-docrs --target <triple> <spec>`

Cargo fallback in this repository:

`cargo run -p md-docrs-cli -- <spec>`

## Copy-paste examples

`md-docrs anyhow`

`md-docrs anyhow::Error`

`md-docrs tokio@1.52.1::sync::Mutex`

`md-docrs --target x86_64-unknown-linux-gnu tokio::sync::Mutex`

`cargo run -p md-docrs-cli -- tokio::sync::Mutex`

## Guidance

- give the exact command first
- use the shortest valid spec
- include version only when needed
- include target only when needed
- use `::` for Rust item paths
- successful output is Markdown on stdout

## Avoid

- inventing unsupported flags
- using docs.rs HTML URLs when a spec is enough
- assuming a version the user did not request