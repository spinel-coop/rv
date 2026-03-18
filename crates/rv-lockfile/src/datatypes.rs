//! Most of the types in this module borrow a string from their input,
//! so they have a lifetime 'i, which is short for 'input.

use rv_gem_types::requirement::ComparisonOperator;
use rv_ruby::version::RubyVersion;

#[derive(Debug, Clone, Default, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct GemfileDotLock<'i> {
    /// Dependencies sourced from a Git repo.
    pub git: Vec<GitSection<'i>>,

    /// Dependencies sourced from a RubyGems server.
    pub gem: Vec<GemSection<'i>>,

    /// Dependencies sourced from a filesystem path.
    pub path: Vec<PathSection<'i>>,

    /// Lists every triple that Bundler has resolved and included in this lockfile.
    pub platforms: Vec<&'i str>,

    /// Lists every gem that this lockfile has been resolved to include
    pub dependencies: Vec<GemRange<'i>>,

    /// Which version of Ruby this lockfile was built with.
    pub ruby_version: Option<RubyVersionSection>,

    /// Which version of Bundler this lockfile was built with.
    pub bundled_with: Option<BundledWithSection<'i>>,

    /// Checksums for each dependency.
    pub checksums: Option<Vec<Checksum<'i>>>,
}

impl GemfileDotLock<'_> {
    /// Returns the total number of gem specs from RubyGems server sources.
    pub fn gem_spec_count(&self) -> usize {
        self.gem.iter().map(|s| s.specs.len()).sum()
    }

    pub fn spec_count(&self) -> usize {
        self.gem_spec_count()
            + self.git.iter().map(|s| s.specs.len()).sum::<usize>()
            + self.path.iter().map(|s| s.specs.len()).sum::<usize>()
    }
}

impl std::fmt::Display for GemfileDotLock<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for path_section in &self.path {
            writeln!(f, "{path_section}")?;
        }

        for git_section in &self.git {
            writeln!(f, "{git_section}")?;
        }

        for gem_section in &self.gem {
            writeln!(f, "{gem_section}")?;
        }

        writeln!(f, "PLATFORMS")?;
        for platform in &self.platforms {
            writeln!(f, "  {platform}")?;
        }

        writeln!(f, "\nDEPENDENCIES")?;
        for dep in &self.dependencies {
            writeln!(f, "  {dep}")?;
        }

        if let Some(checksums) = &self.checksums {
            writeln!(f, "\nCHECKSUMS")?;

            for checksum in checksums {
                writeln!(f, "{checksum}")?;
            }
        }

        if let Some(ruby_version) = &self.ruby_version {
            writeln!(f, "\nRUBY VERSION")?;

            writeln!(f, "{ruby_version}")?;
        }

        if let Some(bundled_with) = &self.bundled_with {
            writeln!(f, "\nBUNDLED WITH")?;

            writeln!(f, "{bundled_with}")?;
        }

        Ok(())
    }
}

/// Git source that gems could come from.
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct GitSection<'i> {
    /// Location of the Git repo.
    pub remote: &'i str,
    /// Commit used from the Git repo.
    pub revision: &'i str,
    /// Branch used from the Git repo.
    pub branch: Option<&'i str>,
    /// Ref used from the Git repo.
    pub git_ref: Option<&'i str>,
    /// Tag used from the Git repo.
    pub tag: Option<&'i str>,
    /// Includes git submodules, or not.
    /// Optional, defaults to false.
    pub submodules: Option<bool>,
    /// Optional gemspec glob
    pub glob: Option<&'i str>,
    /// All gems which came from this source in particular.
    pub specs: Vec<Spec<'i>>,
}

impl std::fmt::Display for GitSection<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "GIT")?;
        writeln!(f, "  remote: {}", self.remote)?;
        writeln!(f, "  revision: {}", self.revision)?;
        if let Some(git_ref) = self.git_ref {
            writeln!(f, "  ref: {git_ref}")?;
        }
        if let Some(branch) = self.branch {
            writeln!(f, "  branch: {branch}")?;
        }
        if let Some(glob) = self.glob {
            writeln!(f, "  glob: {glob}")?;
        }
        if let Some(tag) = self.tag {
            writeln!(f, "  tag: {tag}")?;
        }
        if let Some(submodules) = self.submodules {
            writeln!(f, "  submodules: {submodules}")?;
        }
        writeln!(f, "  specs:")?;
        for spec in &self.specs {
            write!(f, "{spec}")?;
        }

        Ok(())
    }
}

/// Rubygems server source that gems could come from.
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct GemSection<'i> {
    /// Location of the RubyGems server.
    pub remote: Option<&'i str>,
    /// All gems which came from this source in particular.
    pub specs: Vec<Spec<'i>>,
}

impl std::fmt::Display for GemSection<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "GEM")?;
        if let Some(remote) = self.remote {
            writeln!(f, "  remote: {remote}")?;
        }
        writeln!(f, "  specs:")?;
        for spec in &self.specs {
            write!(f, "{spec}")?;
        }

        Ok(())
    }
}

/// Filesystem path that gems could come from.
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct PathSection<'i> {
    /// The filesystem path that sourced these dependencies.
    pub remote: &'i str,
    /// All gems which came from this source in particular.
    pub specs: Vec<Spec<'i>>,
}

impl std::fmt::Display for PathSection<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "PATH")?;
        writeln!(f, "  remote: {}", self.remote)?;
        writeln!(f, "  specs:")?;
        for spec in &self.specs {
            write!(f, "{spec}")?;
        }

        Ok(())
    }
}

/// A (gem, version) pair.
#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct GemVersion<'i> {
    /// Name of the gem.
    pub name: &'i str,
    /// Version of the gem.
    pub version: &'i str,
}

impl<'i> GemVersion<'i> {
    pub fn full_name(&self) -> String {
        format!("{}-{}", self.name, self.version)
    }
}

impl std::fmt::Display for GemVersion<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({})", self.name, self.version)
    }
}

/// A range of possible versions of a certain gem.
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct GemRange<'i> {
    pub name: &'i str,
    pub semver: Option<Vec<GemRangeSemver<'i>>>,
    /// Dependencies specified with a source other than the main Rubygems index (e.g., git dependencies, path-based, dependencies) have a ! which means they are "pinned" to that source.
    /// According to <https://stackoverflow.com/questions/7517524/understanding-the-gemfile-lock-file>.
    pub nonstandard: bool,
}

impl std::fmt::Display for GemRange<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name)?;

        if let Some(semver) = &self.semver {
            let gem_ranges = semver
                .iter()
                .map(|gem_range| gem_range.to_string())
                .collect::<Vec<_>>()
                .join(", ");

            write!(f, " ({})", gem_ranges)?;
        }

        if self.nonstandard {
            write!(f, "!")?;
        }

        Ok(())
    }
}

/// A range of possible versions of a gem.
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct GemRangeSemver<'i> {
    pub semver_constraint: ComparisonOperator,
    pub version: &'i str,
}

impl std::fmt::Display for GemRangeSemver<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {}", self.semver_constraint, self.version)
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum LockfileIndentation {
    Standard,
    ThreeSpaces,
}

impl std::fmt::Display for LockfileIndentation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Standard => write!(f, "  "),
            Self::ThreeSpaces => write!(f, "   "),
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct RubyVersionSection {
    pub indentation: LockfileIndentation,
    pub cruby_version: RubyVersion,
    pub engine_version: Option<RubyVersion>,
}

impl std::fmt::Display for RubyVersionSection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}{}",
            self.indentation,
            self.cruby_version.to_gemfile_lock()
        )?;

        if let Some(engine_version) = &self.engine_version {
            write!(f, " ({})", engine_version.to_gemfile_lock())?;
        };

        Ok(())
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct BundledWithSection<'i> {
    pub indentation: LockfileIndentation,
    pub bundler_version: &'i str,
}

impl std::fmt::Display for BundledWithSection<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}{}", self.indentation, self.bundler_version)
    }
}

/// Gem which has been locked and came from some particular source.
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct Spec<'i> {
    pub gem_version: GemVersion<'i>,
    pub deps: Vec<GemRange<'i>>,
}

impl std::fmt::Display for Spec<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "    {}", self.gem_version)?;

        if !self.deps.is_empty() {
            let dep_strs = self
                .deps
                .iter()
                .map(|d| d.to_string())
                .collect::<Vec<_>>()
                .join("\n      ");

            writeln!(f, "      {dep_strs}")?;
        }

        Ok(())
    }
}

/// Checksum of a particular gem version.
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct Checksum<'i> {
    pub gem_version: GemVersion<'i>,
    pub algorithm: ChecksumAlgorithm<'i>,
    pub value: Vec<u8>,
}

impl std::fmt::Display for Checksum<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.algorithm {
            ChecksumAlgorithm::None => write!(f, "  {}", self.gem_version),
            other => write!(
                f,
                "  {} {}={}",
                self.gem_version,
                other,
                hex::encode(&self.value)
            ),
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Default)]
pub enum ChecksumAlgorithm<'i> {
    None,
    Unknown(&'i str),
    #[default]
    SHA256,
}

impl std::fmt::Display for ChecksumAlgorithm<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::None => write!(f, ""),
            Self::Unknown(algo) => write!(f, "{algo}"),
            Self::SHA256 => write!(f, "sha256"),
        }
    }
}
