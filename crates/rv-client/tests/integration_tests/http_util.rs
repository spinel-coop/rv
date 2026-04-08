// These test helpers are adapted from
// https://github.com/astral-sh/uv/blob/7924ba5b1419345dc5b9a9a16e6bcba2b59a41a6/crates/uv-client/tests/it/http_util.rs

use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::{Context, Result};
use futures::future;
use http_body_util::{BodyExt, Full};
use hyper::body::{Bytes, Incoming};
use hyper::header::USER_AGENT;
use hyper::service::service_fn;
use hyper::{Request, Response};
use hyper_util::rt::{TokioExecutor, TokioIo};
use hyper_util::server::conn::auto::Builder;
use rcgen::{
    BasicConstraints, Certificate, CertificateParams, DnType, ExtendedKeyUsagePurpose, IsCa,
    Issuer, KeyPair, KeyUsagePurpose, SanType, date_time_ymd,
};
use rustls::ServerConfig;
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use tokio::net::TcpListener;
use tokio::task::JoinHandle;
use tokio_rustls::TlsAcceptor;

/// An issued certificate, together with the subject keypair.
#[derive(Debug)]
pub(crate) struct SelfSigned {
    /// An issued certificate.
    pub public: Certificate,
    /// The certificate's subject signing key.
    pub private: KeyPair,
}

/// Generates a self-signed root CA, server certificate
/// There are no intermediate certs generated as part of this function.
/// The server certificate is for `rv.test` issued by this CA.
///
/// Use sparingly as generation of these certs is a very slow operation.
pub(crate) fn generate_self_signed_certs_with_ca() -> Result<(SelfSigned, SelfSigned)> {
    // Generate the CA
    let mut ca_params = CertificateParams::default();
    ca_params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained); // root cert
    ca_params.not_before = date_time_ymd(1975, 1, 1);
    ca_params.not_after = date_time_ymd(4096, 1, 1);
    ca_params.key_usages.push(KeyUsagePurpose::DigitalSignature);
    ca_params.key_usages.push(KeyUsagePurpose::KeyEncipherment);
    ca_params.key_usages.push(KeyUsagePurpose::KeyCertSign);
    ca_params
        .distinguished_name
        .push(DnType::OrganizationName, "Developer Certificate");
    ca_params
        .distinguished_name
        .push(DnType::CommonName, "Puma-dev CA");

    let ca_private_key = KeyPair::generate()?;
    let ca_public_cert = ca_params.self_signed(&ca_private_key)?;
    let ca_cert_issuer = Issuer::new(ca_params, &ca_private_key);

    // Generate server cert issued by this CA
    let mut server_params = CertificateParams::default();
    server_params.is_ca = IsCa::NoCa;
    server_params.not_before = date_time_ymd(1975, 1, 1);
    server_params.not_after = date_time_ymd(4096, 1, 1);
    server_params.use_authority_key_identifier_extension = true;
    server_params
        .key_usages
        .push(KeyUsagePurpose::DigitalSignature);
    server_params
        .key_usages
        .push(KeyUsagePurpose::KeyEncipherment);
    server_params
        .extended_key_usages
        .push(ExtendedKeyUsagePurpose::ServerAuth);
    server_params
        .distinguished_name
        .push(DnType::OrganizationName, "Puma-dev Signed");
    server_params
        .distinguished_name
        .push(DnType::CommonName, "rv.test");
    server_params
        .subject_alt_names
        .push(SanType::IpAddress("127.0.0.1".parse()?));
    server_params
        .subject_alt_names
        .push(SanType::DnsName("rv.test".try_into()?));

    let server_private_key = KeyPair::generate()?;
    let server_public_cert = server_params.signed_by(&server_private_key, &ca_cert_issuer)?;

    let ca_self_signed = SelfSigned {
        public: ca_public_cert,
        private: ca_private_key,
    };
    let server_self_signed = SelfSigned {
        public: server_public_cert,
        private: server_private_key,
    };

    Ok((ca_self_signed, server_self_signed))
}

pub(crate) struct TestServerBuilder<'a> {
    // Server certificate
    server_cert: &'a SelfSigned,
}

impl<'a> TestServerBuilder<'a> {
    /// Starts the HTTP(S) server
    pub(crate) async fn start(self) -> Result<(JoinHandle<Result<()>>, SocketAddr)> {
        // Set up the TCP listener on a random available port
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let addr = listener.local_addr()?;

        // Prepare Server Cert and KeyPair
        let server_key = PrivateKeyDer::try_from(self.server_cert.private.serialize_der()).unwrap();
        let server_cert = vec![CertificateDer::from(self.server_cert.public.der().to_vec())];

        let mut tls_config = ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(server_cert, server_key)?;
        tls_config.alpn_protocols = vec![b"http/1.1".to_vec(), b"http/1.0".to_vec()];
        let tls_acceptor = TlsAcceptor::from(Arc::new(tls_config));

        // Setup Response Handler
        let svc_fn = |req: Request<Incoming>| {
            // Get User Agent Header and send it back in the response
            let user_agent = req
                .headers()
                .get(USER_AGENT)
                .and_then(|v| v.to_str().ok())
                .map(ToString::to_string)
                .unwrap_or_default(); // Empty Default
            let response_content = Full::new(Bytes::from(user_agent))
                .map_err(|_| unreachable!())
                .boxed();
            future::ok::<_, hyper::Error>(Response::new(response_content))
        };

        // Spawn the server loop in a background task
        let server_task = tokio::spawn(async move {
            let svc = service_fn(move |req: Request<Incoming>| svc_fn(req));

            let (tcp_stream, _remote_addr) = listener
                .accept()
                .await
                .context("Failed to accept TCP connection")?;

            // Start Server (not wrapped in loop {} since we want a single response server)
            // If we want server to accept multiple connections, we can wrap it in loop {}
            // but we'll need to ensure to handle termination signals in the tests otherwise
            // it may never stop.
            let tls_stream = tls_acceptor
                .accept(tcp_stream)
                .await
                .context("Failed to accept TLS connection")?;
            let socket = TokioIo::new(tls_stream);
            tokio::task::spawn(async move {
                Builder::new(TokioExecutor::new())
                    .serve_connection(socket, svc)
                    .await
                    .expect("HTTPS Server Started");
            });

            Ok(())
        });

        Ok((server_task, addr))
    }
}

/// Single Request HTTPS server that echoes the User Agent Header.
pub(crate) async fn start_https_user_agent_server(
    server_cert: &SelfSigned,
) -> Result<(JoinHandle<Result<()>>, SocketAddr)> {
    TestServerBuilder { server_cert }.start().await
}
