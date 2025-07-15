use std::fmt;
use std::str::FromStr;

/// Represents a platform constraint similar to Gem::Platform
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Platform {
    /// Generic Ruby platform
    Ruby,
    /// Specific platform with CPU, OS, and optional version
    Specific {
        cpu: String,
        os: String,
        version: Option<String>,
    },
    /// Unknown/custom platform
    Unknown(String),
}

impl Platform {
    /// Create a new specific platform
    pub fn new(cpu: &str, os: &str, version: Option<&str>) -> Self {
        Platform::Specific {
            cpu: cpu.to_string(),
            os: os.to_string(),
            version: version.map(|v| v.to_string()),
        }
    }
    
    /// Get the platform string representation
    pub fn to_string(&self) -> String {
        match self {
            Platform::Ruby => "ruby".to_string(),
            Platform::Specific { cpu, os, version } => {
                if let Some(v) = version {
                    format!("{}-{}-{}", cpu, os, v)
                } else {
                    format!("{}-{}", cpu, os)
                }
            }
            Platform::Unknown(s) => s.clone(),
        }
    }
    
    /// Check if this platform is Ruby (generic)
    pub fn is_ruby(&self) -> bool {
        matches!(self, Platform::Ruby)
    }
    
    /// Get the CPU architecture if this is a specific platform
    pub fn cpu(&self) -> Option<&str> {
        match self {
            Platform::Specific { cpu, .. } => Some(cpu),
            _ => None,
        }
    }
    
    /// Get the OS if this is a specific platform
    pub fn os(&self) -> Option<&str> {
        match self {
            Platform::Specific { os, .. } => Some(os),
            _ => None,
        }
    }
    
    /// Get the version if this is a specific platform with version
    pub fn version(&self) -> Option<&str> {
        match self {
            Platform::Specific { version: Some(v), .. } => Some(v),
            _ => None,
        }
    }
}

impl FromStr for Platform {
    type Err = ();
    
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim();
        
        if s == "ruby" {
            return Ok(Platform::Ruby);
        }
        
        // Parse platform string like "x86_64-linux" or "x86_64-linux-gnu"
        let parts: Vec<&str> = s.split('-').collect();
        
        match parts.len() {
            2 => Ok(Platform::Specific {
                cpu: parts[0].to_string(),
                os: parts[1].to_string(),
                version: None,
            }),
            3 => Ok(Platform::Specific {
                cpu: parts[0].to_string(),
                os: parts[1].to_string(),
                version: Some(parts[2].to_string()),
            }),
            _ => Ok(Platform::Unknown(s.to_string())),
        }
    }
}

impl fmt::Display for Platform {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_string())
    }
}

impl PartialOrd for Platform {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Platform {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // Ruby platform comes first, then sort alphabetically
        match (self, other) {
            (Platform::Ruby, Platform::Ruby) => std::cmp::Ordering::Equal,
            (Platform::Ruby, _) => std::cmp::Ordering::Less,
            (_, Platform::Ruby) => std::cmp::Ordering::Greater,
            _ => self.to_string().cmp(&other.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_platform_parsing() {
        assert_eq!("ruby".parse::<Platform>().unwrap(), Platform::Ruby);
        
        let x86_linux = "x86_64-linux".parse::<Platform>().unwrap();
        assert_eq!(x86_linux.cpu(), Some("x86_64"));
        assert_eq!(x86_linux.os(), Some("linux"));
        assert_eq!(x86_linux.version(), None);
        
        let x86_linux_gnu = "x86_64-linux-gnu".parse::<Platform>().unwrap();
        assert_eq!(x86_linux_gnu.cpu(), Some("x86_64"));
        assert_eq!(x86_linux_gnu.os(), Some("linux"));
        assert_eq!(x86_linux_gnu.version(), Some("gnu"));
    }
    
    #[test]
    fn test_platform_ordering() {
        let ruby = Platform::Ruby;
        let linux = "x86_64-linux".parse::<Platform>().unwrap();
        let darwin = "x86_64-darwin".parse::<Platform>().unwrap();
        
        assert!(ruby < linux);
        assert!(ruby < darwin);
        assert!(darwin < linux); // alphabetical ordering
    }
}