use async_trait::async_trait;
use rustdoc_types::Crate;
use std::sync::Arc;

#[cfg(feature = "hybrid-cache")]
use serde::{Deserialize, Serialize};

mod memory;
#[cfg(feature = "hybrid-cache")]
mod hybrid;

pub use memory::InMemoryCache;
#[cfg(feature = "hybrid-cache")]
pub use hybrid::{FoyerHybridCache, FoyerHybridCacheConfig};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "hybrid-cache", derive(Serialize, Deserialize))]
pub struct CacheKey {
    pub crate_name: String,
    pub version: String,
    pub target: Option<String>,
}

/// A pluggable backing store for parsed rustdoc `Crate`s.
///
/// Implementations may live in-process (see [`InMemoryCache`]), as a
/// memory+disk hybrid (see [`FoyerHybridCache`] behind the `hybrid-cache`
/// feature), or in a remote key-value store such as Cloudflare Workers KV.
/// Methods are async so backends that perform I/O can await without
/// blocking.
#[async_trait]
pub trait CrateCache: Send + Sync {
    async fn get(&self, key: &CacheKey) -> Option<Arc<Crate>>;
    async fn put(&self, key: CacheKey, value: Arc<Crate>);
}
