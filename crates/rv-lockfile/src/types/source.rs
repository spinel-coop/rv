use indexmap::IndexMap;

/// Represents different types of gem sources in a lockfile
#[derive(Debug, Clone, PartialEq)]
pub enum Source {
    Git(GitSource),
    Gem(GemSource),
    Path(PathSource),
    Plugin(PluginSource),
}

impl Source {
    /// Get the source type as a string
    pub fn source_type(&self) -> &'static str {
        match self {
            Source::Git(_) => "GIT",
            Source::Gem(_) => "GEM",
            Source::Path(_) => "PATH",
            Source::Plugin(_) => "PLUGIN SOURCE",
        }
    }
}

/// Git source configuration
#[derive(Debug, Clone, PartialEq)]
pub struct GitSource {
    pub remote: String,
    pub revision: String,
    pub ref_: Option<String>,
    pub branch: Option<String>,
    pub tag: Option<String>,
    pub submodules: Option<bool>,
    pub glob: Option<String>,
    pub specs: Vec<String>, // Gem names in this source
}

impl GitSource {
    pub fn new(remote: String, revision: String) -> Self {
        GitSource {
            remote,
            revision,
            ref_: None,
            branch: None,
            tag: None,
            submodules: None,
            glob: None,
            specs: Vec::new(),
        }
    }
}

/// Gem source (RubyGems repositories)
#[derive(Debug, Clone, PartialEq)]
pub struct GemSource {
    pub remotes: Vec<String>,
    pub specs: Vec<String>, // Gem names in this source
}

impl GemSource {
    pub fn new(remote: String) -> Self {
        GemSource {
            remotes: vec![remote],
            specs: Vec::new(),
        }
    }
    
    pub fn add_remote(&mut self, remote: String) {
        if !self.remotes.contains(&remote) {
            self.remotes.push(remote);
        }
    }
}

/// Path source (local filesystem)
#[derive(Debug, Clone, PartialEq)]
pub struct PathSource {
    pub remote: String, // Actually a path
    pub glob: Option<String>,
    pub specs: Vec<String>, // Gem names in this source
}

impl PathSource {
    pub fn new(path: String) -> Self {
        PathSource {
            remote: path,
            glob: None,
            specs: Vec::new(),
        }
    }
}

/// Plugin source (extensible source type)
#[derive(Debug, Clone, PartialEq)]
pub struct PluginSource {
    pub source_type: String,
    pub options: IndexMap<String, String>,
    pub specs: Vec<String>, // Gem names in this source
}

impl PluginSource {
    pub fn new(source_type: String) -> Self {
        PluginSource {
            source_type,
            options: IndexMap::new(),
            specs: Vec::new(),
        }
    }
    
    pub fn add_option(&mut self, key: String, value: String) {
        self.options.insert(key, value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_git_source() {
        let git = GitSource::new(
            "https://github.com/example/repo.git".to_string(),
            "abc123".to_string(),
        );
        
        assert_eq!(git.remote, "https://github.com/example/repo.git");
        assert_eq!(git.revision, "abc123");
        assert_eq!(git.ref_, None);
    }
    
    #[test]
    fn test_gem_source() {
        let mut gem = GemSource::new("https://rubygems.org/".to_string());
        gem.add_remote("https://gems.example.com/".to_string());
        
        assert_eq!(gem.remotes.len(), 2);
        assert!(gem.remotes.contains(&"https://rubygems.org/".to_string()));
        assert!(gem.remotes.contains(&"https://gems.example.com/".to_string()));
    }
    
    #[test]
    fn test_plugin_source() {
        let mut plugin = PluginSource::new("custom".to_string());
        plugin.add_option("url".to_string(), "https://example.com".to_string());
        plugin.add_option("token".to_string(), "secret".to_string());
        
        assert_eq!(plugin.source_type, "custom");
        assert_eq!(plugin.options.get("url"), Some(&"https://example.com".to_string()));
        assert_eq!(plugin.options.get("token"), Some(&"secret".to_string()));
    }
}