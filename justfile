default:
    @just --list

# Run all Rust workspace tests.
test:
    cargo test --workspace

# Build the Rust workspace.
build:
    cargo build --workspace

# Render Markdown for a docs.rs spec with the native CLI.
cli spec="anyhow":
    cargo run -p md-docrs-cli -- {{ spec }}

# Render Markdown for a docs.rs spec with an explicit target triple.
cli-target spec="tokio::sync::Mutex" target="x86_64-unknown-linux-gnu":
    cargo run -p md-docrs-cli -- --target {{ target }} {{ spec }}

# Preview the release artifacts cargo-dist would produce for a tag.
dist-plan tag="v0.1.0":
    dist plan --tag {{ tag }}

# Build the current platform's release artifacts for a tag.
dist-build tag="v0.1.0":
    dist build --tag {{ tag }}

# Preview the cargo-release flow for a version.
release-preview version="0.1.0":
    cargo release {{ version }}

# Create the release commit/tag and publish crates, but keep the remote push explicit.
release-run version="0.1.0":
    cargo release {{ version }} --execute --no-push

# Build the Cloudflare Worker crate for wasm.
build-worker:
    cargo check -p md-docrs-worker --target wasm32-unknown-unknown

# Run the Cloudflare Worker locally with Wrangler.
worker-dev:
    npx wrangler@latest dev --config wrangler.toml --cwd crates/md-docrs-worker --local --port 8787 --persist-to .wrangler/state

# Probe the worker root with a crate spec in the path.
curl-worker spec="anyhow":
    curl -sS "http://127.0.0.1:8787/{{ spec }}"

# Probe the worker with a target triple query parameter.
curl-worker-target spec="tokio::sync::Mutex" target="x86_64-unknown-linux-gnu":
    curl -sS "http://127.0.0.1:8787/{{ spec }}?target={{ target }}"

# Probe the worker using the spec query parameter form.
curl-worker-query spec="anyhow::Error":
    curl -sS "http://127.0.0.1:8787/?spec={{ spec }}"

# Run a few common worker smoke tests.
test-worker:
    just curl-worker anyhow
    echo
    just curl-worker-query "anyhow::Error"
    echo
    just curl-worker-target "tokio::sync::Mutex" "x86_64-unknown-linux-gnu"

# Run the native Markdown server locally.
server-dev:
    cargo run -p md-docrs-server -- --port 8080 --bind 127.0.0.1

# Probe the native server.
curl-server path="anyhow":
    curl -sS "http://127.0.0.1:8080/{{ path }}"

# Run the WASM comparison flow described in the repo docs.
wasm-compare:
    ./wasm/build.sh
    cargo run -p md-docrs-wasm-compare -- --offline

# Run Zig tests from the repo root.
zig-test:
    zig build test --build-file zig/lib/build.zig

# Show the main commands collected from workspace READMEs.
help-commands:
    @echo "Common commands from README files:"
    @echo "  cargo build --workspace"
    @echo "  cargo test --workspace"
    @echo "  cargo run -p md-docrs-cli -- anyhow"
    @echo "  cargo run -p md-docrs-cli -- --target x86_64-unknown-linux-gnu tokio::sync::Mutex"
    @echo "  cargo run -p md-docrs-server -- --port 8080 --bind 127.0.0.1"
    @echo "  dist plan --tag v0.1.0"
    @echo "  dist build --tag v0.1.0"
    @echo "  cargo release 0.2.0"
    @echo "  cargo release 0.2.0 --execute --no-push"
    @echo "  cargo build --profile wasm-release --target wasm32-unknown-unknown -p md-docrs-rust-wasm --no-default-features"
    @echo "  ./wasm/build.sh"
    @echo "  cargo run -p md-docrs-wasm-compare -- --offline"
    @echo "  zig build test --build-file zig/lib/build.zig"
