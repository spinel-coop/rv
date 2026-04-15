use crate::http_client::rv_http_client;
use reqwest::{Client, Response};
use url::Url;

pub struct RegistryClient {
    url: Url,
    client: Client,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Invalid remote URL")]
    BadRemote {
        remote: String,
        err: url::ParseError,
    },
    #[error(transparent)]
    Reqwest(#[from] reqwest::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

impl RegistryClient {
    pub fn new(remote: &str, command: &'static str) -> Result<Self> {
        let url = Url::parse(remote).map_err(|err| Error::BadRemote {
            remote: remote.to_owned(),
            err,
        })?;

        Ok(Self {
            url,
            client: rv_http_client(command)?,
        })
    }

    pub fn package_url(&self, gem: &str) -> Url {
        self.url
            .join(format!("gems/{}", gem).as_str())
            .expect("guaranteed to succeed")
    }

    /// Make a single HTTP get request
    pub async fn get(&self, remote_path: impl AsRef<str>) -> Result<Response> {
        Ok(self
            .client
            .get(remote_path.as_ref())
            .send()
            .await?
            .error_for_status()?)
    }
}
