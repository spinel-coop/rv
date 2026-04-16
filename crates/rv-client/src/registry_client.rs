use crate::http_client::rv_http_client;
use reqwest::header::HeaderMap;
use reqwest::{Client, RequestBuilder, Response, StatusCode};
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
    #[error("{gem_name} doesn't exist on {server}")]
    NotFound { gem_name: String, server: String },
}

pub type Result<T> = std::result::Result<T, Error>;

impl RegistryClient {
    pub fn new(remote: &str, command: &'static str) -> Result<Self> {
        let mut url = Url::parse(remote).map_err(|err| Error::BadRemote {
            remote: remote.to_owned(),
            err,
        })?;

        // Add a trailing slash to the url if not already there. Otherwise, if the gemserver is
        // namespaced, the namespace is ignored because joining url's requires the base url with
        // have a trailing slash, and we join url's to construct compact index endpoints
        url.path_segments_mut()
            .expect("this url cannot be a base")
            .pop_if_empty()
            .push("");

        Ok(Self {
            url,
            client: rv_http_client(command)?,
        })
    }

    pub fn url(&self) -> String {
        self.url.to_string()
    }

    pub fn info_url(&self, gem: &str) -> Url {
        self.base_url_with_path(format!("info/{}", gem))
    }

    pub fn package_url(&self, gem: &str) -> Url {
        self.base_url_with_path(format!("gems/{}", gem))
    }

    /// Make a single HTTP get request to the /info/<gem> endoint
    pub async fn get_info(&self, gem: &str, headers: HeaderMap) -> Result<Response> {
        self.build_request(self.info_url(gem).as_str())
            .headers(headers)
            .send()
            .await?
            .error_for_status()
            .map_err(|err| {
                // If the HTTP error was 404, then return a nice error explaining that the gem
                // wasn't found.
                if err.status() == Some(StatusCode::NOT_FOUND) {
                    Error::NotFound {
                        gem_name: gem.to_owned(),
                        server: self.url(),
                    }
                } else {
                    Error::from(err) // Otherwise, keep the error as-is.
                }
            })
    }

    /// Make a single HTTP get request
    pub async fn get(&self, remote_path: impl AsRef<str>) -> Result<Response> {
        Ok(self
            .build_request(remote_path)
            .send()
            .await?
            .error_for_status()?)
    }

    fn build_request(&self, remote_path: impl AsRef<str>) -> RequestBuilder {
        self.client.get(remote_path.as_ref())
    }

    fn base_url_with_path(&self, path: String) -> Url {
        self.url.join(path.as_str()).expect("guaranteed to succeed")
    }
}
