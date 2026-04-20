#![warn(clippy::pedantic)]

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use md_docrs_proxy::{ItemSpec, cache::CrateCache, fetch::Fetcher, render_spec};
use std::net::SocketAddr;
use std::sync::Arc;

mod server;

#[derive(Parser, Debug)]
#[command(
    name = "md-docrs",
    version,
    about = "Serve Rust crate docs as Markdown via rustdoc JSON"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,

    /// Spec: crate[@version][::path::to::item]. Equivalent to `render` subcommand.
    #[arg(value_name = "SPEC")]
    spec: Option<String>,

    /// Override the target triple (e.g. x86_64-pc-windows-msvc).
    #[arg(long, global = true)]
    target: Option<String>,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Render a single spec to stdout.
    Render {
        spec: String,
        #[arg(long)]
        target: Option<String>,
    },
    /// Run the HTTP server mirroring docs.rs URLs.
    Serve {
        #[arg(long, default_value_t = 8080)]
        port: u16,
        #[arg(long, default_value = "127.0.0.1")]
        bind: String,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    init_tracing();

    let cli = Cli::parse();

    match (cli.command, cli.spec) {
        (Some(Command::Render { spec, target }), _) => {
            render_cmd(&spec, target.or(cli.target)).await
        }
        (Some(Command::Serve { port, bind }), _) => serve_cmd(&bind, port).await,
        (None, Some(spec)) => render_cmd(&spec, cli.target).await,
        (None, None) => {
            eprintln!("usage: md-docrs <SPEC> | md-docrs serve | md-docrs render <SPEC>");
            std::process::exit(2);
        }
    }
}

fn init_tracing() {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .init();
}

async fn render_cmd(raw: &str, target: Option<String>) -> Result<()> {
    let spec = ItemSpec::parse(raw)
        .with_context(|| format!("invalid spec: {raw}"))?
        .with_target(target);
    let fetcher = Fetcher::new()?;
    let cache = CrateCache::default();
    let md = render_spec(&spec, &fetcher, &cache).await?;
    print!("{md}");
    Ok(())
}

async fn serve_cmd(bind: &str, port: u16) -> Result<()> {
    let addr: SocketAddr = format!("{bind}:{port}").parse()?;
    let state = Arc::new(server::AppState {
        fetcher: Fetcher::new()?,
        cache: CrateCache::default(),
    });
    let app = server::router(state);
    tracing::info!(%addr, "md-docrs serve listening");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
