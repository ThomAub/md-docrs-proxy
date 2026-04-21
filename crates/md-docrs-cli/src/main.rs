#![warn(clippy::pedantic)]

use anyhow::{Context, Result};
use clap::Parser;
use md_docrs_core::{ItemSpec, cache::InMemoryCache, render_spec};
use md_docrs_fetch_http::UreqRustdocFetcher;

#[derive(Parser, Debug)]
#[command(
    name = "md-docrs",
    version,
    about = "Render Rust crate docs as Markdown via rustdoc JSON"
)]
struct Cli {
    /// Spec: crate[@version][::path::to::item].
    #[arg(value_name = "SPEC")]
    spec: String,

    /// Override the target triple (e.g. x86_64-pc-windows-msvc).
    #[arg(long)]
    target: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    init_tracing();

    let cli = Cli::parse();
    let spec = ItemSpec::parse(&cli.spec)
        .with_context(|| format!("invalid spec: {}", cli.spec))?
        .with_target(cli.target);

    let fetcher =
        UreqRustdocFetcher::with_user_agent(concat!("md-docrs-cli/", env!("CARGO_PKG_VERSION")));
    let cache = InMemoryCache::default();

    let md = render_spec(&spec, &fetcher, &cache).await?;
    print!("{md}");

    Ok(())
}

fn init_tracing() {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .init();
}
