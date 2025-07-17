use std::str::FromStr;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Platform {
    Ruby,
    Current,
    Specific {
        cpu: Option<String>,
        os: String,
        version: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PlatformError {
    #[error("Invalid platform string: {platform}")]
    InvalidPlatform { platform: String },
    #[error("Array platform must have at most 3 elements")]
    InvalidArray,
}

impl Platform {
    pub fn new(platform: impl Into<String>) -> Result<Self, PlatformError> {
        let platform_str = platform.into();

        match platform_str.as_str() {
            "ruby" | "" => Ok(Platform::Ruby),
            "current" => Ok(Platform::Current),
            _ => Self::parse_platform_string(&platform_str),
        }
    }

    pub fn from_array(parts: &[String]) -> Result<Self, PlatformError> {
        if parts.len() > 3 {
            return Err(PlatformError::InvalidArray);
        }

        let cpu = parts.first().cloned();
        let os = parts.get(1).cloned().unwrap_or_default();
        let version = parts.get(2).cloned();

        Ok(Platform::Specific { cpu, os, version })
    }

    fn parse_platform_string(platform: &str) -> Result<Platform, PlatformError> {
        let platform = platform.trim_end_matches('-');
        let parts: Vec<&str> = platform.split('-').collect();

        if parts.is_empty() {
            return Err(PlatformError::InvalidPlatform {
                platform: platform.to_string(),
            });
        }

        let (cpu, os_string) = if parts.len() == 1 {
            // Single part - treat as OS (legacy jruby case)
            (None, parts[0].to_string())
        } else {
            let cpu_part = parts[0];
            let os_part = parts[1..].join("-");

            // Handle special CPU cases
            let cpu = if cpu_part.starts_with('i')
                && cpu_part.len() == 4
                && cpu_part.chars().skip(1).all(|c| c.is_ascii_digit())
            {
                Some("x86".to_string())
            } else if cpu_part == "dotnet" {
                None
            } else {
                Some(cpu_part.to_string())
            };

            let os = if cpu_part == "dotnet" {
                format!("dotnet-{os_part}")
            } else {
                os_part
            };

            (cpu, os)
        };

        let (parsed_os, version) = Self::parse_os_and_version(&os_string);

        // Special case: set CPU to x86 for mswin32 when CPU is None
        let final_cpu = if cpu.is_none() && parsed_os.ends_with("32") {
            Some("x86".to_string())
        } else {
            cpu
        };

        Ok(Platform::Specific {
            cpu: final_cpu,
            os: parsed_os,
            version,
        })
    }

    fn parse_os_and_version(os: &str) -> (String, Option<String>) {
        // Handle various OS patterns based on rubygems logic
        match os {
            s if s.starts_with("aix") => {
                let version = s
                    .strip_prefix("aix")
                    .and_then(|v| v.strip_prefix('-'))
                    .map(|v| v.to_string());
                ("aix".to_string(), version)
            }
            s if s.starts_with("cygwin") => ("cygwin".to_string(), None),
            s if s.starts_with("darwin") => {
                let version_part = s.strip_prefix("darwin").unwrap();
                let version = if version_part.is_empty() {
                    None
                } else if version_part.starts_with('-') {
                    Some(version_part.strip_prefix('-').unwrap().to_string())
                } else {
                    Some(version_part.to_string())
                };
                ("darwin".to_string(), version)
            }
            "macruby" => ("macruby".to_string(), None),
            s if s.starts_with("macruby") => {
                let version = s
                    .strip_prefix("macruby")
                    .and_then(|v| v.strip_prefix('-'))
                    .map(|v| v.to_string());
                ("macruby".to_string(), version)
            }
            s if s.starts_with("freebsd") => {
                let version = s
                    .strip_prefix("freebsd")
                    .and_then(|v| v.strip_prefix('-'))
                    .map(|v| v.to_string());
                ("freebsd".to_string(), version)
            }
            "java" | "jruby" => ("java".to_string(), None),
            s if s.starts_with("java") => {
                let version = s
                    .strip_prefix("java")
                    .and_then(|v| v.strip_prefix('-'))
                    .map(|v| v.to_string());
                ("java".to_string(), version)
            }
            s if s.starts_with("dalvik") => {
                let version = s
                    .strip_prefix("dalvik")
                    .and_then(|v| v.strip_prefix('-'))
                    .map(|v| v.to_string());
                ("dalvik".to_string(), version)
            }
            "dotnet" => ("dotnet".to_string(), None),
            s if s.starts_with("dotnet") => {
                let version = s
                    .strip_prefix("dotnet")
                    .and_then(|v| v.strip_prefix('-'))
                    .map(|v| v.to_string());
                ("dotnet".to_string(), version)
            }
            s if s.starts_with("linux") => {
                let version = s
                    .strip_prefix("linux")
                    .and_then(|v| v.strip_prefix('-'))
                    .map(|v| v.to_string());
                ("linux".to_string(), version)
            }
            "mingw32" => ("mingw32".to_string(), None),
            s if s.starts_with("mingw") => {
                let version = s
                    .strip_prefix("mingw")
                    .and_then(|v| v.strip_prefix('-'))
                    .map(|v| v.to_string());
                ("mingw".to_string(), version)
            }
            s if s.starts_with("mswin") => {
                // Handle mswin32_80 or mswin32-80 patterns
                if let Some(underscore_pos) = s.find('_') {
                    let os_part = &s[..underscore_pos];
                    let version_part = &s[underscore_pos + 1..];
                    (os_part.to_string(), Some(version_part.to_string()))
                } else if let Some(dash_pos) = s.find('-') {
                    let os_part = &s[..dash_pos];
                    let version_part = &s[dash_pos + 1..];
                    (os_part.to_string(), Some(version_part.to_string()))
                } else {
                    (s.to_string(), None)
                }
            }
            "netbsdelf" => ("netbsdelf".to_string(), None),
            s if s.starts_with("openbsd") => {
                let version = s
                    .strip_prefix("openbsd")
                    .and_then(|v| v.strip_prefix('-'))
                    .map(|v| v.to_string());
                ("openbsd".to_string(), version)
            }
            s if s.starts_with("solaris") => {
                let version = s
                    .strip_prefix("solaris")
                    .and_then(|v| v.strip_prefix('-'))
                    .map(|v| v.to_string());
                ("solaris".to_string(), version)
            }
            s if s.starts_with("wasi") => ("wasi".to_string(), None),
            _ => (os.to_string(), None),
        }
    }

    pub fn to_array(&self) -> Vec<Option<String>> {
        match self {
            Platform::Ruby => vec![None, Some("ruby".to_string()), None],
            Platform::Current => vec![None, Some("current".to_string()), None],
            Platform::Specific { cpu, os, version } => {
                vec![cpu.clone(), Some(os.clone()), version.clone()]
            }
        }
    }

    pub fn matches(&self, other: &Platform) -> bool {
        match (self, other) {
            (Platform::Ruby, Platform::Ruby) => true,
            (Platform::Current, Platform::Current) => true,
            (
                Platform::Specific {
                    cpu: cpu1,
                    os: os1,
                    version: version1,
                },
                Platform::Specific {
                    cpu: cpu2,
                    os: os2,
                    version: version2,
                },
            ) => {
                self.cpu_matches(cpu1, cpu2)
                    && os1 == os2
                    && self.version_matches(os1, version1, version2)
            }
            _ => false,
        }
    }

    fn cpu_matches(&self, cpu1: &Option<String>, cpu2: &Option<String>) -> bool {
        match (cpu1, cpu2) {
            (None, _) | (_, None) => true,
            (Some(c1), Some(c2)) if c1 == "universal" || c2 == "universal" => true,
            (Some(c1), Some(c2)) if c1 == c2 => true,
            (Some(c1), Some(c2)) if c1 == "arm" && c2.starts_with("armv") => true,
            _ => false,
        }
    }

    fn version_matches(
        &self,
        os: &str,
        version1: &Option<String>,
        version2: &Option<String>,
    ) -> bool {
        match (version1, version2) {
            // For non-Linux platforms, any None version matches
            (None, _) | (_, None) if os != "linux" => true,
            // For Linux platforms, both None versions match
            (None, None) if os == "linux" => true,
            // For Linux platforms with both versions, they need to match exactly
            (Some(v1), Some(v2)) if os == "linux" => v1 == v2,
            // For all other platforms, versions must match exactly
            (Some(v1), Some(v2)) => v1 == v2,
            _ => false,
        }
    }
}

impl std::fmt::Display for Platform {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Platform::Ruby => write!(f, "ruby"),
            Platform::Current => write!(f, "current"),
            Platform::Specific { cpu, os, version } => {
                let parts: Vec<String> = [cpu.as_ref(), Some(os), version.as_ref()]
                    .iter()
                    .filter_map(|opt| opt.cloned())
                    .collect();

                if cpu.is_none() {
                    write!(f, "{}", parts.join(""))
                } else {
                    write!(f, "{}", parts.join("-"))
                }
            }
        }
    }
}

impl FromStr for Platform {
    type Err = PlatformError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Platform::new(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_platform_constants() {
        assert_eq!(Platform::new("ruby").unwrap(), Platform::Ruby);
        assert_eq!(Platform::new("").unwrap(), Platform::Ruby);
        assert_eq!(Platform::new("current").unwrap(), Platform::Current);
    }

    #[test]
    fn test_platform_parsing() {
        let test_cases = vec![
            ("java", [None, Some("java".to_string()), None]),
            ("jruby", [None, Some("java".to_string()), None]),
            (
                "i686-darwin",
                [Some("x86".to_string()), Some("darwin".to_string()), None],
            ),
            (
                "x86_64-linux",
                [Some("x86_64".to_string()), Some("linux".to_string()), None],
            ),
            (
                "x86_64-linux-gnu",
                [
                    Some("x86_64".to_string()),
                    Some("linux".to_string()),
                    Some("gnu".to_string()),
                ],
            ),
            (
                "arm-linux-eabi",
                [
                    Some("arm".to_string()),
                    Some("linux".to_string()),
                    Some("eabi".to_string()),
                ],
            ),
            (
                "universal-darwin8",
                [
                    Some("universal".to_string()),
                    Some("darwin".to_string()),
                    Some("8".to_string()),
                ],
            ),
            (
                "mswin32",
                [Some("x86".to_string()), Some("mswin32".to_string()), None],
            ),
            (
                "i386-mswin32-80",
                [
                    Some("x86".to_string()),
                    Some("mswin32".to_string()),
                    Some("80".to_string()),
                ],
            ),
        ];

        for (platform_str, expected) in test_cases {
            let platform = Platform::new(platform_str).unwrap();
            if let Platform::Specific { cpu, os, version } = platform {
                assert_eq!(
                    [cpu, Some(os), version],
                    expected,
                    "Failed for platform: {platform_str}"
                );
            } else {
                panic!("Expected Specific platform for: {platform_str}");
            }
        }
    }

    #[test]
    fn test_platform_display() {
        assert_eq!(Platform::Ruby.to_string(), "ruby");
        assert_eq!(Platform::Current.to_string(), "current");

        let platform = Platform::Specific {
            cpu: Some("x86_64".to_string()),
            os: "linux".to_string(),
            version: Some("gnu".to_string()),
        };
        assert_eq!(platform.to_string(), "x86_64-linux-gnu");

        let platform = Platform::Specific {
            cpu: None,
            os: "java".to_string(),
            version: None,
        };
        assert_eq!(platform.to_string(), "java");
    }

    #[test]
    fn test_platform_matching() {
        let linux_x86_64 = Platform::new("x86_64-linux").unwrap();
        let _linux_x86_64_gnu = Platform::new("x86_64-linux-gnu").unwrap();
        let linux_arm = Platform::new("arm-linux").unwrap();

        assert!(linux_x86_64.matches(&linux_x86_64));
        assert!(!linux_x86_64.matches(&linux_arm));

        let universal_darwin = Platform::new("universal-darwin8").unwrap();
        let x86_darwin = Platform::new("x86-darwin8").unwrap();

        assert!(universal_darwin.matches(&x86_darwin));
        assert!(x86_darwin.matches(&universal_darwin));
    }

    #[test]
    fn test_from_array() {
        let platform =
            Platform::from_array(&["x86_64".to_string(), "linux".to_string(), "gnu".to_string()])
                .unwrap();

        if let Platform::Specific { cpu, os, version } = platform {
            assert_eq!(cpu, Some("x86_64".to_string()));
            assert_eq!(os, "linux");
            assert_eq!(version, Some("gnu".to_string()));
        } else {
            panic!("Expected Specific platform");
        }
    }
}
