use crate::{Error, Result};
use rustdoc_types::{Crate, FORMAT_VERSION};

pub const DOCS_RS_BASE: &str = "https://docs.rs";

/// Build the docs.rs rustdoc JSON URL for a crate/version/target tuple.
///
/// When `format_version` is `Some`, the URL is pinned to a specific
/// rustdoc JSON schema version, e.g. `/json/57.zst`.
///
/// When `format_version` is `None`, the legacy unpinned endpoint is used,
/// e.g. `/json.zst`.
#[must_use]
pub fn build_url(
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

/// Shared validation helper for fetcher implementations.
///
/// # Errors
/// Returns `Error::FormatVersionMismatch` when the crate's
/// `format_version` differs from the one supported by this build.
pub fn validate_format_version(krate: &Crate) -> Result<()> {
    if krate.format_version != FORMAT_VERSION {
        return Err(Error::FormatVersionMismatch {
            got: krate.format_version,
            expected: FORMAT_VERSION,
        });
    }
    Ok(())
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
