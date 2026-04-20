#![warn(clippy::pedantic)]

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use md_docrs_proxy::{
    ItemSpec,
    cache::{CrateCache, InMemoryCache},
    fetch::Fetcher,
    render_spec,
};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

#[cfg(feature = "hybrid-cache")]
use md_docrs_proxy::cache::{FoyerHybridCache, FoyerHybridCacheConfig};

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
        /// Enable the memory+disk hybrid cache backed by foyer; requires the
        /// `hybrid-cache` feature. When set, the directory is created if
        /// missing and used as the disk tier.
        #[arg(long, value_name = "DIR")]
        cache_dir: Option<PathBuf>,
        /// Disk tier capacity in bytes. Only applied when `--cache-dir` is
        /// set.
        #[arg(long, default_value_t = 4 * 1024 * 1024 * 1024)]
        cache_disk_bytes: usize,
        /// Memory tier weight budget in bytes. Only applied when
        /// `--cache-dir` is set.
        #[arg(long, default_value_t = 256 * 1024 * 1024)]
        cache_memory_bytes: usize,
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
        (
            Some(Command::Serve {
                port,
                bind,
                cache_dir,
                cache_disk_bytes,
                cache_memory_bytes,
            }),
            _,
        ) => {
            serve_cmd(
                &bind,
                port,
                cache_dir,
                cache_disk_bytes,
                cache_memory_bytes,
            )
            .await
        }
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
    let cache = InMemoryCache::default();
    let md = render_spec(&spec, &fetcher, &cache).await?;
    print!("{md}");
    Ok(())
}

async fn serve_cmd(
    bind: &str,
    port: u16,
    cache_dir: Option<PathBuf>,
    cache_disk_bytes: usize,
    cache_memory_bytes: usize,
) -> Result<()> {
    let addr: SocketAddr = format!("{bind}:{port}").parse()?;
    let cache = build_cache(cache_dir, cache_disk_bytes, cache_memory_bytes).await?;
    let state = Arc::new(server::AppState {
        fetcher: Fetcher::new()?,
        cache,
    });
    let app = server::router(state);
    tracing::info!(%addr, "md-docrs serve listening");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

#[cfg(feature = "hybrid-cache")]
async fn build_cache(
    cache_dir: Option<PathBuf>,
    disk_bytes: usize,
    memory_bytes: usize,
) -> Result<Arc<dyn CrateCache>> {
    if let Some(dir) = cache_dir {
        std::fs::create_dir_all(&dir)
            .with_context(|| format!("create cache dir {}", dir.display()))?;
        tracing::info!(dir = %dir.display(), disk_bytes, memory_bytes, "using foyer hybrid cache");
        let hybrid = FoyerHybridCache::new(FoyerHybridCacheConfig {
            dir,
            memory_capacity_bytes: memory_bytes,
            disk_capacity_bytes: disk_bytes,
        })
        .await?;
        Ok(Arc::new(hybrid))
    } else {
        Ok(Arc::new(InMemoryCache::default()))
    }
}

#[cfg(not(feature = "hybrid-cache"))]
#[allow(clippy::unused_async)]
async fn build_cache(
    cache_dir: Option<PathBuf>,
    _disk_bytes: usize,
    _memory_bytes: usize,
) -> Result<Arc<dyn CrateCache>> {
    if cache_dir.is_some() {
        anyhow::bail!(
            "--cache-dir was supplied but this binary was built without the \
             `hybrid-cache` feature; rebuild with `cargo build --features hybrid-cache`"
        );
    }
    Ok(Arc::new(InMemoryCache::default()))
}
