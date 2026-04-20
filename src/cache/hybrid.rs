use super::{CacheKey, CrateCache};
use async_trait::async_trait;
use foyer::{
    BlockEngineConfig, DeviceBuilder, FsDeviceBuilder, HybridCache, HybridCacheBuilder,
    HybridCachePolicy, PsyncIoEngineConfig, RecoverMode,
};
use rustdoc_types::Crate;
use std::path::PathBuf;
use std::sync::Arc;

/// Knobs for [`FoyerHybridCache`].
///
/// The defaults are tuned for a small single-node deployment; production
/// should size `memory_capacity_bytes` and `disk_capacity_bytes` to the host.
#[derive(Debug, Clone)]
pub struct FoyerHybridCacheConfig {
    /// Filesystem path used by the disk tier. Must exist.
    pub dir: PathBuf,
    /// In-memory tier weight budget (bytes). Entry weight is the estimated
    /// serialized size of the `Crate`.
    pub memory_capacity_bytes: usize,
    /// Disk tier capacity (bytes). Backed by a single `FsDevice`.
    pub disk_capacity_bytes: usize,
}

impl FoyerHybridCacheConfig {
    #[must_use]
    pub fn new(dir: PathBuf) -> Self {
        Self {
            dir,
            memory_capacity_bytes: 256 * 1024 * 1024,
            disk_capacity_bytes: 4 * 1024 * 1024 * 1024,
        }
    }
}

/// Memory + disk hybrid cache backed by [`foyer`].
///
/// Parsed `Crate`s are held as `Arc<Crate>` in the in-memory tier and
/// serialized (bincode, via serde) to the on-disk tier. This gives the
/// process a much larger working set than pure memory while keeping hot
/// gets at memory speed.
pub struct FoyerHybridCache {
    inner: HybridCache<CacheKey, Arc<Crate>>,
}

impl FoyerHybridCache {
    /// Build a new hybrid cache. `cfg.dir` must be an existing directory.
    ///
    /// # Errors
    /// Returns any I/O error surfaced by the underlying `FsDevice` or
    /// `HybridCacheBuilder`.
    pub async fn new(cfg: FoyerHybridCacheConfig) -> anyhow::Result<Self> {
        let device = FsDeviceBuilder::new(&cfg.dir)
            .with_capacity(cfg.disk_capacity_bytes)
            .build()?;
        let inner: HybridCache<CacheKey, Arc<Crate>> = HybridCacheBuilder::new()
            .with_name("md-docrs-proxy")
            .with_policy(HybridCachePolicy::WriteOnInsertion)
            .memory(cfg.memory_capacity_bytes)
            .with_weighter(|_k: &CacheKey, v: &Arc<Crate>| {
                // Rough estimate - the exact serialized size is bincode-derived
                // and expensive to compute, and foyer only needs monotonicity.
                v.index.len().saturating_mul(512)
            })
            .storage()
            .with_io_engine_config(PsyncIoEngineConfig::new())
            .with_engine_config(BlockEngineConfig::new(device))
            .with_recover_mode(RecoverMode::Quiet)
            .build()
            .await?;
        Ok(Self { inner })
    }

    /// Flush outstanding writes and shut the cache down cleanly.
    ///
    /// # Errors
    /// Forwards errors from `foyer::HybridCache::close`.
    pub async fn close(&self) -> anyhow::Result<()> {
        self.inner.close().await?;
        Ok(())
    }
}

#[async_trait]
impl CrateCache for FoyerHybridCache {
    async fn get(&self, key: &CacheKey) -> Option<Arc<Crate>> {
        match self.inner.get(key).await {
            Ok(Some(entry)) => Some(entry.value().clone()),
            Ok(None) => None,
            Err(e) => {
                tracing::warn!(error = %e, "foyer hybrid cache get failed");
                None
            }
        }
    }

    async fn put(&self, key: CacheKey, value: Arc<Crate>) {
        self.inner.insert(key, value);
    }
}
