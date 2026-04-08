// This whole module is mostly copied from
// https://github.com/astral-sh/uv/blob/466a0f0df262ac804069d3b3e90c7a4d776806f9/crates/uv-client/src/tls.rs

use std::env;
use std::io;
use std::path::{Path, PathBuf};

use itertools::Itertools;
use reqwest::Certificate;

// We're only importing some functionality to load certificates from this crate but nothing actually
// platform specific or related to loading system certificates, so kind of misleading to import it
// from th rustls-native-certs crate, but still convenient.
use rustls_native_certs::{CertificateResult, load_certs_from_paths};
use rustls_pki_types::CertificateDer;

use owo_colors::OwoColorize;
use tracing::debug;
use tracing::warn;

const ENV_CERT_FILE: &str = "SSL_CERT_FILE";
const ENV_CERT_DIR: &str = "SSL_CERT_DIR";

/// A collection of TLS certificates in DER form.
#[derive(Debug, Clone, Default)]
pub(crate) struct Certificates(Vec<CertificateDer<'static>>);

impl Certificates {
    /// Load custom CA certificates from `SSL_CERT_FILE` and `SSL_CERT_DIR` environment variables.
    ///
    /// Returns `None` if neither variable is set, if the referenced files or directories are
    /// missing or inaccessible, or if no valid certificates are found (with a warning in each
    /// case). Delegates path loading to [`rustls_native_certs::load_certs_from_paths`].
    pub(crate) fn from_env() -> Option<Self> {
        let mut certs = Self::default();
        let mut has_source = false;

        if let Some(ssl_cert_file) = env::var_os(ENV_CERT_FILE)
            && let Some(file_certs) = Self::from_ssl_cert_file(&ssl_cert_file)
        {
            has_source = true;
            certs.merge(file_certs);
        }

        if let Some(ssl_cert_dir) = env::var_os(ENV_CERT_DIR)
            && let Some(dir_certs) = Self::from_ssl_cert_dir(&ssl_cert_dir)
        {
            has_source = true;
            certs.merge(dir_certs);
        }

        if has_source { Some(certs) } else { None }
    }

    /// Load certificates from the value of `SSL_CERT_FILE`.
    ///
    /// Returns `None` if the value is empty, the path does not refer to an accessible file,
    /// or the file contains no valid certificates.
    fn from_ssl_cert_file(ssl_cert_file: &std::ffi::OsStr) -> Option<Self> {
        if ssl_cert_file.is_empty() {
            return None;
        }

        let file = PathBuf::from(ssl_cert_file);
        match file.metadata() {
            Ok(metadata) if metadata.is_file() => {
                let result = Self::from_paths(Some(&file), None);
                for err in &result.errors {
                    warn!(
                        "Failed to load `SSL_CERT_FILE` ({}): {err}",
                        file.to_string_lossy().to_string().cyan()
                    );
                }
                let certs = Self::from(result);
                if certs.0.is_empty() {
                    warn!(
                        "Ignoring `SSL_CERT_FILE`. No certificates found in: {}.",
                        file.to_string_lossy().to_string().cyan()
                    );
                    return None;
                }
                Some(certs)
            }
            Ok(_) => {
                warn!(
                    "Ignoring invalid `SSL_CERT_FILE`. Path is not a file: {}.",
                    file.to_string_lossy().to_string().cyan()
                );
                None
            }
            Err(err) if err.kind() == io::ErrorKind::NotFound => {
                warn!(
                    "Ignoring invalid `SSL_CERT_FILE`. Path does not exist: {}.",
                    file.to_string_lossy().to_string().cyan()
                );
                None
            }
            Err(err) => {
                warn!(
                    "Ignoring invalid `SSL_CERT_FILE`. Path is not accessible: {} ({err}).",
                    file.to_string_lossy().to_string().cyan()
                );
                None
            }
        }
    }

    /// Load certificates from the value of `SSL_CERT_DIR`.
    ///
    /// The value may include multiple entries, separated by a platform-specific delimiter (`:` on
    /// Unix, `;` on Windows).
    ///
    /// Returns `None` if the value is empty, no listed directories exist, or no valid
    /// certificates are found.
    fn from_ssl_cert_dir(ssl_cert_dir: &std::ffi::OsStr) -> Option<Self> {
        if ssl_cert_dir.is_empty() {
            return None;
        }

        let (existing, missing): (Vec<_>, Vec<_>) =
            env::split_paths(ssl_cert_dir).partition(|path| path.exists());

        if existing.is_empty() {
            let end_note = if missing.len() == 1 {
                "The directory does not exist."
            } else {
                "The entries do not exist."
            };
            warn!(
                "Ignoring invalid `SSL_CERT_DIR`. {end_note}: {}.",
                missing
                    .iter()
                    .map(|s| s.to_string_lossy().to_string())
                    .join(", ")
                    .cyan()
            );
            return None;
        }

        if !missing.is_empty() {
            let end_note = if missing.len() == 1 {
                "The following directory does not exist:"
            } else {
                "The following entries do not exist:"
            };
            warn!(
                "Invalid entries in `SSL_CERT_DIR`. {end_note}: {}.",
                missing
                    .iter()
                    .map(|s| s.to_string_lossy().to_string())
                    .join(", ")
                    .cyan()
            );
        }

        let mut certs = Self::default();
        for dir in &existing {
            let result = Self::from_paths(None, Some(dir));
            for err in &result.errors {
                warn!(
                    "Failed to load `SSL_CERT_DIR` ({}): {err}",
                    dir.to_string_lossy().to_string().cyan()
                );
            }
            certs.merge(Self::from(result));
        }

        if certs.0.is_empty() {
            warn!(
                "Ignoring `SSL_CERT_DIR`. No certificates found in: {}.",
                existing
                    .iter()
                    .map(|s| s.to_string_lossy().to_string())
                    .join(", ")
                    .cyan()
            );
            return None;
        }

        Some(certs)
    }

    /// Load certificates from explicit file and directory paths.
    fn from_paths(file: Option<&Path>, dir: Option<&Path>) -> CertificateResult {
        load_certs_from_paths(file, dir)
    }

    /// Remove duplicate certificates, sorting by DER bytes.
    fn dedup(&mut self) {
        self.0
            .sort_unstable_by(|left, right| left.as_ref().cmp(right.as_ref()));
        self.0.dedup();
    }

    /// Merge another set of certificates into this one.
    ///
    /// After merging, duplicates are removed.
    fn merge(&mut self, other: Self) {
        self.0.extend(other.0);
        self.dedup();
    }

    /// Convert certificates to reqwest [`Certificate`] objects.
    pub(crate) fn to_reqwest_certs(&self) -> Vec<Certificate> {
        self.0
            .iter()
            // `Certificate::from_der` returns a `Result` for backend compatibility, but these
            // certificates come from `rustls-native-certs` and are already validated DER certs.
            // In our rustls-based client configuration this conversion is expected to succeed.
            .filter_map(|cert| match Certificate::from_der(cert) {
                Ok(certificate) => Some(certificate),
                Err(err) => {
                    debug!("Failed to convert DER certificate to reqwest certificate: {err}");
                    None
                }
            })
            .collect()
    }

    /// Iterate over raw DER certificates.
    #[cfg(test)]
    fn iter(&self) -> impl Iterator<Item = &CertificateDer<'static>> {
        self.0.iter()
    }
}

impl From<CertificateResult> for Certificates {
    fn from(result: CertificateResult) -> Self {
        Self(result.certs)
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;

    use super::*;

    fn generate_cert_pem() -> String {
        let cert = rcgen::generate_simple_self_signed(vec!["localhost".to_string()]).unwrap();
        cert.cert.pem()
    }

    #[test]
    fn test_from_ssl_cert_file_nonexistent_returns_none() {
        let dir = tempfile::tempdir().unwrap();
        let missing_file = dir.path().join("missing.pem");

        let certs = Certificates::from_ssl_cert_file(missing_file.as_os_str());
        assert!(certs.is_none());
    }

    #[test]
    fn test_from_ssl_cert_file_empty_value_returns_none() {
        let certs = Certificates::from_ssl_cert_file(OsString::new().as_os_str());
        assert!(certs.is_none());
    }

    #[test]
    fn test_from_ssl_cert_file_no_valid_certs_returns_none() {
        let dir = tempfile::tempdir().unwrap();
        let cert_path = dir.path().join("empty.pem");
        fs_err::write(&cert_path, "not a certificate").unwrap();

        let certs = Certificates::from_ssl_cert_file(cert_path.as_os_str());
        assert!(certs.is_none());
    }

    #[test]
    fn test_from_ssl_cert_dir_empty_value_returns_none() {
        let certs = Certificates::from_ssl_cert_dir(OsString::new().as_os_str());
        assert!(certs.is_none());
    }

    #[test]
    fn test_from_ssl_cert_dir_nonexistent_returns_none() {
        let dir = tempfile::tempdir().unwrap();
        let missing_dir = dir.path().join("missing-dir");
        let cert_dirs = std::env::join_paths([&missing_dir]).unwrap();

        let certs = Certificates::from_ssl_cert_dir(cert_dirs.as_os_str());
        assert!(certs.is_none());
    }

    #[test]
    fn test_from_ssl_cert_dir_empty_existing_returns_none() {
        let dir = tempfile::tempdir().unwrap();
        let cert_dirs = std::env::join_paths([dir.path()]).unwrap();

        let certs = Certificates::from_ssl_cert_dir(cert_dirs.as_os_str());
        assert!(certs.is_none());
    }

    #[test]
    fn test_merge_deduplicates() {
        let dir = tempfile::tempdir().unwrap();
        let cert_path = dir.path().join("cert.pem");
        fs_err::write(&cert_path, generate_cert_pem()).unwrap();

        let first = Certificates::from_ssl_cert_file(cert_path.as_os_str()).unwrap();
        let second = Certificates::from_ssl_cert_file(cert_path.as_os_str()).unwrap();

        let mut merged = first;
        merged.merge(second);

        assert_eq!(merged.iter().count(), 1);
    }
}
