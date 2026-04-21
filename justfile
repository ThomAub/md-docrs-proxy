default:
    @just --list

# Run all Rust workspace tests.
test:
    cargo test --workspace

# Build the Rust workspace.
build:
    cargo build --workspace

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
    @echo "  cargo run -p md-docrs-server -- --port 8080 --bind 127.0.0.1"
    @echo "  cargo build --profile wasm-release --target wasm32-unknown-unknown -p md-docrs-rust-wasm --no-default-features"
    @echo "  ./wasm/build.sh"
    @echo "  cargo run -p md-docrs-wasm-compare -- --offline"
    @echo "  zig build test --build-file zig/lib/build.zig"
