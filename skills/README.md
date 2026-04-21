# Skills

This directory contains reusable agent skills for working with `md-docrs` and related workflows.

## Available skills

### `rust-docrs-cli`

Use this skill when you need to retrieve, inspect, or summarize Rust crate and item documentation from docs.rs with the `md-docrs` CLI.

Typical uses:

- get docs for a crate root like `anyhow`
- get docs for an item like `anyhow::Error`
- get docs for a versioned item like `tokio@1.52.1::sync::Mutex`
- get docs for a target-specific item with `--target`
- form the correct `md-docrs` command from a user request
- summarize the Markdown returned by the CLI

Path:

- `skills/rust-docrs-cli/SKILL.md`

Reference material:

- `skills/rust-docrs-cli/references/usage.md`

## Notes

Keep each skill focused:

- put trigger logic and core instructions in `SKILL.md`
- put longer examples and lookup details in `references/`
- avoid mixing unrelated workflows into one skill