use md_docrs_core::{
    Error, Result, RustdocFetcher,
    fetch::{DOCS_RS_BASE, build_url, validate_format_version},
};
use rustdoc_types::{Crate, FORMAT_VERSION};
use std::{future::Future, io::Cursor, pin::Pin, time::Duration};

/// Native docs.rs fetcher used by the CLI/server binary.
///
/// This implementation is intentionally outside `md-docrs-core` so the core
/// stays transport/runtime agnostic.
pub struct CliFetcher {
    agent: ureq::Agent,
    base: String,
}

impl CliFetcher {
    /// Create a fetcher configured for docs.rs.
    #[must_use]
    pub fn new() -> Self {
        let agent = ureq::AgentBuilder::new()
            .timeout(Duration::from_secs(30))
            .redirects(10)
            .user_agent(concat!("md-docrs-cli/", env!("CARGO_PKG_VERSION")))
            .build();

        Self {
            agent,
            base: DOCS_RS_BASE.to_string(),
        }
    }

    /// Override the docs.rs base URL, mainly for tests.
    #[must_use]
    pub fn with_base(mut self, base: impl Into<String>) -> Self {
        self.base = base.into();
        self
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

impl RustdocFetcher for CliFetcher {
    fn fetch<'a>(
        &'a self,
        crate_name: &'a str,
        version: &'a str,
        target: Option<&'a str>,
    ) -> Pin<Box<dyn Future<Output = Result<Crate>> + 'a>> {
        Box::pin(async move {
            let url = build_url(
                &self.base,
                crate_name,
                version,
                target,
                Some(FORMAT_VERSION),
            );

            let (status, bytes) = self.get_bytes(&url)?;

            if status == 404 {
                let probe_url = build_url(&self.base, crate_name, version, target, None);
                let probe_status = self.head_status(&probe_url)?;
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
    }
}
