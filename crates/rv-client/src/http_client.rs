use crate::tls::Certificates;
use reqwest::Client;

pub fn rv_http_client_builder(command: &'static str) -> reqwest::ClientBuilder {
    use reqwest::header;
    let mut headers = header::HeaderMap::new();

    // Load custom CA certificates from `SSL_CERT_FILE` and `SSL_CERT_DIR`.
    let custom_certs = Certificates::from_env().map(|certs| certs.to_reqwest_certs());

    headers.insert(
        "X-RV-PLATFORM",
        header::HeaderValue::from_static(current_platform::CURRENT_PLATFORM),
    );
    headers.insert("X-RV-COMMAND", header::HeaderValue::from_static(command));

    let client_builder = reqwest::Client::builder()
        .user_agent(format!("rv-{}", env!("CARGO_PKG_VERSION")))
        .default_headers(headers)
        .tls_backend_rustls();

    // Configure the certificate source.
    //
    // `SSL_CERT_FILE` and `SSL_CERT_DIR` override the default certificate source when they
    // contain valid certificates.
    let Some(custom_certs) = custom_certs else {
        return add_webpki_root_certs(client_builder);
    };

    client_builder.tls_certs_only(custom_certs)
}

pub fn rv_http_client(command: &'static str) -> Result<Client, reqwest::Error> {
    rv_http_client_builder(command).build()
}

fn add_webpki_root_certs(mut builder: reqwest::ClientBuilder) -> reqwest::ClientBuilder {
    for cert in webpki_root_certs::TLS_SERVER_ROOT_CERTS {
        match reqwest::Certificate::from_der(cert) {
            Ok(cert) => builder = builder.add_root_certificate(cert),
            Err(err) => tracing::debug!("Failed to add bundled webpki root cert: {err}"),
        }
    }
    builder
}
