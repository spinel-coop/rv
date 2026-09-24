use camino::Utf8PathBuf;
use ignore::WalkBuilder;
use std::path::Path;

/// Custom ignore file honored by `rv fmt`, alongside `.gitignore`.
pub(crate) const RUBYFMT_IGNORE_FILE: &str = ".rubyfmtignore";

/// Extensions that hold Ruby source.
const RUBY_EXTENSIONS: &[&str] = &[
    "rb", "rake", "gemspec", "ru", "rbi", "builder", "jbuilder", "thor", "podspec", "opal",
];

/// Filenames that are Ruby source by convention despite having no useful extension.
const RUBY_FILENAMES: &[&str] = &[
    "Appfile",
    "Appraisals",
    "Berksfile",
    "Brewfile",
    "Capfile",
    "Cheffile",
    "Dangerfile",
    "Deliverfile",
    "Fastfile",
    "Gemfile",
    "Guardfile",
    "Matchfile",
    "Podfile",
    "Puppetfile",
    "Rakefile",
    "Scanfile",
    "Snapfile",
    "Steepfile",
    "Thorfile",
    "Vagrantfile",
];

/// Whether a discovered path looks like Ruby source, by extension (`.rb`,
/// `.rake`, `config.ru`, `*.gemspec`, ...) or by a conventional filename
/// (`Rakefile`, `Gemfile`, ...).
pub(crate) fn is_ruby_file(path: &Path) -> bool {
    if path
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| RUBY_EXTENSIONS.contains(&ext))
    {
        return true;
    }

    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| RUBY_FILENAMES.contains(&name))
}

#[derive(Debug)]
pub struct DiscoveryError {
    pub(crate) path: String,
}

impl std::fmt::Display for DiscoveryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Discovery error for path: {}", self.path)
    }
}

impl std::error::Error for DiscoveryError {}

#[derive(Debug)]
pub(crate) struct DiscoverableFileFailure {
    pub(crate) path: String,
    pub(crate) error: std::io::Error,
}

impl From<DiscoveryError> for DiscoverableFileFailure {
    fn from(err: DiscoveryError) -> Self {
        DiscoverableFileFailure {
            path: err.path,
            error: std::io::Error::new(std::io::ErrorKind::NotFound, "path not found"),
        }
    }
}

#[derive(Debug, Default)]
pub(crate) struct DiscoveryResult {
    pub(crate) files: Vec<(Utf8PathBuf, Vec<u8>)>,
    pub(crate) failures: Vec<DiscoverableFileFailure>,
}

pub(crate) fn expand_paths(paths: &[String]) -> Result<Vec<String>, DiscoveryError> {
    let mut expanded = Vec::with_capacity(paths.len());

    for path in paths {
        if let Some(file_path) = path.strip_prefix('@') {
            let content = std::fs::read_to_string(file_path).map_err(|_| DiscoveryError {
                path: file_path.to_string(),
            })?;
            for line in content.lines() {
                let trimmed = line.trim();
                if !trimmed.is_empty() {
                    expanded.push(trimmed.to_string());
                }
            }
        } else {
            expanded.push(path.clone());
        }
    }

    Ok(expanded)
}

/// Walk `include_paths`, reading every Ruby source file found.
///
/// `custom_ignore_filename` names an extra ignore file to honor alongside
/// `.gitignore`; callers pass [`RUBYFMT_IGNORE_FILE`] to honor the formatter's
/// custom ignore file, or `None` to only honor `.gitignore`.
///
/// A path that names an existing file is read regardless of its name — naming
/// a file explicitly is taken as intent to process it. Directory walks are
/// filtered by [`is_ruby_file`].
pub(crate) fn discover_rb_files(
    include_paths: &[String],
    include_gitignored: bool,
    custom_ignore_filename: Option<&str>,
) -> Result<DiscoveryResult, DiscoveryError> {
    let mut result = DiscoveryResult::default();

    for path in include_paths {
        let root = Path::new(path);
        let named_explicitly = root.is_file();

        let mut builder = WalkBuilder::new(root);
        builder.git_ignore(!include_gitignored);
        if let Some(ignore_filename) = custom_ignore_filename {
            builder.add_custom_ignore_filename(ignore_filename);
        }

        for entry in builder.build() {
            let dir_entry = match entry {
                Ok(dir_entry) => dir_entry,
                Err(e) => {
                    result.failures.push(DiscoverableFileFailure {
                        path: path.clone(),
                        error: std::io::Error::other(e.to_string()),
                    });
                    continue;
                }
            };

            let entry_path = dir_entry.path();
            if !dir_entry.file_type().is_some_and(|ft| ft.is_file()) {
                continue;
            }
            if !named_explicitly && !is_ruby_file(entry_path) {
                continue;
            }

            let utf8_path = match Utf8PathBuf::from_path_buf(entry_path.to_path_buf()) {
                Ok(utf8_path) => utf8_path,
                Err(non_utf8) => {
                    result.failures.push(DiscoverableFileFailure {
                        path: non_utf8.to_string_lossy().into_owned(),
                        error: std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            "path is not valid UTF-8",
                        ),
                    });
                    continue;
                }
            };

            match std::fs::read(&utf8_path) {
                Ok(bytes) => result.files.push((utf8_path, bytes)),
                Err(e) => result.failures.push(DiscoverableFileFailure {
                    path: utf8_path.into_string(),
                    error: e,
                }),
            }
        }
    }

    Ok(result)
}

#[cfg(test)]
mod tests;
