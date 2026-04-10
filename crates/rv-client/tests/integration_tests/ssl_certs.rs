// These tests are adapted from
// https://github.com/astral-sh/uv/blob/7924ba5b1419345dc5b9a9a16e6bcba2b59a41a6/crates/uv-client/tests/it/ssl_certs.rs

use std::io::Write;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};

use anyhow::Result;
use temp_env::async_with_vars;
use tempfile::{NamedTempFile, TempDir};
use url::Url;

use crate::http_util::{
    SelfSigned, generate_self_signed_certs_with_ca, start_https_user_agent_server,
};

/// A self-signed CA together with a server certificate and a client certificate
/// it has issued.  Every [`TestCertificate`] is an independent trust domain.
struct TestCertificate {
    _temp_dir: TempDir,
    /// The CA certificate (root of trust).
    ca: SelfSigned,
    /// A server certificate signed by [`ca`](Self::ca).
    server: SelfSigned,
    /// Path to the CA public cert PEM — the file you put in `SSL_CERT_FILE` to
    /// trust this certificate family.
    trust_path: PathBuf,
}

impl TestCertificate {
    /// Generate a fresh CA, server cert, and client cert, persisting the
    /// relevant PEM files to a temporary directory.
    fn new() -> Result<Self> {
        let cert_dir = std::env::temp_dir()
            .canonicalize()
            .expect("failed to canonicalize temp dir")
            .join("rv")
            .join("tests")
            .join("certs");

        fs_err::create_dir_all(&cert_dir)?;
        let temp_dir = TempDir::new_in(cert_dir)?;

        let (ca, server) = generate_self_signed_certs_with_ca()?;

        let trust_path = temp_dir.path().join("ca.pem");
        fs_err::write(&trust_path, ca.public.pem())?;

        Ok(Self {
            _temp_dir: temp_dir,
            ca,
            server,
            trust_path,
        })
    }

    /// Write a CA + server PEM bundle to a [`NamedTempFile`].
    fn write_bundle_pem(&self) -> NamedTempFile {
        let mut file = NamedTempFile::new().unwrap();
        write!(
            file,
            "{}\n{}",
            self.ca.public.pem(),
            self.server.public.pem()
        )
        .unwrap();
        file
    }

    /// Write the CA public PEM into a fresh temporary directory, returning it.
    fn ca_pem_dir(&self) -> TempDir {
        self.ca_pem_dir_as("ca.pem")
    }

    /// Write the CA public PEM with a custom filename into a fresh temporary
    /// directory, returning it.
    fn ca_pem_dir_as(&self, filename: &str) -> TempDir {
        let dir = TempDir::new().unwrap();
        fs_err::write(dir.path().join(filename), self.ca.public.pem()).unwrap();
        dir
    }

    /// Write a CA + server PEM bundle into a fresh temporary directory,
    /// returning it.
    fn bundle_pem_dir(&self) -> TempDir {
        let dir = TempDir::new().unwrap();
        fs_err::write(
            dir.path().join("bundle.pem"),
            format!("{}\n{}", self.ca.public.pem(), self.server.public.pem()),
        )
        .unwrap();
        dir
    }
}

/// Client-side configuration builder.  Collects environment variable overrides
/// and provides terminal assertion methods that start a server, send a request,
/// and verify the outcome.
struct TestClient {
    overrides: Vec<(&'static str, String)>,
}

/// Create a [`TestClient`] with no environment overrides.
fn client() -> TestClient {
    TestClient {
        overrides: Vec::new(),
    }
}

impl TestClient {
    /// Set `SSL_CERT_FILE` to `path`.
    fn ssl_cert_file(self, path: &Path) -> Self {
        self.with_env("SSL_CERT_FILE", path.to_str().unwrap())
    }

    /// Set `SSL_CERT_DIR` to a single directory.
    fn ssl_cert_dir(self, path: &Path) -> Self {
        self.with_env("SSL_CERT_DIR", path.to_str().unwrap())
    }

    /// Set `SSL_CERT_DIR` to multiple directories joined with the
    /// platform-specific path separator.
    fn ssl_cert_dirs(self, paths: &[&Path]) -> Self {
        let joined = std::env::join_paths(paths).unwrap();
        self.with_env("SSL_CERT_DIR", joined.to_str().unwrap())
    }

    /// Set an arbitrary environment variable.
    fn with_env(mut self, key: &'static str, value: &str) -> Self {
        self.overrides.push((key, value.to_string()));
        self
    }

    /// Assert that an HTTPS connection to `cert`'s server succeeds.
    async fn expect_https_connect_succeeds(&self, cert: &TestCertificate) {
        self.run_https(cert, |response, server_task| async move {
            assert!(
                response.is_ok(),
                "expected successful response, got: {:?}",
                response.err()
            );
            server_task.await.unwrap().unwrap();
        })
        .await;
    }

    /// Assert that an HTTPS connection to `cert`'s server fails with a TLS
    /// error on the client side.
    async fn expect_https_connect_fails(&self, cert: &TestCertificate) {
        self.run_https(cert, |response, server_task| async move {
            assert_connection_error(&response);
            // Server may or may not have errored — just ensure no panic.
            let _ = server_task.await;
        })
        .await;
    }

    /// Build the full environment variable list: clear all SSL-related
    /// variables, then apply the accumulated overrides.
    fn ssl_vars(&self) -> Vec<(&'static str, Option<&str>)> {
        let mut vars: Vec<(&'static str, Option<&str>)> =
            vec![("SSL_CERT_FILE", None), ("SSL_CERT_DIR", None)];
        vars.extend(self.overrides.iter().map(|(k, v)| (*k, Some(v.as_str()))));
        vars
    }

    /// Start an HTTPS server, send a request inside `async_with_vars`, and
    /// hand the response + server task to `check`.
    async fn run_https<F, Fut>(&self, cert: &TestCertificate, check: F)
    where
        F: FnOnce(
            Result<reqwest::Response, reqwest::Error>,
            tokio::task::JoinHandle<Result<()>>,
        ) -> Fut,
        Fut: std::future::Future<Output = ()>,
    {
        let vars = self.ssl_vars();
        async_with_vars(vars, async {
            let (server_task, addr) = start_https_user_agent_server(&cert.server).await.unwrap();
            let response = send_request(addr).await;
            check(response, server_task).await;
        })
        .await;
    }
}

/// Send a GET request to the given server address using a fresh registry client.
async fn send_request(addr: SocketAddr) -> Result<reqwest::Response, reqwest::Error> {
    let url = &format!("https://{addr}");
    send_request_to(url).await
}

/// Send a GET request to an arbitrary URL using a fresh registry client.
async fn send_request_to(url: &str) -> Result<reqwest::Response, reqwest::Error> {
    let client = rv_client::http_client::rv_http_client("foo")?;
    client.get(Url::parse(url).unwrap()).send().await
}

/// Assert that a request result is a TLS connection error.
fn assert_connection_error(res: &Result<reqwest::Response, reqwest::Error>) {
    assert!(res.as_ref().expect_err("to fail").is_connect());
}

/// A self-signed server certificate is rejected when no custom certs are
/// configured — the bundled webpki roots don't include our test CA.
#[tokio::test]
async fn test_no_custom_certs_rejects_self_signed() -> Result<()> {
    let cert = TestCertificate::new()?;
    client().expect_https_connect_fails(&cert).await;
    Ok(())
}

/// Trusting cert A does not let you connect to a server presenting cert B.
#[tokio::test]
async fn test_ssl_cert_file_wrong_cert_rejected() -> Result<()> {
    let cert_a = TestCertificate::new()?;
    let cert_b = TestCertificate::new()?;
    client()
        .ssl_cert_file(&cert_a.trust_path)
        .expect_https_connect_fails(&cert_b)
        .await;
    Ok(())
}

// In linux, the system CA bundle may not be available and we will fail to build a connection in
// the client side due to lack of certificates, meaning the our single response server will never
// receive any requests and never stop. Because of this, we limit this test to macos since it seems
// enough to cover the fallback.
#[cfg(target_os = "macos")]
#[tokio::test]
async fn test_ssl_cert_fallbacks() -> Result<()> {
    let cert = TestCertificate::new()?;
    let dir = TempDir::new()?;

    // A nonexistent `SSL_CERT_FILE` is ignored; the client falls back to webpki
    // roots which don't include our test CA.
    let missing = dir.path().join("missing.pem");
    client()
        .ssl_cert_file(&missing)
        .expect_https_connect_fails(&cert)
        .await;

    // A nonexistent `SSL_CERT_DIR` is ignored; the client falls back to webpki
    // roots which don't include our test CA.
    let missing = dir.path().join("missing-certs");
    client()
        .ssl_cert_dir(&missing)
        .expect_https_connect_fails(&cert)
        .await;

    Ok(())
}

/// A valid `SSL_CERT_FILE` pointing to the server's CA cert is trusted.
#[tokio::test]
async fn test_ssl_cert_file_valid() -> Result<()> {
    let cert = TestCertificate::new()?;
    client()
        .ssl_cert_file(&cert.trust_path)
        .expect_https_connect_succeeds(&cert)
        .await;
    Ok(())
}

/// A PEM bundle containing multiple certificates in `SSL_CERT_FILE` is loaded.
#[tokio::test]
async fn test_ssl_cert_file_bundle() -> Result<()> {
    let cert = TestCertificate::new()?;
    let bundle = cert.write_bundle_pem();
    client()
        .ssl_cert_file(bundle.path())
        .expect_https_connect_succeeds(&cert)
        .await;
    Ok(())
}

/// Certificates from both `SSL_CERT_FILE` and `SSL_CERT_DIR` are trusted.
#[tokio::test]
async fn test_ssl_cert_file_and_dir_combined() -> Result<()> {
    let cert_a = TestCertificate::new()?;
    let cert_b = TestCertificate::new()?;

    let dir = cert_b.ca_pem_dir();
    let c = client()
        .ssl_cert_file(&cert_a.trust_path)
        .ssl_cert_dir(dir.path());
    c.expect_https_connect_succeeds(&cert_a).await;
    c.expect_https_connect_succeeds(&cert_b).await;
    Ok(())
}

/// PEM bundles inside `SSL_CERT_DIR` are loaded correctly.
#[tokio::test]
async fn test_ssl_cert_dir_bundle_files() -> Result<()> {
    let cert = TestCertificate::new()?;
    let dir = cert.bundle_pem_dir();
    client()
        .ssl_cert_dir(dir.path())
        .expect_https_connect_succeeds(&cert)
        .await;
    Ok(())
}

/// OpenSSL hash-based filenames in `SSL_CERT_DIR` are loaded correctly.
///
/// The filename `5d30f3c5.3` is not the actual OpenSSL hash of the CA cert —
/// it's an arbitrary name matching the `[hex].[digit]` pattern to verify that
/// such files are loaded from the directory.
#[tokio::test]
async fn test_ssl_cert_dir_hash_named_files() -> Result<()> {
    let cert = TestCertificate::new()?;
    let dir = cert.ca_pem_dir_as("5d30f3c5.3");
    client()
        .ssl_cert_dir(dir.path())
        .expect_https_connect_succeeds(&cert)
        .await;
    Ok(())
}

/// `SSL_CERT_DIR` supports multiple platform-separated directories. Certs are
/// split across two directories; each only has one cert, but both are trusted.
#[tokio::test]
async fn test_ssl_cert_dir_multiple_directories() -> Result<()> {
    let cert_a = TestCertificate::new()?;
    let cert_b = TestCertificate::new()?;

    let dir_a = cert_a.ca_pem_dir();
    let dir_b = cert_b.ca_pem_dir();
    let c = client().ssl_cert_dirs(&[dir_a.path(), dir_b.path()]);
    c.expect_https_connect_succeeds(&cert_a).await;
    c.expect_https_connect_succeeds(&cert_b).await;
    Ok(())
}

/// Missing entries in `SSL_CERT_DIR` do not prevent valid directories from
/// being loaded.
#[tokio::test]
async fn test_ssl_cert_dir_multiple_directories_with_missing_entry() -> Result<()> {
    let cert = TestCertificate::new()?;

    let dir = cert.ca_pem_dir();
    let scratch = TempDir::new()?;
    let missing = scratch.path().join("missing-certs");

    client()
        .ssl_cert_dirs(&[&missing, dir.path()])
        .expect_https_connect_succeeds(&cert)
        .await;
    Ok(())
}
