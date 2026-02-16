//! Most of the types in this module borrow a string from their input,
//! so they have a lifetime 'i, which is short for 'input.

#[derive(Debug, Clone, Default, Eq, PartialEq, Ord, PartialOrd, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GemfileDotLock<'i> {
    /// Dependencies sourced from a Git repo.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub git: Vec<GitSection<'i>>,

    /// Dependencies sourced from a RubyGems server.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub gem: Vec<GemSection<'i>>,

    /// Dependencies sourced from a filesystem path.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub path: Vec<PathSection<'i>>,

    /// Lists every triple that Bundler has resolved and included in this lockfile.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub platforms: Vec<&'i str>,

    /// Lists every gem that this lockfile has been resolved to include
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dependencies: Vec<GemRange<'i>>,

    /// Which version of Ruby this lockfile was built with.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ruby_version: Option<&'i str>,

    /// Which version of Bundler this lockfile was built with.
    pub bundled_with: Option<&'i str>,

    /// Checksums for each dependency.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checksums: Option<Vec<Checksum<'i>>>,
}

impl GemfileDotLock<'_> {
    /// Returns the total number of gem specs from RubyGems server sources.
    pub fn gem_spec_count(&self) -> usize {
        self.gem.iter().map(|s| s.specs.len()).sum()
    }

    pub fn platform_specific_spec_count(&self) -> usize {
        self.gem
            .iter()
            .map(|s| s.platform_specific_gems().len())
            .sum::<usize>()
            + self.git.iter().map(|s| s.specs.len()).sum::<usize>()
            + self.path.iter().map(|s| s.specs.len()).sum::<usize>()
    }

    pub fn discard_installed_gems(&mut self, install_path: &camino::Utf8PathBuf) {
        self.gem
            .iter_mut()
            .for_each(|gem_section| gem_section.discard_installed_gems(install_path));

        self.gem.retain(|section| !section.specs.is_empty());
    }
}

/// Git source that gems could come from.
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GitSection<'i> {
    /// Location of the Git repo.
    pub remote: &'i str,
    /// Commit used from the Git repo.
    pub revision: &'i str,
    /// Branch used from the Git repo.
    pub branch: Option<&'i str>,
    /// Ref used from the Git repo.
    #[serde(rename = "ref")]
    pub git_ref: Option<&'i str>,
    /// Tag used from the Git repo.
    pub tag: Option<&'i str>,
    /// Includes git submodules, or not.
    /// Optional, defaults to false.
    pub submodules: bool,
    /// All gems which came from this source in particular.
    pub specs: Vec<Spec<'i>>,
}

/// Rubygems server source that gems could come from.
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GemSection<'i> {
    /// Location of the RubyGems server.
    pub remote: &'i str,
    /// All gems which came from this source in particular.
    pub specs: Vec<Spec<'i>>,
}

impl<'i> GemSection<'i> {
    pub fn platform_specific_gems(&self) -> Vec<Spec<'i>> {
        use rv_gem_types::VersionPlatform;
        use std::collections::HashMap;
        use std::str::FromStr;

        let mut by_name: HashMap<&str, Spec<'i>> = HashMap::new();
        for spec in &self.specs {
            let gem_version = spec.gem_version;

            let Ok(vp) = VersionPlatform::from_str(gem_version.version) else {
                continue;
            };

            if !vp.platform.is_local() {
                continue;
            }

            if let Some(other_spec) = by_name.get_mut(gem_version.name) {
                let Ok(other_vp) = VersionPlatform::from_str(other_spec.gem_version.version) else {
                    continue;
                };

                if vp > other_vp {
                    *other_spec = spec.clone();
                }
            } else {
                by_name.insert(gem_version.name, spec.clone());
            }
        }

        by_name.into_values().collect()
    }

    pub fn discard_installed_gems(&mut self, install_path: &camino::Utf8PathBuf) {
        use std::path::Path;

        self.specs.retain(|spec| {
            let full_version = spec.gem_version;
            let gem_path = install_path.join(format!("gems/{full_version}"));
            let spec_path = install_path.join(format!("specifications/{full_version}.gemspec"));

            !Path::new(&gem_path).exists() || !Path::new(&spec_path).exists()
        });
    }
}

/// Filesystem path that gems could come from.
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PathSection<'i> {
    /// The filesystem path that sourced these dependencies.
    pub remote: &'i str,
    /// All gems which came from this source in particular.
    pub specs: Vec<Spec<'i>>,
}

/// A (gem, version) pair.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GemVersion<'i> {
    /// Name of the gem.
    pub name: &'i str,
    /// Version of the gem.
    pub version: &'i str,
}

impl<'i> std::fmt::Display for GemVersion<'i> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}-{}", self.name, self.version)
    }
}

/// A range of possible versions of a certain gem.
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GemRange<'i> {
    pub name: &'i str,
    pub semver: Option<Vec<GemRangeSemver<'i>>>,
    /// Dependencies specified with a source other than the main Rubygems index (e.g., git dependencies, path-based, dependencies) have a ! which means they are "pinned" to that source.
    /// According to <https://stackoverflow.com/questions/7517524/understanding-the-gemfile-lock-file>.
    pub nonstandard: bool,
}

/// A range of possible versions of a gem.
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GemRangeSemver<'i> {
    pub semver_constraint: SemverConstraint,
    #[serde(borrow)]
    pub version: &'i str,
}

/// Gem which has been locked and came from some particular source.
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Spec<'i> {
    #[serde(borrow)]
    pub gem_version: GemVersion<'i>,
    #[serde(borrow)]
    pub deps: Vec<GemRange<'i>>,
}

/// Checksum of a particular gem version.
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Checksum<'i> {
    #[serde(borrow)]
    pub gem_version: GemVersion<'i>,
    pub algorithm: ChecksumAlgorithm<'i>,
    pub value: Vec<u8>,
}

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ChecksumAlgorithm<'i> {
    None,
    #[serde(borrow)]
    Unknown(&'i str),
    #[default]
    SHA256,
}

/// Constrains the range of possible versions of a gem which could be selected.
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Copy)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum SemverConstraint {
    /// `=`
    Exact,
    /// `!=`
    NotEqual,
    /// `>`
    GreaterThan,
    /// `<`
    LessThan,
    /// `>=`
    GreaterThanOrEqual,
    /// `<=`
    LessThanOrEqual,
    /// `~>`
    Pessimistic,
}

impl std::fmt::Display for SemverConstraint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Exact => {
                write!(f, "=")
            }
            Self::NotEqual => {
                write!(f, "!=")
            }
            Self::GreaterThan => {
                write!(f, ">")
            }
            Self::LessThan => {
                write!(f, "<")
            }
            Self::GreaterThanOrEqual => {
                write!(f, ">=")
            }
            Self::LessThanOrEqual => {
                write!(f, "<=")
            }
            Self::Pessimistic => {
                write!(f, "~>")
            }
        }
    }
}
