use crate::error::ParseError;
use crate::types::*;
use indexmap::IndexMap;
use regex::Regex;
use semver::Version;

/// Main lockfile parser
pub struct LockfileParser {
    sources: Vec<Source>,
    specs: IndexMap<String, LazySpecification>,
    dependencies: IndexMap<String, Dependency>,
    platforms: Vec<Platform>,
    bundler_version: Option<Version>,
    ruby_version: Option<String>,
    checksums: IndexMap<String, Checksum>,
    /// The key of the current gem spec being parsed (for associating dependencies)
    current_gem_spec: Option<String>,
}

impl std::fmt::Debug for LockfileParser {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LockfileParser")
            .field("sources", &self.sources)
            .field("specs", &self.specs)
            .field("dependencies", &self.dependencies)
            .field("platforms", &self.platforms)
            .field("bundler_version", &self.bundler_version)
            .field("ruby_version", &self.ruby_version)
            .field("checksums", &self.checksums)
            .finish()
    }
}

impl LockfileParser {
    /// Create a new parser with the given content
    pub fn new(content: &str) -> Result<Self, ParseError> {
        let mut parser = LockfileParser {
            sources: Vec::new(),
            specs: IndexMap::new(),
            dependencies: IndexMap::new(),
            platforms: Vec::new(),
            bundler_version: None,
            ruby_version: None,
            checksums: IndexMap::new(),
            current_gem_spec: None,
        };

        parser.parse(content)?;
        Ok(parser)
    }

    /// Parse the lockfile content
    fn parse(&mut self, content: &str) -> Result<(), ParseError> {
        let lines: Vec<&str> = content.lines().collect();
        let mut line_num = 0;
        let mut current_section = ParseState::None;
        let mut current_source: Option<Source> = None;

        // Check for merge conflicts first
        for (i, line) in lines.iter().enumerate() {
            if line.contains("<<<<<<< HEAD")
                || line.contains("=======")
                || line.contains(">>>>>>> ")
            {
                return Err(ParseError::merge_conflict(i + 1).with_source_context(content, i, line));
            }
        }

        while line_num < lines.len() {
            let line = lines[line_num];
            line_num += 1;

            // Skip empty lines
            if line.trim().is_empty() {
                continue;
            }

            // Check for section headers
            if let Some(new_section) = self.detect_section(line, line_num)? {
                // Finalize current source if we're leaving a source section
                if matches!(current_section, ParseState::Source(_)) {
                    if let Some(source) = current_source.take() {
                        self.sources.push(source);
                    }
                    // Clear current gem spec when leaving source section
                    self.current_gem_spec = None;
                }

                current_section = new_section;

                // Initialize new source if entering a source section
                if let ParseState::Source(ref source_type) = current_section {
                    current_source = Some(self.create_source(source_type)?);
                }

                continue;
            }

            // Parse line based on current section
            match &current_section {
                ParseState::None => {
                    // Ignore lines outside of sections
                }
                ParseState::Source(_) => {
                    if let Some(ref mut source) = current_source {
                        self.parse_source_line(source, line, line_num)?;
                    }
                }
                ParseState::Dependencies => {
                    self.parse_dependency_line(line, line_num)?;
                }
                ParseState::Platforms => {
                    self.parse_platform_line(line, line_num)?;
                }
                ParseState::RubyVersion => {
                    self.parse_ruby_line(line, line_num)?;
                }
                ParseState::BundledWith => {
                    self.parse_bundled_line(line, line_num)?;
                }
                ParseState::Checksums => {
                    self.parse_checksum_line(line, line_num)?;
                }
            }
        }

        // Finalize last source if needed
        if let Some(source) = current_source {
            self.sources.push(source);
        }

        // Sort collections
        self.platforms.sort();

        Ok(())
    }

    /// Detect section headers
    fn detect_section(
        &mut self,
        line: &str,
        _line_num: usize,
    ) -> Result<Option<ParseState>, ParseError> {
        let trimmed = line.trim();

        match trimmed {
            "GIT" => Ok(Some(ParseState::Source("GIT".to_string()))),
            "GEM" => Ok(Some(ParseState::Source("GEM".to_string()))),
            "PATH" => Ok(Some(ParseState::Source("PATH".to_string()))),
            "PLUGIN SOURCE" => Ok(Some(ParseState::Source("PLUGIN SOURCE".to_string()))),
            "DEPENDENCIES" => Ok(Some(ParseState::Dependencies)),
            "PLATFORMS" => Ok(Some(ParseState::Platforms)),
            "RUBY VERSION" => Ok(Some(ParseState::RubyVersion)),
            "BUNDLED WITH" => Ok(Some(ParseState::BundledWith)),
            "CHECKSUMS" => Ok(Some(ParseState::Checksums)),
            _ => Ok(None),
        }
    }

    /// Create a new source based on type
    fn create_source(&self, source_type: &str) -> Result<Source, ParseError> {
        match source_type {
            "GIT" => Ok(Source::Git(GitSource::new("".to_string(), "".to_string()))),
            "GEM" => Ok(Source::Gem(GemSource::new("".to_string()))),
            "PATH" => Ok(Source::Path(PathSource::new("".to_string()))),
            "PLUGIN SOURCE" => Ok(Source::Plugin(PluginSource::new("".to_string()))),
            _ => Err(ParseError::unknown_source_type(0, source_type.to_string())),
        }
    }

    /// Parse a line within a source section
    fn parse_source_line(
        &mut self,
        source: &mut Source,
        line: &str,
        line_num: usize,
    ) -> Result<(), ParseError> {
        let indent = self.count_leading_spaces(line);
        let content = line.trim();

        match indent {
            2 => {
                // Source configuration
                self.parse_source_config(source, content, line_num)?;
            }
            4 => {
                // Gem specification
                if content == "specs:" {
                    // specs: header, do nothing
                } else {
                    self.parse_gem_spec(source, content, line_num)?;
                }
            }
            6 => {
                // Gem dependency - associate with the last parsed gem spec
                self.parse_gem_dependency(content, line_num)?;
            }
            _ => {}
        }

        Ok(())
    }

    /// Parse source configuration lines
    fn parse_source_config(
        &mut self,
        source: &mut Source,
        content: &str,
        _line_num: usize,
    ) -> Result<(), ParseError> {
        if let Some((key, value)) = content.split_once(": ") {
            match source {
                Source::Git(ref mut git) => {
                    match key {
                        "remote" => git.remote = value.to_string(),
                        "revision" => git.revision = value.to_string(),
                        "ref" => git.ref_ = Some(value.to_string()),
                        "branch" => git.branch = Some(value.to_string()),
                        "tag" => git.tag = Some(value.to_string()),
                        "submodules" => git.submodules = Some(value == "true"),
                        "glob" => git.glob = Some(value.to_string()),
                        _ => {} // Ignore unknown options
                    }
                }
                Source::Gem(ref mut gem) => {
                    if key == "remote" {
                        if gem.remotes.is_empty() || gem.remotes[0].is_empty() {
                            gem.remotes = vec![value.to_string()];
                        } else {
                            gem.add_remote(value.to_string());
                        }
                    }
                }
                Source::Path(ref mut path) => {
                    match key {
                        "remote" => path.remote = value.to_string(),
                        "glob" => path.glob = Some(value.to_string()),
                        _ => {} // Ignore unknown options
                    }
                }
                Source::Plugin(ref mut plugin) => {
                    plugin.add_option(key.to_string(), value.to_string());
                }
            }
        }

        Ok(())
    }

    /// Parse gem specification lines
    fn parse_gem_spec(
        &mut self,
        source: &mut Source,
        content: &str,
        line_num: usize,
    ) -> Result<(), ParseError> {
        // Parse gem name and version using regex similar to bundler
        // Handles formats like: gem-name (1.0.0), gem-name (1.0.0-platform)
        let re = Regex::new(r"^([^\s]+)\s+\(([^-\)]+)(?:-(.+))?\)(?:\s+(.+))?$").unwrap();

        if let Some(captures) = re.captures(content) {
            let name = captures.get(1).unwrap().as_str().to_string();
            let version_str = captures.get(2).unwrap().as_str();
            let platform_str = captures.get(3).map(|m| m.as_str()).unwrap_or("ruby");

            let version = Version::parse(version_str)
                .map_err(|_| ParseError::invalid_version(line_num, version_str.to_string()))?;

            let platform = platform_str
                .parse()
                .map_err(|_| ParseError::invalid_platform(line_num, platform_str.to_string()))?;

            let spec = LazySpecification::new(name.clone(), version, platform);
            let spec_key = spec.full_name();

            // Set as current gem for dependency association
            self.current_gem_spec = Some(spec_key.clone());

            self.specs.insert(spec_key, spec);

            // Add to source
            match source {
                Source::Git(ref mut git) => git.specs.push(name),
                Source::Gem(ref mut gem) => gem.specs.push(name),
                Source::Path(ref mut path) => path.specs.push(name),
                Source::Plugin(ref mut plugin) => plugin.specs.push(name),
            }
        } else {
            return Err(ParseError::invalid_specification(
                line_num,
                content.to_string(),
            ));
        }

        Ok(())
    }

    /// Parse gem dependency lines (6-space indented)
    fn parse_gem_dependency(&mut self, content: &str, _line_num: usize) -> Result<(), ParseError> {
        // Parse dependency format: "gem-name (>= 1.0, < 2.0)"
        let re = Regex::new(r"^([^\s]+)(?:\s+\(([^)]+)\))?$").unwrap();

        if let Some(captures) = re.captures(content) {
            let name = captures.get(1).unwrap().as_str().to_string();
            let mut dependency = Dependency::new(name);

            if let Some(req_match) = captures.get(2) {
                let req_str = req_match.as_str();
                // Parse version requirements - convert Ruby-style to semver
                let semver_req = req_str
                    .replace("~>", "~") // Convert pessimistic operator
                    .replace(" ", ""); // Remove spaces

                if let Ok(req) = semver::VersionReq::parse(&semver_req) {
                    dependency.requirements.push(req);
                } else {
                    // If semver parsing fails, try to handle common Ruby patterns
                    if req_str.contains(">=") || req_str.contains("<=") || req_str.contains("=") {
                        // For now, store as a default requirement
                        dependency.requirements.push(semver::VersionReq::default());
                    }
                }
            }

            // Associate with current gem spec
            if let Some(ref current_spec_key) = self.current_gem_spec.clone() {
                if let Some(spec) = self.specs.get_mut(current_spec_key) {
                    spec.dependencies.push(dependency);
                }
            }
        }

        Ok(())
    }

    /// Parse dependency lines
    fn parse_dependency_line(&mut self, line: &str, line_num: usize) -> Result<(), ParseError> {

        let content = line.trim();
        let pinned = content.ends_with('!');
        let content = if pinned {
            &content[..content.len() - 1]
        } else {
            content
        };

        // Parse dependency format: "gem-name (>= 1.0, < 2.0)"
        let re = Regex::new(r"^([^\s]+)(?:\s+\(([^)]+)\))?$").unwrap();

        if let Some(captures) = re.captures(content) {
            let name = captures.get(1).unwrap().as_str().to_string();
            let mut dependency = Dependency::new(name.clone());
            dependency.set_pinned(pinned);

            if let Some(_req_match) = captures.get(2) {
                // TODO: Implement proper requirement parsing
            }

            self.dependencies.insert(name, dependency);
        } else {
            return Err(ParseError::invalid_dependency(
                line_num,
                content.to_string(),
            ));
        }

        Ok(())
    }

    /// Parse platform lines
    fn parse_platform_line(&mut self, line: &str, line_num: usize) -> Result<(), ParseError> {

        let platform_str = line.trim();
        let platform = platform_str
            .parse()
            .map_err(|_| ParseError::invalid_platform(line_num, platform_str.to_string()))?;

        if !self.platforms.contains(&platform) {
            self.platforms.push(platform);
        }

        Ok(())
    }

    /// Parse ruby version lines
    fn parse_ruby_line(&mut self, line: &str, _line_num: usize) -> Result<(), ParseError> {
        let content = line.trim();
        if let Some(stripped) = content.strip_prefix("ruby ") {
            self.ruby_version = Some(stripped.to_string());
        }
        Ok(())
    }

    /// Parse bundled with lines
    fn parse_bundled_line(&mut self, line: &str, _line_num: usize) -> Result<(), ParseError> {
        let version_str = line.trim();
        if let Ok(version) = Version::parse(version_str) {
            self.bundler_version = Some(version);
        }
        Ok(())
    }

    /// Parse checksum lines
    fn parse_checksum_line(&mut self, line: &str, line_num: usize) -> Result<(), ParseError> {

        let content = line.trim();

        // Parse checksum format: "gem-name (version) algorithm=hash" or "gem-name (version-platform) algorithm=hash"
        let re = Regex::new(r"^([^\s]+)\s+\(([^-\)]+)(?:-(.+))?\)\s+([^=]+)=(.+)$").unwrap();

        if let Some(captures) = re.captures(content) {
            let name = captures.get(1).unwrap().as_str().to_string();
            let version_str = captures.get(2).unwrap().as_str();
            let platform_str = captures.get(3).map(|m| m.as_str());
            let algorithm = captures.get(4).unwrap().as_str().to_string();
            let value = captures.get(5).unwrap().as_str().to_string();

            let version = Version::parse(version_str)
                .map_err(|_| ParseError::invalid_version(line_num, version_str.to_string()))?;

            let platform = if let Some(platform_str) = platform_str {
                Some(platform_str.parse().map_err(|_| {
                    ParseError::invalid_platform(line_num, platform_str.to_string())
                })?)
            } else {
                None
            };

            let checksum = Checksum::new(name.clone(), version, platform, algorithm, value);
            self.checksums.insert(checksum.full_name(), checksum);
        } else {
            return Err(ParseError::invalid_checksum(line_num, content.to_string()));
        }

        Ok(())
    }

    /// Count leading spaces in a line
    fn count_leading_spaces(&self, line: &str) -> usize {
        line.chars().take_while(|&c| c == ' ').count()
    }

    // Public API methods

    /// Get all sources
    pub fn sources(&self) -> &[Source] {
        &self.sources
    }

    /// Get all specifications
    pub fn specs(&self) -> &IndexMap<String, LazySpecification> {
        &self.specs
    }

    /// Get all dependencies
    pub fn dependencies(&self) -> &IndexMap<String, Dependency> {
        &self.dependencies
    }

    /// Get all platforms
    pub fn platforms(&self) -> &[Platform] {
        &self.platforms
    }

    /// Get bundler version
    pub fn bundler_version(&self) -> Option<&Version> {
        self.bundler_version.as_ref()
    }

    /// Get ruby version
    pub fn ruby_version(&self) -> Option<&str> {
        self.ruby_version.as_deref()
    }

    /// Get all checksums
    pub fn checksums(&self) -> &IndexMap<String, Checksum> {
        &self.checksums
    }
}

/// Parsing state machine
#[derive(Debug, Clone)]
enum ParseState {
    None,
    Source(String), // Source type
    Dependencies,
    Platforms,
    RubyVersion,
    BundledWith,
    Checksums,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_parsing() {
        let content = r#"
GEM
  remote: https://rubygems.org/
  specs:
    test-gem (1.0.0)

PLATFORMS
  ruby

DEPENDENCIES
  test-gem

BUNDLED WITH
   2.3.0
"#;

        let parser = LockfileParser::new(content).unwrap();
        assert_eq!(parser.platforms().len(), 1);
        assert_eq!(parser.dependencies().len(), 1);
        assert!(parser.bundler_version().is_some());
    }

    #[test]
    fn test_merge_conflict_detection() {
        let content = r#"
<<<<<<< HEAD
GEM
  remote: https://rubygems.org/
=======
GEM
  remote: https://example.com/
>>>>>>> branch
"#;

        let result = LockfileParser::new(content);
        assert!(matches!(result, Err(ParseError::MergeConflict { .. })));
    }
}
