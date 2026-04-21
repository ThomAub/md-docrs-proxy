#![warn(clippy::pedantic)]

use anyhow::{Context, Result};
use md_docrs_core::cache::{CrateCache, InMemoryCache};
#[cfg(feature = "hybrid-cache")]
use md_docrs_core::cache::{FoyerHybridCache, FoyerHybridCacheConfig};
use md_docrs_fetch_http::UreqRustdocFetcher;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

mod server;

#[derive(clap::Parser, Debug)]
#[command(
    name = "md-docrs-server",
    version,
    about = "Serve Rust crate docs as Markdown via rustdoc JSON"
)]
struct Cli {
    #[arg(long, default_value_t = 8080)]
    port: u16,

    #[arg(long, default_value = "127.0.0.1")]
    bind: String,

    /// Enable the memory+disk hybrid cache backed by foyer. When set, the
    /// directory is created if missing and used as the disk tier.
    #[arg(long, value_name = "DIR")]
    cache_dir: Option<PathBuf>,

    /// Disk tier capacity in bytes. Only applied when `--cache-dir` is set.
    #[arg(long, default_value_t = 4 * 1024 * 1024 * 1024)]
    cache_disk_bytes: usize,

    /// Memory tier weight budget in bytes. Only applied when `--cache-dir` is set.
    #[arg(long, default_value_t = 256 * 1024 * 1024)]
    cache_memory_bytes: usize,
}

#[tokio::main]
async fn main() -> Result<()> {
    init_tracing();

    let cli = <Cli as clap::Parser>::parse();
    let addr: SocketAddr = format!("{}:{}", cli.bind, cli.port).parse()?;

    let cache = build_cache(cli.cache_dir, cli.cache_disk_bytes, cli.cache_memory_bytes).await?;

    let state = Arc::new(server::AppState {
        fetcher: Arc::new(UreqRustdocFetcher::new()),
        cache,
    });

    let app = server::router(state);

    tracing::info!(%addr, "md-docrs-server listening");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

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

#[cfg(feature = "hybrid-cache")]
async fn build_cache(
    cache_dir: Option<PathBuf>,
    disk_bytes: usize,
    memory_bytes: usize,
) -> Result<Arc<dyn CrateCache>> {
    if let Some(dir) = cache_dir {
        std::fs::create_dir_all(&dir)
            .with_context(|| format!("create cache dir {}", dir.display()))?;

        tracing::info!(
            dir = %dir.display(),
            disk_bytes,
            memory_bytes,
            "using foyer hybrid cache"
        );

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
             `hybrid-cache` feature; rebuild with `cargo build -p md-docrs-server --features hybrid-cache`"
        );
    }

    Ok(Arc::new(InMemoryCache::default()))
}
