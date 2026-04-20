#![warn(clippy::pedantic)]

pub mod cache;
pub mod error;
#[cfg(feature = "http")]
pub mod fetch;
pub mod render;
pub mod resolve;
pub mod spec;

pub use error::{Error, Result};
pub use spec::ItemSpec;

#[cfg(feature = "http")]
use std::sync::Arc;

/// High-level entry point: take a parsed `ItemSpec`, return rendered Markdown.
///
/// Fetches and caches the crate's rustdoc JSON via the supplied `fetch::Fetcher`
/// and any `cache::CrateCache` implementation, resolves the requested item,
/// and renders it to Markdown.
///
/// # Errors
/// Forwards errors from `Fetcher::fetch` (network / docs.rs / decode failures)
/// and `resolve::resolve` (`Error::NotFound` when the path does not match).
#[cfg(feature = "http")]
pub async fn render_spec(
    spec: &ItemSpec,
    fetcher: &fetch::Fetcher,
    cache: &dyn cache::CrateCache,
) -> Result<String> {
    let krate = load_crate(spec, fetcher, cache).await?;
    let resolved = resolve::resolve(&krate, spec)?;
    Ok(render::render(&krate, &resolved, spec))
}

#[cfg(feature = "http")]
async fn load_crate(
    spec: &ItemSpec,
    fetcher: &fetch::Fetcher,
    cache: &dyn cache::CrateCache,
) -> Result<Arc<rustdoc_types::Crate>> {
    let key = cache::CacheKey {
        crate_name: spec.crate_name.clone(),
        version: spec.version.clone(),
        target: spec.target.clone(),
    };
    if let Some(hit) = cache.get(&key).await {
        return Ok(hit);
    }
    let krate = fetcher
        .fetch(&spec.crate_name, &spec.version, spec.target.as_deref())
        .await?;
    let arc = Arc::new(krate);
    cache.put(key, Arc::clone(&arc)).await;
    Ok(arc)
}
