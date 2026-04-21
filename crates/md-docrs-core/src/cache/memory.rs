use super::{CacheKey, CrateCache};
use async_trait::async_trait;
use lru::LruCache;
use rustdoc_types::Crate;
use std::num::NonZeroUsize;
use std::sync::{Arc, Mutex};

/// Thread-safe, bounded LRU holding parsed `Crate`s behind an `Arc` so
/// renderers can operate without holding the cache lock.
pub struct InMemoryCache {
    inner: Mutex<LruCache<CacheKey, Arc<Crate>>>,
}

impl InMemoryCache {
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        let cap = NonZeroUsize::new(capacity).unwrap_or(NonZeroUsize::MIN);
        Self {
            inner: Mutex::new(LruCache::new(cap)),
        }
    }
}

impl Default for InMemoryCache {
    fn default() -> Self {
        Self::new(32)
    }
}

#[async_trait]
impl CrateCache for InMemoryCache {
    async fn get(&self, key: &CacheKey) -> Option<Arc<Crate>> {
        self.inner.lock().ok()?.get(key).cloned()
    }

    async fn put(&self, key: CacheKey, value: Arc<Crate>) {
        if let Ok(mut g) = self.inner.lock() {
            g.put(key, value);
        }
    }
}
