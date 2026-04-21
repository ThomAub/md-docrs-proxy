#![warn(clippy::pedantic)]

use async_trait::async_trait;
use md_docrs_core::{
    Error, Result, RustdocFetcher,
    fetch::{DOCS_RS_BASE, build_url, validate_format_version},
};
use rustdoc_types::{Crate, FORMAT_VERSION};
use std::{io::Cursor, time::Duration};
use tokio::task;

/// Native docs.rs fetcher shared by the CLI and native server crates.
///
/// This lives outside `md-docrs-core` so the core remains transport-agnostic.
pub struct UreqRustdocFetcher {
    agent: ureq::Agent,
    base: String,
    user_agent: String,
}

impl UreqRustdocFetcher {
    /// Create a fetcher configured for docs.rs with a default user agent.
    #[must_use]
    pub fn new() -> Self {
        Self::with_user_agent(concat!("md-docrs/", env!("CARGO_PKG_VERSION")))
    }

    /// Create a fetcher with a custom user agent string.
    #[must_use]
    pub fn with_user_agent(user_agent: impl Into<String>) -> Self {
        let user_agent = user_agent.into();
        let agent = ureq::AgentBuilder::new()
            .timeout(Duration::from_secs(30))
            .redirects(10)
            .user_agent(&user_agent)
            .build();

        Self {
            agent,
            base: DOCS_RS_BASE.to_string(),
            user_agent,
        }
    }

    /// Override the docs.rs base URL, mainly for tests.
    #[must_use]
    pub fn with_base(mut self, base: impl Into<String>) -> Self {
        self.base = base.into();
        self
    }

    /// Return the configured user agent.
    #[must_use]
    pub fn user_agent(&self) -> &str {
        &self.user_agent
    }

    fn read_body_bytes(response: ureq::Response, url: &str) -> Result<Vec<u8>> {
        let mut reader = response.into_reader();
        let mut bytes = Vec::new();
        std::io::Read::read_to_end(&mut reader, &mut bytes).map_err(|err| {
            Error::Fetch(format!("failed to read response body for {url}: {err}"))
        })?;
        Ok(bytes)
    }

    fn get_bytes(&self, url: &str) -> Result<(u16, Vec<u8>)> {
        match self.agent.get(url).call() {
            Ok(response) => {
                let status = response.status();
                let bytes = Self::read_body_bytes(response, url)?;
                Ok((status, bytes))
            }
            Err(ureq::Error::Status(status, response)) => {
                let bytes = Self::read_body_bytes(response, url)?;
                Ok((status, bytes))
            }
            Err(err) => Err(Error::Fetch(format!("request failed for {url}: {err}"))),
        }
    }

    fn head_status(&self, url: &str) -> Result<u16> {
        match self.agent.head(url).call() {
            Ok(response) => Ok(response.status()),
            Err(ureq::Error::Status(status, _response)) => Ok(status),
            Err(err) => Err(Error::Fetch(format!("request failed for {url}: {err}"))),
        }
    }
}

impl Default for UreqRustdocFetcher {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl RustdocFetcher for UreqRustdocFetcher {
    async fn fetch(&self, crate_name: &str, version: &str, target: Option<&str>) -> Result<Crate> {
        let url = build_url(
            &self.base,
            crate_name,
            version,
            target,
            Some(FORMAT_VERSION),
        );
        let probe_url = build_url(&self.base, crate_name, version, target, None);
        let fetcher = Self {
            agent: self.agent.clone(),
            base: self.base.clone(),
            user_agent: self.user_agent.clone(),
        };
        let crate_name = crate_name.to_string();
        let version = version.to_string();

        task::spawn_blocking(move || {
            let (status, bytes) = fetcher.get_bytes(&url)?;

            if status == 404 {
                let probe_status = fetcher.head_status(&probe_url)?;
                if (200..300).contains(&probe_status) {
                    return Err(Error::Fetch(format!(
                        "{crate_name}@{version} has no rustdoc JSON for format version {FORMAT_VERSION}; waiting on docs.rs rebuild"
                    )));
                }
                return Err(Error::Fetch(format!(
                    "{crate_name}@{version} not found on docs.rs"
                )));
            }

            if !(200..300).contains(&status) {
                return Err(Error::Fetch(format!(
                    "{status} response for {crate_name}@{version}"
                )));
            }

            let decoded = zstd::decode_all(Cursor::new(bytes))?;
            let krate: Crate = serde_json::from_slice(&decoded)?;
            validate_format_version(&krate)?;
            Ok(krate)
        })
        .await
        .map_err(|err| Error::Fetch(format!("blocking fetch task failed: {err}")))?
    }
}
