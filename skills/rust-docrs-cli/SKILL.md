---
name: rust-docrs-cli
description: Use this skill when you need to retrieve or summarize Rust crate or item documentation from docs.rs with the `md-docrs` CLI. Use it for crate-root lookups, item lookups, versioned lookups, target-specific lookups, and for forming the correct command. Do NOT use it for general Rust programming help, source-code editing, deployment, server workflows, Zig, WASM, or unrelated Cargo tasks.
---

# Rust docs.rs CLI

Use this skill to help people get Rust documentation from docs.rs through the `md-docrs` CLI.

## Use this skill when

- the user wants docs for a crate
- the user wants docs for a Rust item
- the user wants docs for a specific crate version
- the user wants docs for a specific target
- the user wants the correct `md-docrs` command
- the user wants the returned docs summarized

## Do not use this skill when

- the task is general Rust advice without docs lookup
- the task is editing or reviewing code
- the task is about servers, deployment, Zig, or WASM
- the task is general Cargo troubleshooting unrelated to `md-docrs`

## Core rules

- Use this spec grammar: `crate[@version][::path::to::item]`
- Prefer `md-docrs <spec>`
- Use `md-docrs --target <triple> <spec>` for target-specific docs
- Give the exact command first
- State that successful output is Markdown on stdout
- Correct invalid specs directly
- Do not invent unsupported flags or URL formats

## Common patterns

- crate root: `anyhow`
- item: `anyhow::Error`
- versioned item: `tokio@1.52.1::sync::Mutex`

## Examples

- `md-docrs anyhow`
- `md-docrs anyhow::Error`
- `md-docrs tokio@1.52.1::sync::Mutex`
- `md-docrs --target x86_64-unknown-linux-gnu tokio::sync::Mutex`

## Fallback

If `md-docrs` is not installed, use:

- `cargo run -p md-docrs-cli -- <spec>`

## Response style

- command first
- shortest valid spec
- include version only when needed
- include target only when needed
- summarize the returned Markdown only if the user wants interpretation

## Additional reference

For more examples and lookup guidance, read `references/usage.md`.
