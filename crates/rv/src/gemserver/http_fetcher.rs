use reqwest::Response;
use reqwest::header::HeaderMap;
use rv_client::http_client::rv_http_client;

#[derive(Clone)]
pub struct HttpFetcher {
    client: reqwest::Client,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Reqwest(#[from] reqwest::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

impl HttpFetcher {
    pub fn new(command: &'static str) -> Result<Self> {
        Ok(Self {
            client: rv_http_client(command)?,
        })
    }

    /// Make a single HTTP call without retry logic
    pub async fn call(&self, remote_path: &str, headers: HeaderMap) -> Result<Response> {
        let request = self.client.get(remote_path).headers(headers);

        Ok(request.send().await?.error_for_status()?)
    }
}
