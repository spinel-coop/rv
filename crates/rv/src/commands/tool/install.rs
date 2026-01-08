use owo_colors::OwoColorize;
use url::Url;

use crate::{config::Config, http_client::rv_http_client};

const GEM_COOP: &str = "https://gem.coop/";

#[derive(Debug, thiserror::Error, miette::Diagnostic)]
pub enum Error {
    #[error("{0} is not a valid URL")]
    BadUrl(String),
    #[error(transparent)]
    HttpError(#[from] reqwest::Error),
    #[error("The requested gem {gem} was not found on the RubyGems server {gem_server}")]
    GemNotFound { gem: String, gem_server: Url },
}

type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
struct InnerArgs {
    /// Gemserver to install from.
    gem_server: Url,
    /// Gem to install as a tool.
    gem: String,
}

impl InnerArgs {
    fn new(gem: String) -> Result<Self> {
        let out = Self {
            gem_server: GEM_COOP
                .parse()
                .map_err(|_| Error::BadUrl(GEM_COOP.to_owned()))?,
            gem,
        };
        Ok(out)
    }
}

pub async fn install(_config: &Config, gem: String) -> Result<()> {
    let args = InnerArgs::new(gem)?;
    let client = rv_http_client("install")?;
    let mut url = args.gem_server.clone();
    url.set_path(&format!("info/{}", args.gem));
    let index_body = client
        .get(url)
        .send()
        .await?
        .error_for_status()?
        .text()
        .await?;
    let versions: Vec<_> = index_body.lines().collect();
    if versions.is_empty() {
        return Err(Error::GemNotFound {
            gem: args.gem,
            gem_server: args.gem_server,
        });
    }
    tracing::debug!("Found {} versions for the gem", versions.len());
    println!("{}: install {}", "todo".yellow(), args.gem);
    Ok(())
}
