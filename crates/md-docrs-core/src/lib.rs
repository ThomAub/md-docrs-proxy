#![warn(clippy::pedantic)]

use rustdoc_types::Crate;
use std::{future::Future, pin::Pin, sync::Arc};

pub mod cache;
pub mod error;
pub mod fetch;
pub mod render;
pub mod resolve;
pub mod spec;

pub use error::{Error, Result};
pub use fetch::{DOCS_RS_BASE, build_url, validate_format_version};
pub use spec::ItemSpec;

pub trait RustdocFetcher: Send + Sync {
    fn fetch<'a>(
        &'a self,
        crate_name: &'a str,
        version: &'a str,
        target: Option<&'a str>,
    ) -> Pin<Box<dyn Future<Output = Result<Crate>> + 'a>>;
}

/// High-level entry point: take a parsed [`ItemSpec`], fetch the rustdoc crate,
/// resolve the requested item, and render Markdown.
///
/// # Errors
/// Forwards:
/// - fetch errors from [`RustdocFetcher::fetch`]
/// - cache-independent resolution errors from [`resolve::resolve`]
pub async fn render_spec(
    spec: &ItemSpec,
    fetcher: &dyn RustdocFetcher,
    cache: &dyn cache::CrateCache,
) -> Result<String> {
    let krate = load_crate(spec, fetcher, cache).await?;
    render_loaded_crate(&krate, spec)
}

/// Load a rustdoc crate through the cache + fetcher abstraction.
///
/// # Errors
/// Returns any error produced by the fetcher.
pub async fn load_crate(
    spec: &ItemSpec,
    fetcher: &dyn RustdocFetcher,
    cache: &dyn cache::CrateCache,
) -> Result<Arc<Crate>> {
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

/// Resolve and render Markdown from an already-loaded rustdoc crate.
///
/// Useful for environments that obtain the crate data elsewhere but still want
/// to reuse the shared resolution + rendering pipeline.
///
/// # Errors
/// Returns resolution errors when the requested item path cannot be found.
pub fn render_loaded_crate(krate: &Crate, spec: &ItemSpec) -> Result<String> {
    let resolved = resolve::resolve(krate, spec)?;
    Ok(render::render(krate, &resolved, spec))
}
