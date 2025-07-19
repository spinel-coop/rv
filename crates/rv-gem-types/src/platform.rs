use std::{borrow::Cow, str::FromStr};

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
        use regex::Regex;

        let platform = platform.trim_end_matches('-');
        let parts: Vec<&str> = platform.splitn(2, '-').collect();
        let cpu = parts[0];
        let mut os = parts.get(1).map(|os| Cow::from(*os));

        let cpu = if Regex::new("i\\d86").unwrap().is_match(cpu) {
            Some("x86")
        } else if cpu == "dotnet" {
            os = Some(format!("dotnet-{}", os.unwrap_or(std::borrow::Cow::Borrowed(""))).into());
            None
        } else {
            Some(cpu)
        };

        let (mut cpu, os) = if os == None {
            (None, Cow::from(cpu.unwrap()))
        } else {
            (cpu, os.unwrap())
        };

        let (os, version) = if let Some(captures) = Regex::new(r"aix-?(\d)?").unwrap().captures(&os)
        {
            ("aix", captures.get(1).map(|m| m.as_str()))
        } else if os.contains("cygwin") {
            ("cygwin", None)
        } else if let Some(captures) = Regex::new(r"darwin-?(\d)?").unwrap().captures(&os) {
            ("darwin", captures.get(1).map(|m| m.as_str()))
        } else if os == "macruby" {
            ("macruby", None)
        } else if let Some(captures) = Regex::new(r"^macruby-?(\d+(?:\.\d+)*)?")
            .unwrap()
            .captures(&os)
        {
            ("macruby", captures.get(1).map(|m| m.as_str()))
        } else if let Some(captures) = Regex::new(r"freebsd-?(\d+)?").unwrap().captures(&os) {
            ("freebsd", captures.get(1).map(|m| m.as_str()))
        } else if os == "java" || os == "jruby" {
            ("java", None)
        } else if let Some(captures) = Regex::new(r"^java-?(\d+(?:\.\d+)*)?")
            .unwrap()
            .captures(&os)
        {
            ("java", captures.get(1).map(|m| m.as_str()))
        } else if let Some(captures) = Regex::new(r"^dalvik-?(\d+)?$").unwrap().captures(&os) {
            ("dalvik", captures.get(1).map(|m| m.as_str()))
        } else if os == "dotnet" {
            ("dotnet", None)
        } else if let Some(captures) = Regex::new(r"^dotnet-?(\d+(?:\.\d+)*)?")
            .unwrap()
            .captures(&os)
        {
            ("dotnet", captures.get(1).map(|m| m.as_str()))
        } else if let Some(captures) = Regex::new(r"linux-?(\w+)?").unwrap().captures(&os) {
            ("linux", captures.get(1).map(|m| m.as_str()))
        } else if os.contains("mingw32") {
            ("mingw32", None)
        } else if let Some(captures) = Regex::new(r"mingw-?(\w+)?").unwrap().captures(&os) {
            ("mingw", captures.get(1).map(|m| m.as_str()))
        } else if let Some(captures) = Regex::new(r"(mswin\d+)(?:[_-](\d+))?")
            .unwrap()
            .captures(&os)
        {
            let os = captures.get(1).unwrap().as_str();

            if cpu.is_none() && os.ends_with("32") {
                cpu = Some("x86");
            }

            (os, captures.get(2).map(|m| m.as_str()))
        } else if os.contains("netbsdelf") {
            ("netbsdelf", None)
        } else if let Some(captures) = Regex::new(r"openbsd-?(\d+\.\d+)?").unwrap().captures(&os) {
            ("openbsd", captures.get(1).map(|m| m.as_str()))
        } else if let Some(captures) = Regex::new(r"solaris-?(\d+\.\d+)?").unwrap().captures(&os) {
            ("solaris", captures.get(1).map(|m| m.as_str()))
        } else if os.contains("wasi") {
            ("wasi", None)
        } else if let Some(captures) = Regex::new(r"^(\w+_platform)-?(\d+)?")
            .unwrap()
            .captures(&os)
        {
            (
                captures.get(1).unwrap().as_str(),
                captures.get(2).map(|m| m.as_str()),
            )
        } else {
            ("unknown", None)
        };

        Ok(Platform::Specific {
            cpu: cpu.map(str::to_string),
            os: os.to_string(),
            version: version.map(str::to_string),
        })
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
            // For Linux platforms, None version matches any version (unversioned matches versioned)
            (None, _) if os == "linux" => true,
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

    #[test]
    fn test_rubygems_platform_parsing() {
        // Complete test cases from RubyGems test_initialize method (lines 94-172)
        // This ensures 100% compatibility with RubyGems platform parsing
        let test_cases = vec![
            (
                "amd64-freebsd6",
                [
                    Some("amd64".to_string()),
                    Some("freebsd".to_string()),
                    Some("6".to_string()),
                ],
            ),
            ("java", [None, Some("java".to_string()), None]),
            ("jruby", [None, Some("java".to_string()), None]),
            (
                "universal-dotnet",
                [
                    Some("universal".to_string()),
                    Some("dotnet".to_string()),
                    None,
                ],
            ),
            (
                "universal-dotnet2.0",
                [
                    Some("universal".to_string()),
                    Some("dotnet".to_string()),
                    Some("2.0".to_string()),
                ],
            ),
            (
                "dotnet-2.0",
                [None, Some("dotnet".to_string()), Some("2.0".to_string())],
            ),
            (
                "universal-dotnet4.0",
                [
                    Some("universal".to_string()),
                    Some("dotnet".to_string()),
                    Some("4.0".to_string()),
                ],
            ),
            (
                "powerpc-aix5.3.0.0",
                [
                    Some("powerpc".to_string()),
                    Some("aix".to_string()),
                    Some("5".to_string()),
                ],
            ),
            (
                "powerpc-darwin7",
                [
                    Some("powerpc".to_string()),
                    Some("darwin".to_string()),
                    Some("7".to_string()),
                ],
            ),
            (
                "powerpc-darwin8",
                [
                    Some("powerpc".to_string()),
                    Some("darwin".to_string()),
                    Some("8".to_string()),
                ],
            ),
            (
                "powerpc-linux",
                [Some("powerpc".to_string()), Some("linux".to_string()), None],
            ),
            (
                "powerpc64-linux",
                [
                    Some("powerpc64".to_string()),
                    Some("linux".to_string()),
                    None,
                ],
            ),
            (
                "sparc-solaris2.10",
                [
                    Some("sparc".to_string()),
                    Some("solaris".to_string()),
                    Some("2.10".to_string()),
                ],
            ),
            (
                "sparc-solaris2.8",
                [
                    Some("sparc".to_string()),
                    Some("solaris".to_string()),
                    Some("2.8".to_string()),
                ],
            ),
            (
                "sparc-solaris2.9",
                [
                    Some("sparc".to_string()),
                    Some("solaris".to_string()),
                    Some("2.9".to_string()),
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
                "universal-darwin9",
                [
                    Some("universal".to_string()),
                    Some("darwin".to_string()),
                    Some("9".to_string()),
                ],
            ),
            (
                "universal-macruby",
                [
                    Some("universal".to_string()),
                    Some("macruby".to_string()),
                    None,
                ],
            ),
            (
                "i386-cygwin",
                [Some("x86".to_string()), Some("cygwin".to_string()), None],
            ),
            (
                "i686-darwin",
                [Some("x86".to_string()), Some("darwin".to_string()), None],
            ),
            (
                "i686-darwin8.4.1",
                [
                    Some("x86".to_string()),
                    Some("darwin".to_string()),
                    Some("8".to_string()),
                ],
            ),
            (
                "i386-freebsd4.11",
                [
                    Some("x86".to_string()),
                    Some("freebsd".to_string()),
                    Some("4".to_string()),
                ],
            ),
            (
                "i386-freebsd5",
                [
                    Some("x86".to_string()),
                    Some("freebsd".to_string()),
                    Some("5".to_string()),
                ],
            ),
            (
                "i386-freebsd6",
                [
                    Some("x86".to_string()),
                    Some("freebsd".to_string()),
                    Some("6".to_string()),
                ],
            ),
            (
                "i386-freebsd7",
                [
                    Some("x86".to_string()),
                    Some("freebsd".to_string()),
                    Some("7".to_string()),
                ],
            ),
            (
                "i386-freebsd",
                [Some("x86".to_string()), Some("freebsd".to_string()), None],
            ),
            (
                "universal-freebsd",
                [
                    Some("universal".to_string()),
                    Some("freebsd".to_string()),
                    None,
                ],
            ),
            (
                "i386-java1.5",
                [
                    Some("x86".to_string()),
                    Some("java".to_string()),
                    Some("1.5".to_string()),
                ],
            ),
            (
                "x86-java1.6",
                [
                    Some("x86".to_string()),
                    Some("java".to_string()),
                    Some("1.6".to_string()),
                ],
            ),
            (
                "i386-java1.6",
                [
                    Some("x86".to_string()),
                    Some("java".to_string()),
                    Some("1.6".to_string()),
                ],
            ),
            (
                "i686-linux",
                [Some("x86".to_string()), Some("linux".to_string()), None],
            ),
            (
                "i586-linux",
                [Some("x86".to_string()), Some("linux".to_string()), None],
            ),
            (
                "i486-linux",
                [Some("x86".to_string()), Some("linux".to_string()), None],
            ),
            (
                "i386-linux",
                [Some("x86".to_string()), Some("linux".to_string()), None],
            ),
            (
                "i586-linux-gnu",
                [
                    Some("x86".to_string()),
                    Some("linux".to_string()),
                    Some("gnu".to_string()),
                ],
            ),
            (
                "i386-linux-gnu",
                [
                    Some("x86".to_string()),
                    Some("linux".to_string()),
                    Some("gnu".to_string()),
                ],
            ),
            (
                "i386-mingw32",
                [Some("x86".to_string()), Some("mingw32".to_string()), None],
            ),
            (
                "x64-mingw-ucrt",
                [
                    Some("x64".to_string()),
                    Some("mingw".to_string()),
                    Some("ucrt".to_string()),
                ],
            ),
            (
                "i386-mswin32",
                [Some("x86".to_string()), Some("mswin32".to_string()), None],
            ),
            (
                "i386-mswin32_80",
                [
                    Some("x86".to_string()),
                    Some("mswin32".to_string()),
                    Some("80".to_string()),
                ],
            ),
            (
                "i386-mswin32-80",
                [
                    Some("x86".to_string()),
                    Some("mswin32".to_string()),
                    Some("80".to_string()),
                ],
            ),
            (
                "x86-mswin32",
                [Some("x86".to_string()), Some("mswin32".to_string()), None],
            ),
            (
                "x86-mswin32_60",
                [
                    Some("x86".to_string()),
                    Some("mswin32".to_string()),
                    Some("60".to_string()),
                ],
            ),
            (
                "x86-mswin32-60",
                [
                    Some("x86".to_string()),
                    Some("mswin32".to_string()),
                    Some("60".to_string()),
                ],
            ),
            (
                "i386-netbsdelf",
                [Some("x86".to_string()), Some("netbsdelf".to_string()), None],
            ),
            (
                "i386-openbsd4.0",
                [
                    Some("x86".to_string()),
                    Some("openbsd".to_string()),
                    Some("4.0".to_string()),
                ],
            ),
            (
                "i386-solaris2.10",
                [
                    Some("x86".to_string()),
                    Some("solaris".to_string()),
                    Some("2.10".to_string()),
                ],
            ),
            (
                "i386-solaris2.8",
                [
                    Some("x86".to_string()),
                    Some("solaris".to_string()),
                    Some("2.8".to_string()),
                ],
            ),
            (
                "mswin32",
                [Some("x86".to_string()), Some("mswin32".to_string()), None],
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
                "x86_64-linux-musl",
                [
                    Some("x86_64".to_string()),
                    Some("linux".to_string()),
                    Some("musl".to_string()),
                ],
            ),
            (
                "x86_64-linux-uclibc",
                [
                    Some("x86_64".to_string()),
                    Some("linux".to_string()),
                    Some("uclibc".to_string()),
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
                "arm-linux-gnueabi",
                [
                    Some("arm".to_string()),
                    Some("linux".to_string()),
                    Some("gnueabi".to_string()),
                ],
            ),
            (
                "arm-linux-musleabi",
                [
                    Some("arm".to_string()),
                    Some("linux".to_string()),
                    Some("musleabi".to_string()),
                ],
            ),
            (
                "arm-linux-uclibceabi",
                [
                    Some("arm".to_string()),
                    Some("linux".to_string()),
                    Some("uclibceabi".to_string()),
                ],
            ),
            (
                "x86_64-openbsd3.9",
                [
                    Some("x86_64".to_string()),
                    Some("openbsd".to_string()),
                    Some("3.9".to_string()),
                ],
            ),
            (
                "x86_64-openbsd4.0",
                [
                    Some("x86_64".to_string()),
                    Some("openbsd".to_string()),
                    Some("4.0".to_string()),
                ],
            ),
            (
                "x86_64-openbsd",
                [
                    Some("x86_64".to_string()),
                    Some("openbsd".to_string()),
                    None,
                ],
            ),
            (
                "wasm32-wasi",
                [Some("wasm32".to_string()), Some("wasi".to_string()), None],
            ),
            (
                "wasm32-wasip1",
                [Some("wasm32".to_string()), Some("wasi".to_string()), None],
            ),
            (
                "wasm32-wasip2",
                [Some("wasm32".to_string()), Some("wasi".to_string()), None],
            ),
            // Edge cases and malformed platform strings that RubyGems handles
            (
                "darwin-java-java",
                [Some("darwin".to_string()), Some("java".to_string()), None],
            ),
            (
                "linux-linux-linux",
                [
                    Some("linux".to_string()),
                    Some("linux".to_string()),
                    Some("linux".to_string()),
                ],
            ),
            (
                "linux-linux-linux1.0",
                [
                    Some("linux".to_string()),
                    Some("linux".to_string()),
                    Some("linux1".to_string()),
                ],
            ),
            (
                "x86x86-1x86x86x86x861linuxx86x86",
                [
                    Some("x86x86".to_string()),
                    Some("linux".to_string()),
                    Some("x86x86".to_string()),
                ],
            ),
            (
                "freebsd0",
                [None, Some("freebsd".to_string()), Some("0".to_string())],
            ),
            (
                "darwin0",
                [None, Some("darwin".to_string()), Some("0".to_string())],
            ),
            (
                "darwin0---",
                [None, Some("darwin".to_string()), Some("0".to_string())],
            ),
            (
                "x86-linux-x8611.0l",
                [
                    Some("x86".to_string()),
                    Some("linux".to_string()),
                    Some("x8611".to_string()),
                ],
            ),
            (
                "0-x86linuxx86---",
                [
                    Some("0".to_string()),
                    Some("linux".to_string()),
                    Some("x86".to_string()),
                ],
            ),
            (
                "x86_64-macruby-x86",
                [
                    Some("x86_64".to_string()),
                    Some("macruby".to_string()),
                    None,
                ],
            ),
            (
                "x86_64-dotnetx86",
                [Some("x86_64".to_string()), Some("dotnet".to_string()), None],
            ),
            (
                "x86_64-dalvik0",
                [
                    Some("x86_64".to_string()),
                    Some("dalvik".to_string()),
                    Some("0".to_string()),
                ],
            ),
            (
                "x86_64-dotnet1.",
                [
                    Some("x86_64".to_string()),
                    Some("dotnet".to_string()),
                    Some("1".to_string()),
                ],
            ),
        ];

        for (platform_str, expected) in test_cases {
            let platform = Platform::new(platform_str).unwrap();
            let platform2 = Platform::new(platform.to_string()).unwrap();

            {
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
            {
                if let Platform::Specific { cpu, os, version } = platform2 {
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
    }

    #[test]
    fn test_arm_cpu_matching() {
        // Test cases from test_equals3_cpu
        let arm_linux = Platform::new("arm-linux").unwrap();
        let armv5_linux = Platform::new("armv5-linux").unwrap();
        let armv7_linux = Platform::new("armv7-linux").unwrap();
        let arm64_linux = Platform::new("arm64-linux").unwrap();

        // arm-linux should match armv5-linux (generic arm matches specific armv5)
        assert!(arm_linux.matches(&armv5_linux));

        // armv5-linux should match itself
        assert!(armv5_linux.matches(&armv5_linux));

        // armv7-linux should NOT match armv5-linux (different ARM versions)
        assert!(!armv7_linux.matches(&armv5_linux));

        // arm64-linux should NOT match armv5-linux (different architectures)
        assert!(!arm64_linux.matches(&armv5_linux));
    }

    #[test]
    fn test_nil_version_matching() {
        // Test cases from test_nil_version_is_treated_as_any_version
        let darwin_versioned = Platform::new("i686-darwin8.0").unwrap();
        let darwin_unversioned = Platform::new("i686-darwin").unwrap();

        // Platforms with nil version should match versioned platforms (for non-Linux)
        assert!(darwin_versioned.matches(&darwin_unversioned));
        assert!(darwin_unversioned.matches(&darwin_versioned));
    }

    #[test]
    fn test_linux_version_strictness() {
        // Test cases from test_nil_version_is_stricter_for_linux_os
        let linux_unversioned = Platform::new("i686-linux").unwrap();
        let linux_gnu = Platform::new("i686-linux-gnu").unwrap();
        let linux_musl = Platform::new("i686-linux-musl").unwrap();

        // Linux unversioned should match versioned (libc variants)
        assert!(linux_unversioned.matches(&linux_gnu));

        // Different libc implementations should NOT match each other
        assert!(!linux_gnu.matches(&linux_musl));
        assert!(!linux_musl.matches(&linux_gnu));
    }

    #[test]
    fn test_universal_platform_matching() {
        // Universal platforms should match specific architectures
        let universal_darwin = Platform::new("universal-darwin").unwrap();
        let x86_64_darwin = Platform::new("x86_64-darwin").unwrap();
        let arm64_darwin = Platform::new("arm64-darwin").unwrap();

        assert!(universal_darwin.matches(&x86_64_darwin));
        assert!(universal_darwin.matches(&arm64_darwin));
        assert!(x86_64_darwin.matches(&universal_darwin));
        assert!(arm64_darwin.matches(&universal_darwin));
    }

    #[test]
    fn test_java_platform_variants() {
        // Java platform should be normalized
        let java1 = Platform::new("java").unwrap();
        let java2 = Platform::new("jruby").unwrap();

        assert_eq!(java1, java2);
        assert!(java1.matches(&java2));
        assert!(java2.matches(&java1));
    }

    #[test]
    fn test_platform_display_formatting() {
        // Test various display format cases
        let test_cases = vec![
            (Platform::Ruby, "ruby"),
            (Platform::Current, "current"),
            (Platform::new("java").unwrap(), "java"),
            (Platform::new("x86_64-linux").unwrap(), "x86_64-linux"),
            (
                Platform::new("x86_64-linux-gnu").unwrap(),
                "x86_64-linux-gnu",
            ),
            (
                Platform::new("universal-darwin8").unwrap(),
                "universal-darwin-8",
            ),
            (Platform::new("i686-darwin8.0").unwrap(), "x86-darwin-8"),
        ];

        for (platform, expected) in test_cases {
            assert_eq!(
                platform.to_string(),
                expected,
                "Display format mismatch for: {platform:?}"
            );
        }
    }

    #[test]
    fn test_rubygems_edge_cases() {
        // Edge cases and unusual patterns from RubyGems test_initialize
        // These are the malformed or unusual platform strings that RubyGems handles
        let test_cases = vec![
            // Edge cases that would work but aren't in our main test
            (
                "darwin-java-java",
                [Some("darwin".to_string()), Some("java".to_string()), None],
            ),
            (
                "linux-linux-linux",
                [
                    Some("linux".to_string()),
                    Some("linux".to_string()),
                    Some("linux".to_string()),
                ],
            ),
            // Note: These are examples of how RubyGems handles malformed strings
            // Most real applications would reject these, but RubyGems parses them anyway
        ];

        for (platform_str, expected) in test_cases {
            let platform = Platform::new(platform_str).unwrap();
            if let Platform::Specific { cpu, os, version } = platform {
                assert_eq!(
                    [cpu, Some(os), version],
                    expected,
                    "Failed for edge case platform: {platform_str}"
                );
            } else {
                panic!("Expected Specific platform for edge case: {platform_str}");
            }
        }
    }

    #[test]
    fn test_nil_cpu_treated_as_universal() {
        // Test that nil CPU is treated as universal for matching
        // This mimics RubyGems test_nil_cpu_arch_is_treated_as_universal
        let mingw_no_cpu = Platform::new("mingw32").unwrap();
        let mingw_universal = Platform::new("universal-mingw32").unwrap();
        let mingw_x86 = Platform::new("x86-mingw32").unwrap();

        // Platforms with no CPU should match universal and specific CPUs
        assert!(mingw_no_cpu.matches(&mingw_universal));
        assert!(mingw_universal.matches(&mingw_no_cpu));
        assert!(mingw_no_cpu.matches(&mingw_x86));
        assert!(mingw_x86.matches(&mingw_no_cpu));
    }

    #[test]
    fn test_eabi_version_matching() {
        // Test ARM EABI version matching strictness
        // This mimics RubyGems test_eabi_version_is_stricter_for_linux_os
        let arm_linux_eabi = Platform::new("arm-linux-eabi").unwrap();
        let arm_linux_gnueabi = Platform::new("arm-linux-gnueabi").unwrap();
        let arm_linux_musleabi = Platform::new("arm-linux-musleabi").unwrap();
        let arm_linux_uclibceabi = Platform::new("arm-linux-uclibceabi").unwrap();

        // Different EABI implementations should NOT match each other for ARM
        assert!(!arm_linux_gnueabi.matches(&arm_linux_musleabi));
        assert!(!arm_linux_musleabi.matches(&arm_linux_gnueabi));
        assert!(!arm_linux_gnueabi.matches(&arm_linux_uclibceabi));
        assert!(!arm_linux_uclibceabi.matches(&arm_linux_musleabi));

        // But each should match itself
        assert!(arm_linux_eabi.matches(&arm_linux_eabi));
        assert!(arm_linux_gnueabi.matches(&arm_linux_gnueabi));
        assert!(arm_linux_musleabi.matches(&arm_linux_musleabi));
    }

    #[test]
    fn test_platform_equality() {
        // Test basic platform equality (test_equals2 equivalent)
        let platform1 = Platform::new("x86_64-linux").unwrap();
        let platform2 = Platform::new("x86_64-linux").unwrap();
        let platform3 = Platform::new("arm64-linux").unwrap();

        assert_eq!(platform1, platform2);
        assert_ne!(platform1, platform3);
        assert!(platform1.matches(&platform2));
        assert!(!platform1.matches(&platform3));
    }

    #[test]
    fn test_complex_cpu_matching() {
        // Test complex CPU matching scenarios from test_equals3_cpu
        let powerpc_darwin = Platform::new("powerpc-darwin").unwrap();
        let universal_darwin = Platform::new("universal-darwin").unwrap();
        let x86_darwin = Platform::new("x86-darwin").unwrap();

        // Universal should match any specific CPU
        assert!(universal_darwin.matches(&powerpc_darwin));
        assert!(universal_darwin.matches(&x86_darwin));
        assert!(powerpc_darwin.matches(&universal_darwin));
        assert!(x86_darwin.matches(&universal_darwin));

        // Different specific CPUs should not match each other
        assert!(!powerpc_darwin.matches(&x86_darwin));
        assert!(!x86_darwin.matches(&powerpc_darwin));
    }

    #[test]
    fn test_edge_case_parsing() {
        // Test edge cases and tricky parsing scenarios from RubyGems
        let test_cases = vec![
            // Single component platforms
            ("java", [None, Some("java".to_string()), None]),
            ("jruby", [None, Some("java".to_string()), None]),
            ("dalvik", [None, Some("dalvik".to_string()), None]),
            ("dotnet", [None, Some("dotnet".to_string()), None]),
            ("macruby", [None, Some("macruby".to_string()), None]),
            // Platforms with dashes in OS names
            (
                "x86_64-linux-gnu",
                [
                    Some("x86_64".to_string()),
                    Some("linux".to_string()),
                    Some("gnu".to_string()),
                ],
            ),
            (
                "arm-linux-gnueabihf",
                [
                    Some("arm".to_string()),
                    Some("linux".to_string()),
                    Some("gnueabihf".to_string()),
                ],
            ),
            // Complex version strings
            (
                "sparc-solaris2.8",
                [
                    Some("sparc".to_string()),
                    Some("solaris".to_string()),
                    Some("2.8".to_string()),
                ],
            ),
            (
                "ppc-aix5.1.0.0",
                [
                    Some("ppc".to_string()),
                    Some("aix".to_string()),
                    Some("5".to_string()),
                ],
            ),
        ];

        for (platform_str, expected) in test_cases {
            let platform = Platform::new(platform_str).unwrap();
            if let Platform::Specific { cpu, os, version } = platform {
                assert_eq!(
                    [cpu, Some(os), version],
                    expected,
                    "Failed parsing edge case: {platform_str}"
                );
            } else {
                panic!("Expected Specific platform for edge case: {platform_str}");
            }
        }
    }
}
