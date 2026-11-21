//! Which build of a ruby is installed.
//!
//! A ruby can be rebuilt without its version changing, say to pick up a fix in a library
//! it is built with, and nothing in its files tells one build from another. So
//! `rv ruby install` writes down which build it installed, next to the ruby.

use camino::Utf8Path;
use serde::{Deserialize, Serialize};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

const FILE_NAME: &str = ".rv-build";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BuildMarker {
    /// When the installed archive was built, like `20261014`
    pub build: String,

    /// SHA-256 checksum of the archive, when the download came with one
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,

    /// Where the archive was downloaded from
    pub url: String,

    pub installed_at: String,
}

impl BuildMarker {
    pub fn new(build: String, sha256: Option<String>, url: String) -> Self {
        let installed_at = OffsetDateTime::now_utc()
            .format(&Rfc3339)
            .unwrap_or_default();

        Self {
            build,
            sha256,
            url,
            installed_at,
        }
    }

    /// Reads the marker of the ruby installed in `ruby_dir`. A ruby that rv didn't install,
    /// or installed before it kept track of builds, has none.
    pub fn read(ruby_dir: &Utf8Path) -> Option<Self> {
        let content = fs_err::read_to_string(ruby_dir.join(FILE_NAME)).ok()?;
        serde_json::from_str(&content).ok()
    }

    pub fn write(&self, ruby_dir: &Utf8Path) -> std::io::Result<()> {
        let content = serde_json::to_string(self).map_err(std::io::Error::other)?;
        fs_err::write(ruby_dir.join(FILE_NAME), content + "\n")
    }

    /// Has the installed build been replaced by `build`?
    pub fn is_older_than(&self, build: &str) -> bool {
        replaces(build, &self.build)
    }
}

/// Is `build` a later build than `other`?
pub fn replaces(build: &str, other: &str) -> bool {
    build_order(build) > build_order(other)
}

/// Builds are dated. A later build on the same day counts up after a `-` or a `.`,
/// like `20261014-2`.
fn build_order(build: &str) -> (&str, u32) {
    match build.split_once(['-', '.']) {
        Some((date, count)) => (date, count.parse().unwrap_or(0)),
        None => (build, 0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use assert_fs::TempDir;
    use camino::Utf8PathBuf;

    fn marker(build: &str) -> BuildMarker {
        BuildMarker::new(
            build.to_owned(),
            Some("5f1c3d0a9b7e4c2d8a6f0e1b3c5d7f9a2b4c6e8f0a1c3e5d7b9f1a3c5e7d9b0f".to_owned()),
            "https://rv.dev/ruby/3.4.11/20261014/ruby-3.4.11.macos-aarch64.tar.gz".to_owned(),
        )
    }

    #[test]
    fn test_reads_back_what_it_wrote() {
        let temp_dir = TempDir::new().unwrap();
        let ruby_dir = Utf8PathBuf::from(temp_dir.path().to_str().unwrap());
        let written = marker("20261014");

        written.write(&ruby_dir).unwrap();

        assert_eq!(BuildMarker::read(&ruby_dir), Some(written));
    }

    #[test]
    fn test_marker_file_contents() {
        let temp_dir = TempDir::new().unwrap();
        let ruby_dir = Utf8PathBuf::from(temp_dir.path().to_str().unwrap());
        let mut written = marker("20261014");
        written.installed_at = "2026-10-22T09:30:00Z".to_owned();

        written.write(&ruby_dir).unwrap();

        assert_eq!(
            fs_err::read_to_string(ruby_dir.join(".rv-build")).unwrap(),
            "{\"build\":\"20261014\",\
             \"sha256\":\"5f1c3d0a9b7e4c2d8a6f0e1b3c5d7f9a2b4c6e8f0a1c3e5d7b9f1a3c5e7d9b0f\",\
             \"url\":\"https://rv.dev/ruby/3.4.11/20261014/ruby-3.4.11.macos-aarch64.tar.gz\",\
             \"installed_at\":\"2026-10-22T09:30:00Z\"}\n"
        );
    }

    #[test]
    fn test_installed_at_is_a_timestamp() {
        let installed_at = marker("20261014").installed_at;

        assert!(OffsetDateTime::parse(&installed_at, &Rfc3339).is_ok());
    }

    #[test]
    fn test_a_ruby_without_a_marker() {
        let temp_dir = TempDir::new().unwrap();
        let ruby_dir = Utf8PathBuf::from(temp_dir.path().to_str().unwrap());

        assert_eq!(BuildMarker::read(&ruby_dir), None);

        fs_err::write(ruby_dir.join(".rv-build"), "not json").unwrap();
        assert_eq!(BuildMarker::read(&ruby_dir), None);
    }

    #[test]
    fn test_is_older_than_a_later_date() {
        assert!(marker("20261014").is_older_than("20261018"));
        assert!(!marker("20261018").is_older_than("20261014"));
        assert!(!marker("20261014").is_older_than("20261014"));
    }

    #[test]
    fn test_is_older_than_a_later_build_on_the_same_day() {
        assert!(marker("20261014").is_older_than("20261014-2"));
        assert!(marker("20261014-2").is_older_than("20261014-10"));
        assert!(marker("20261018").is_older_than("20261018.2"));
        assert!(!marker("20261014-2").is_older_than("20261014"));
        assert!(marker("20261014-9").is_older_than("20261015"));
    }
}
