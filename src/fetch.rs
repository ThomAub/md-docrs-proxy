use crate::{Error, Result};
use rustdoc_types::{Crate, FORMAT_VERSION};
use std::time::Duration;

/// Downloads rustdoc JSON from docs.rs and parses it with `rustdoc-types`.
pub struct Fetcher {
    client: reqwest::Client,
    base: String,
}

impl Fetcher {
    /// # Errors
    /// Returns `Error::Http` if the underlying HTTP client fails to build.
    pub fn new() -> Result<Self> {
        let client = reqwest::Client::builder()
            .user_agent(concat!("md-docrs-proxy/", env!("CARGO_PKG_VERSION")))
            .timeout(Duration::from_secs(30))
            .redirect(reqwest::redirect::Policy::limited(10))
            .build()?;
        Ok(Self {
            client,
            base: "https://docs.rs".into(),
        })
    }

    /// Override the docs.rs base URL (used in tests).
    #[must_use]
    pub fn with_base(mut self, base: impl Into<String>) -> Self {
        self.base = base.into();
        self
    }

    /// # Errors
    /// Returns `Error::Fetch` on HTTP errors or unsupported format versions,
    /// `Error::Json` on JSON parse failure, and `Error::FormatVersionMismatch`
    /// when the downloaded JSON's `format_version` disagrees with ours.
    pub async fn fetch(
        &self,
        crate_name: &str,
        version: &str,
        target: Option<&str>,
    ) -> Result<Crate> {
        // Always request the format version we can parse. docs.rs keeps
        // multiple format versions during rebuilds, so this is the reliable
        // way to avoid schema-mismatch parse errors. A 404 here means the
        // crate hasn't been rebuilt for our supported format yet.
        let url = build_url(
            &self.base,
            crate_name,
            version,
            target,
            Some(FORMAT_VERSION),
        );
        tracing::debug!(url = %url, "fetch rustdoc JSON");
        let resp = self.client.get(&url).send().await?;

        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            // Distinguish "crate not found" from "format version unavailable"
            // by probing the unpinned endpoint.
            let probe_url = build_url(&self.base, crate_name, version, target, None);
            let probe = self.client.head(&probe_url).send().await?;
            if probe.status().is_success() {
                return Err(Error::Fetch(format!(
                    "{crate_name}@{version} has no rustdoc JSON for format version \
                     {FORMAT_VERSION}; waiting on docs.rs rebuild"
                )));
            }
            return Err(Error::Fetch(format!(
                "{crate_name}@{version} not found on docs.rs"
            )));
        }

        if !resp.status().is_success() {
            return Err(Error::Fetch(format!(
                "{} {} for {crate_name}@{version}",
                resp.status().as_u16(),
                resp.status().canonical_reason().unwrap_or("")
            )));
        }

        let bytes = resp.bytes().await?;

        // Decompress zstd off the tokio runtime - it's CPU-bound.
        let decoded =
            tokio::task::spawn_blocking(move || zstd::decode_all(std::io::Cursor::new(bytes)))
                .await
                .map_err(|e| Error::Fetch(format!("zstd decode panicked: {e}")))??;

        let krate: Crate = serde_json::from_slice(&decoded)?;
        if krate.format_version != FORMAT_VERSION {
            return Err(Error::FormatVersionMismatch {
                got: krate.format_version,
                expected: FORMAT_VERSION,
            });
        }
        Ok(krate)
    }
}

fn build_url(
    base: &str,
    crate_name: &str,
    version: &str,
    target: Option<&str>,
    format_version: Option<u32>,
) -> String {
    let target_seg = target.map(|t| format!("/{t}")).unwrap_or_default();
    match format_version {
        Some(v) => format!("{base}/crate/{crate_name}/{version}{target_seg}/json/{v}.zst"),
        None => format!("{base}/crate/{crate_name}/{version}{target_seg}/json.zst"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn url_basic() {
        assert_eq!(
            build_url("https://docs.rs", "serde", "latest", None, None),
            "https://docs.rs/crate/serde/latest/json.zst"
        );
    }

    #[test]
    fn url_with_target() {
        assert_eq!(
            build_url(
                "https://docs.rs",
                "serde",
                "latest",
                Some("x86_64-pc-windows-msvc"),
                None
            ),
            "https://docs.rs/crate/serde/latest/x86_64-pc-windows-msvc/json.zst"
        );
    }

    #[test]
    fn url_format_pinned() {
        assert_eq!(
            build_url("https://docs.rs", "serde", "1.0.200", None, Some(57)),
            "https://docs.rs/crate/serde/1.0.200/json/57.zst"
        );
    }
}
