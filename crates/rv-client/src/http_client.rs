use crate::tls::Certificates;
use reqwest::Client;

pub fn rv_http_client(command: &'static str) -> Result<Client, reqwest::Error> {
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
    let client_builder = if let Some(custom_certs) = custom_certs {
        client_builder.tls_certs_only(custom_certs)
    } else {
        client_builder
    };

    let client = client_builder.build()?;

    Ok(client)
}
