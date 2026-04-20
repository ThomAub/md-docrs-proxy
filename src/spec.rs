use crate::{Error, Result};

/// A parsed reference to a rustdoc item on docs.rs.
///
/// Examples of accepted grammar:
///   - `serde`
///   - `serde@1.0.200`
///   - `serde::de::Deserialize`
///   - `serde@~1::Serializer`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemSpec {
    pub crate_name: String,
    /// Version string as accepted by docs.rs: "latest", "1.0.200", "~1", etc.
    pub version: String,
    /// Rustdoc target triple; `None` means the default host target.
    pub target: Option<String>,
    /// Path components *without* the crate name, e.g. `["de", "Deserialize"]`.
    /// Empty means the crate root module.
    pub path: Vec<String>,
}

impl ItemSpec {
    /// # Errors
    /// Returns `Error::InvalidSpec` when `raw` is empty, when the
    /// `crate@version` portion is malformed, or when any segment fails the
    /// crate-name / identifier validity rules.
    pub fn parse(raw: &str) -> Result<Self> {
        let raw = raw.trim();
        if raw.is_empty() {
            return Err(Error::InvalidSpec("empty".into()));
        }

        // Split off the path on `::`. The first segment is `crate[@version]`.
        let (head, rest) = raw.split_once("::").unwrap_or((raw, ""));
        let path: Vec<String> = if rest.is_empty() {
            Vec::new()
        } else {
            rest.split("::").map(str::to_string).collect()
        };

        let (crate_name, version) = match head.split_once('@') {
            Some((c, v)) if !c.is_empty() && !v.is_empty() => (c.to_string(), v.to_string()),
            Some(_) => return Err(Error::InvalidSpec(format!("bad crate@version: {head}"))),
            None => (head.to_string(), "latest".to_string()),
        };

        if !is_valid_crate_name(&crate_name) {
            return Err(Error::InvalidSpec(format!("bad crate name: {crate_name}")));
        }
        for seg in &path {
            if !is_valid_ident(seg) {
                return Err(Error::InvalidSpec(format!("bad path segment: {seg}")));
            }
        }

        Ok(Self {
            crate_name,
            version,
            target: None,
            path,
        })
    }

    #[must_use]
    pub fn with_target(mut self, target: Option<String>) -> Self {
        self.target = target;
        self
    }

    #[must_use]
    pub fn is_root(&self) -> bool {
        self.path.is_empty()
    }

    /// Full path as rustdoc sees it: `[crate_name, ..path]`.
    #[must_use]
    pub fn full_path(&self) -> Vec<String> {
        let mut out = Vec::with_capacity(self.path.len() + 1);
        out.push(self.crate_name.clone());
        out.extend(self.path.iter().cloned());
        out
    }
}

/// Matches the crates.io/docs.rs rules:
/// non-empty, <= 64 chars, first char ASCII alphabetic, rest ASCII alphanumeric or `-`/`_`.
/// See <https://github.com/rust-lang/docs.rs/blob/main/crates/lib/crates_io_validation/src/lib.rs>
fn is_valid_crate_name(s: &str) -> bool {
    const MAX_NAME_LENGTH: usize = 64;
    if s.is_empty() || s.chars().count() > MAX_NAME_LENGTH {
        return false;
    }
    let mut chars = s.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !first.is_ascii_alphabetic() {
        return false;
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

fn is_valid_ident(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bare_crate() {
        let s = ItemSpec::parse("serde").unwrap();
        assert_eq!(s.crate_name, "serde");
        assert_eq!(s.version, "latest");
        assert!(s.path.is_empty());
    }

    #[test]
    fn crate_with_version() {
        let s = ItemSpec::parse("serde@1.0.200").unwrap();
        assert_eq!(s.version, "1.0.200");
    }

    #[test]
    fn crate_with_path() {
        let s = ItemSpec::parse("serde::de::Deserialize").unwrap();
        assert_eq!(s.crate_name, "serde");
        assert_eq!(s.version, "latest");
        assert_eq!(s.path, vec!["de", "Deserialize"]);
    }

    #[test]
    fn crate_with_version_and_path() {
        let s = ItemSpec::parse("anyhow@~1::Error").unwrap();
        assert_eq!(s.crate_name, "anyhow");
        assert_eq!(s.version, "~1");
        assert_eq!(s.path, vec!["Error"]);
    }

    #[test]
    fn rejects_empty() {
        assert!(ItemSpec::parse("").is_err());
    }

    #[test]
    fn rejects_bad_at() {
        assert!(ItemSpec::parse("@1.0").is_err());
        assert!(ItemSpec::parse("serde@").is_err());
    }

    #[test]
    fn rejects_invalid_crate_names() {
        assert!(ItemSpec::parse("1serde").is_err(), "leading digit");
        assert!(ItemSpec::parse("-serde").is_err(), "leading dash");
        assert!(ItemSpec::parse("_serde").is_err(), "leading underscore");
        assert!(ItemSpec::parse("ser de").is_err(), "space");
        assert!(ItemSpec::parse("ser.de").is_err(), "dot");
        assert!(ItemSpec::parse(&"a".repeat(65)).is_err(), "too long");
    }

    #[test]
    fn accepts_valid_crate_names() {
        assert!(ItemSpec::parse("serde").is_ok());
        assert!(ItemSpec::parse("serde_json").is_ok());
        assert!(ItemSpec::parse("tracing-subscriber").is_ok());
        assert!(ItemSpec::parse("a").is_ok());
        assert!(ItemSpec::parse(&"a".repeat(64)).is_ok(), "max length");
    }
}
