use once_cell::sync::Lazy;
use regex::Regex;
use std::{borrow::Cow, str::FromStr};

// Cached regexes for platform parsing to avoid repeated compilation
static I386_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"i\d86").unwrap());
static AIX_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"aix-?(\d)?").unwrap());
static DARWIN_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"darwin-?(\d)?").unwrap());
static MACRUBY_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^macruby-?(\d+(?:\.\d+)*)?").unwrap());
static FREEBSD_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"freebsd-?(\d+)?").unwrap());
static JAVA_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"^java-?(\d+(?:\.\d+)*)?").unwrap());
static DALVIK_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"^dalvik-?(\d+)?$").unwrap());
static DOTNET_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"^dotnet-?(\d+(?:\.\d+)*)?").unwrap());
static LINUX_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"linux-?(\w+)?").unwrap());
static MINGW_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"mingw-?(\w+)?").unwrap());
static MSWIN_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"(mswin\d+)(?:[_-](\d+))?").unwrap());
static OPENBSD_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"openbsd-?(\d+\.\d+)?").unwrap());
static SOLARIS_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"solaris-?(\d+\.\d+)?").unwrap());
static PLATFORM_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"^(\w+_platform)-?(\d+)?").unwrap());

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
    pub fn new(platform: impl AsRef<str>) -> Result<Self, PlatformError> {
        match platform.as_ref() {
            "ruby" | "" => Ok(Platform::Ruby),
            "current" => Ok(Platform::Current),
            str => Self::parse_platform_string(str),
        }
    }

    pub fn ruby() -> Self {
        Platform::Ruby
    }

    pub fn is_ruby(&self) -> bool {
        matches!(self, Platform::Ruby)
    }

    pub fn java() -> Self {
        Self::new("java").unwrap()
    }

    pub fn mswin() -> Self {
        Self::new("mswin32").unwrap()
    }

    pub fn mswin64() -> Self {
        Self::new("mswin64").unwrap()
    }

    pub fn universal_mingw() -> Self {
        Self::new("universal-mingw").unwrap()
    }

    pub fn windows() -> Vec<Self> {
        vec![Self::mswin(), Self::mswin64(), Self::universal_mingw()]
    }

    fn parse_platform_string(platform: &str) -> Result<Platform, PlatformError> {
        let platform = platform.trim_end_matches('-');
        let parts: Vec<&str> = platform.splitn(2, '-').collect();
        let Some(cpu) = parts.first() else {
            return Err(PlatformError::InvalidPlatform {
                platform: platform.to_owned(),
            });
        };
        let cpu = *cpu;
        let mut os = parts.get(1).map(|os| Cow::from(*os));

        let cpu = if I386_REGEX.is_match(cpu) {
            Some("x86")
        } else if cpu == "dotnet" {
            os = Some(match os {
                Some(os_val) => format!("dotnet-{os_val}").into(),
                None => "dotnet-".into(),
            });
            None
        } else {
            Some(cpu)
        };

        let (mut cpu, os) = if let Some(os) = os {
            (cpu, os)
        } else {
            (None, Cow::from(cpu.unwrap()))
        };

        let (os, version) = if let Some(captures) = AIX_REGEX.captures(&os) {
            ("aix", captures.get(1).map(|m| m.as_str()))
        } else if os.contains("cygwin") {
            ("cygwin", None)
        } else if let Some(captures) = DARWIN_REGEX.captures(&os) {
            ("darwin", captures.get(1).map(|m| m.as_str()))
        } else if os == "macruby" {
            ("macruby", None)
        } else if let Some(captures) = MACRUBY_REGEX.captures(&os) {
            ("macruby", captures.get(1).map(|m| m.as_str()))
        } else if let Some(captures) = FREEBSD_REGEX.captures(&os) {
            ("freebsd", captures.get(1).map(|m| m.as_str()))
        } else if os == "java" || os == "jruby" {
            ("java", None)
        } else if let Some(captures) = JAVA_REGEX.captures(&os) {
            ("java", captures.get(1).map(|m| m.as_str()))
        } else if let Some(captures) = DALVIK_REGEX.captures(&os) {
            ("dalvik", captures.get(1).map(|m| m.as_str()))
        } else if os == "dotnet" {
            ("dotnet", None)
        } else if let Some(captures) = DOTNET_REGEX.captures(&os) {
            ("dotnet", captures.get(1).map(|m| m.as_str()))
        } else if let Some(captures) = LINUX_REGEX.captures(&os) {
            ("linux", captures.get(1).map(|m| m.as_str()))
        } else if os.contains("mingw32") {
            ("mingw32", None)
        } else if let Some(captures) = MINGW_REGEX.captures(&os) {
            ("mingw", captures.get(1).map(|m| m.as_str()))
        } else if let Some(captures) = MSWIN_REGEX.captures(&os) {
            let os = captures.get(1).unwrap().as_str();

            if cpu.is_none() && os.ends_with("32") {
                cpu = Some("x86");
            }

            (os, captures.get(2).map(|m| m.as_str()))
        } else if os.contains("netbsdelf") {
            ("netbsdelf", None)
        } else if let Some(captures) = OPENBSD_REGEX.captures(&os) {
            ("openbsd", captures.get(1).map(|m| m.as_str()))
        } else if let Some(captures) = SOLARIS_REGEX.captures(&os) {
            ("solaris", captures.get(1).map(|m| m.as_str()))
        } else if os.contains("wasi") {
            ("wasi", None)
        } else if let Some(captures) = PLATFORM_REGEX.captures(&os) {
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

    pub fn to_array(&self) -> [Option<&str>; 3] {
        match self {
            Platform::Ruby => [None, Some("ruby"), None],
            Platform::Current => [None, Some("current"), None],
            Platform::Specific { cpu, os, version } => {
                [cpu.as_deref(), Some(os), version.as_deref()]
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
                // Special mingw universal matching (like RubyGems line 202-203)
                if (cpu1.as_deref() == Some("universal") || cpu2.as_deref() == Some("universal"))
                    && os1.starts_with("mingw")
                    && os2.starts_with("mingw")
                {
                    return true;
                }

                // CPU matching logic (like RubyGems lines 206-207)
                let cpu_compatible = cpu1.is_none()
                    || cpu2.is_none()
                    || cpu1.as_deref() == Some("universal")
                    || cpu2.as_deref() == Some("universal")
                    || cpu1 == cpu2
                    || (cpu1.as_deref() == Some("arm")
                        && cpu2.as_ref().is_some_and(|c| c.starts_with("armv")));

                // OS matching (like RubyGems line 210)
                let os_compatible = os1 == os2;

                // Version matching (like RubyGems lines 213-217)
                let version_compatible = if os1 != "linux" {
                    // For non-Linux platforms, nil version matches any version
                    version1.is_none() || version2.is_none() || version1 == version2
                } else {
                    // For Linux platforms, nil version matches any version (unversioned matches versioned libc)
                    version1.is_none() || version2.is_none() || version1 == version2
                    // TODO: Add musl variant matching if needed
                };

                cpu_compatible && os_compatible && version_compatible
            }
            _ => false,
        }
    }

    pub fn generic(&self) -> Self {
        match self {
            Platform::Current => unimplemented!(),
            Platform::Ruby => Platform::Ruby,
            Platform::Specific { .. } => {
                let mut generics = Self::windows();
                generics.insert(0, Self::java());
                generics
                    .into_iter()
                    .find(|generic| self.matches(generic))
                    .unwrap_or(Platform::Ruby)
            }
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
    fn test_rubygems_platform_parsing() {
        // Complete test cases from RubyGems test_initialize method (lines 94-172)
        // This ensures 100% compatibility with RubyGems platform parsing
        let test_cases = vec![
            (
                "amd64-freebsd6",
                [Some("amd64"), Some("freebsd"), Some("6")],
            ),
            ("java", [None, Some("java"), None]),
            ("jruby", [None, Some("java"), None]),
            (
                "universal-dotnet",
                [Some("universal"), Some("dotnet"), None],
            ),
            (
                "universal-dotnet2.0",
                [Some("universal"), Some("dotnet"), Some("2.0")],
            ),
            ("dotnet-2.0", [None, Some("dotnet"), Some("2.0")]),
            (
                "universal-dotnet4.0",
                [Some("universal"), Some("dotnet"), Some("4.0")],
            ),
            (
                "powerpc-aix5.3.0.0",
                [Some("powerpc"), Some("aix"), Some("5")],
            ),
            (
                "powerpc-darwin7",
                [Some("powerpc"), Some("darwin"), Some("7")],
            ),
            (
                "powerpc-darwin8",
                [Some("powerpc"), Some("darwin"), Some("8")],
            ),
            ("powerpc-linux", [Some("powerpc"), Some("linux"), None]),
            ("powerpc64-linux", [Some("powerpc64"), Some("linux"), None]),
            (
                "sparc-solaris2.10",
                [Some("sparc"), Some("solaris"), Some("2.10")],
            ),
            (
                "sparc-solaris2.8",
                [Some("sparc"), Some("solaris"), Some("2.8")],
            ),
            (
                "sparc-solaris2.9",
                [Some("sparc"), Some("solaris"), Some("2.9")],
            ),
            (
                "universal-darwin8",
                [Some("universal"), Some("darwin"), Some("8")],
            ),
            (
                "universal-darwin9",
                [Some("universal"), Some("darwin"), Some("9")],
            ),
            (
                "universal-macruby",
                [Some("universal"), Some("macruby"), None],
            ),
            ("i386-cygwin", [Some("x86"), Some("cygwin"), None]),
            ("i686-darwin", [Some("x86"), Some("darwin"), None]),
            ("i686-darwin8.4.1", [Some("x86"), Some("darwin"), Some("8")]),
            (
                "i386-freebsd4.11",
                [Some("x86"), Some("freebsd"), Some("4")],
            ),
            ("i386-freebsd5", [Some("x86"), Some("freebsd"), Some("5")]),
            ("i386-freebsd6", [Some("x86"), Some("freebsd"), Some("6")]),
            ("i386-freebsd7", [Some("x86"), Some("freebsd"), Some("7")]),
            ("i386-freebsd", [Some("x86"), Some("freebsd"), None]),
            (
                "universal-freebsd",
                [Some("universal"), Some("freebsd"), None],
            ),
            ("i386-java1.5", [Some("x86"), Some("java"), Some("1.5")]),
            ("x86-java1.6", [Some("x86"), Some("java"), Some("1.6")]),
            ("i386-java1.6", [Some("x86"), Some("java"), Some("1.6")]),
            ("i686-linux", [Some("x86"), Some("linux"), None]),
            ("i586-linux", [Some("x86"), Some("linux"), None]),
            ("i486-linux", [Some("x86"), Some("linux"), None]),
            ("i386-linux", [Some("x86"), Some("linux"), None]),
            ("i586-linux-gnu", [Some("x86"), Some("linux"), Some("gnu")]),
            ("i386-linux-gnu", [Some("x86"), Some("linux"), Some("gnu")]),
            ("i386-mingw32", [Some("x86"), Some("mingw32"), None]),
            ("x64-mingw-ucrt", [Some("x64"), Some("mingw"), Some("ucrt")]),
            ("i386-mswin32", [Some("x86"), Some("mswin32"), None]),
            (
                "i386-mswin32_80",
                [Some("x86"), Some("mswin32"), Some("80")],
            ),
            (
                "i386-mswin32-80",
                [Some("x86"), Some("mswin32"), Some("80")],
            ),
            ("x86-mswin32", [Some("x86"), Some("mswin32"), None]),
            ("x86-mswin32_60", [Some("x86"), Some("mswin32"), Some("60")]),
            ("x86-mswin32-60", [Some("x86"), Some("mswin32"), Some("60")]),
            ("i386-netbsdelf", [Some("x86"), Some("netbsdelf"), None]),
            (
                "i386-openbsd4.0",
                [Some("x86"), Some("openbsd"), Some("4.0")],
            ),
            (
                "i386-solaris2.10",
                [Some("x86"), Some("solaris"), Some("2.10")],
            ),
            (
                "i386-solaris2.8",
                [Some("x86"), Some("solaris"), Some("2.8")],
            ),
            ("mswin32", [Some("x86"), Some("mswin32"), None]),
            ("x86_64-linux", [Some("x86_64"), Some("linux"), None]),
            (
                "x86_64-linux-gnu",
                [Some("x86_64"), Some("linux"), Some("gnu")],
            ),
            (
                "x86_64-linux-musl",
                [Some("x86_64"), Some("linux"), Some("musl")],
            ),
            (
                "x86_64-linux-uclibc",
                [Some("x86_64"), Some("linux"), Some("uclibc")],
            ),
            ("arm-linux-eabi", [Some("arm"), Some("linux"), Some("eabi")]),
            (
                "arm-linux-gnueabi",
                [Some("arm"), Some("linux"), Some("gnueabi")],
            ),
            (
                "arm-linux-musleabi",
                [Some("arm"), Some("linux"), Some("musleabi")],
            ),
            (
                "arm-linux-uclibceabi",
                [Some("arm"), Some("linux"), Some("uclibceabi")],
            ),
            (
                "x86_64-openbsd3.9",
                [Some("x86_64"), Some("openbsd"), Some("3.9")],
            ),
            (
                "x86_64-openbsd4.0",
                [Some("x86_64"), Some("openbsd"), Some("4.0")],
            ),
            ("x86_64-openbsd", [Some("x86_64"), Some("openbsd"), None]),
            ("wasm32-wasi", [Some("wasm32"), Some("wasi"), None]),
            ("wasm32-wasip1", [Some("wasm32"), Some("wasi"), None]),
            ("wasm32-wasip2", [Some("wasm32"), Some("wasi"), None]),
            // Edge cases and malformed platform strings that RubyGems handles
            ("darwin-java-java", [Some("darwin"), Some("java"), None]),
            (
                "linux-linux-linux",
                [Some("linux"), Some("linux"), Some("linux")],
            ),
            (
                "linux-linux-linux1.0",
                [Some("linux"), Some("linux"), Some("linux1")],
            ),
            (
                "x86x86-1x86x86x86x861linuxx86x86",
                [Some("x86x86"), Some("linux"), Some("x86x86")],
            ),
            ("freebsd0", [None, Some("freebsd"), Some("0")]),
            ("darwin0", [None, Some("darwin"), Some("0")]),
            ("darwin0---", [None, Some("darwin"), Some("0")]),
            (
                "x86-linux-x8611.0l",
                [Some("x86"), Some("linux"), Some("x8611")],
            ),
            ("0-x86linuxx86---", [Some("0"), Some("linux"), Some("x86")]),
            (
                "x86_64-macruby-x86",
                [Some("x86_64"), Some("macruby"), None],
            ),
            ("x86_64-dotnetx86", [Some("x86_64"), Some("dotnet"), None]),
            (
                "x86_64-dalvik0",
                [Some("x86_64"), Some("dalvik"), Some("0")],
            ),
            (
                "x86_64-dotnet1.",
                [Some("x86_64"), Some("dotnet"), Some("1")],
            ),
        ];

        for (platform_str, expected) in test_cases {
            // let expected = expected.map(|s| s.map(|s| s.to_string()));
            let platform = Platform::new(platform_str).unwrap();
            assert_eq!(expected, platform.to_array());

            let platform2 = Platform::new(platform.to_string()).unwrap();
            assert_eq!(expected, platform2.to_array());
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

    #[test]
    fn test_platform_generic() {
        // Test cases from test_generic in RubyGems test_gem_platform.rb

        // Non-Windows platforms should convert to ruby
        assert_eq!(
            Platform::new("x86-darwin-10").unwrap().generic(),
            Platform::ruby()
        );
        assert_eq!(Platform::ruby().generic(), Platform::ruby());
        assert_eq!(
            Platform::new("unknown").unwrap().generic(),
            Platform::ruby()
        );

        // Java platform variants should convert to java
        assert_eq!(Platform::new("java").unwrap().generic(), Platform::java());
        assert_eq!(
            Platform::new("universal-java-17").unwrap().generic(),
            Platform::java()
        );

        // MSWin platform variants should convert to mswin32
        assert_eq!(
            Platform::new("mswin32").unwrap().generic(),
            Platform::mswin()
        );
        assert_eq!(
            Platform::new("i386-mswin32").unwrap().generic(),
            Platform::mswin()
        );
        assert_eq!(
            Platform::new("x86-mswin32").unwrap().generic(),
            Platform::mswin()
        );

        // MSWin64 platform variants should convert to mswin64
        assert_eq!(
            Platform::new("mswin64").unwrap().generic(),
            Platform::mswin64()
        );

        // 32-bit MinGW platform variants should convert to universal-mingw
        assert_eq!(
            Platform::new("i386-mingw32").unwrap().generic(),
            Platform::universal_mingw()
        );
        assert_eq!(
            Platform::new("x86-mingw32").unwrap().generic(),
            Platform::universal_mingw()
        );

        // 64-bit MinGW platform variants should convert to universal-mingw
        assert_eq!(
            Platform::new("x64-mingw32").unwrap().generic(),
            Platform::universal_mingw()
        );

        // x64 MinGW UCRT platform variants should convert to universal-mingw
        assert_eq!(
            Platform::new("x64-mingw-ucrt").unwrap().generic(),
            Platform::universal_mingw()
        );

        // aarch64 MinGW UCRT platform variants should convert to universal-mingw
        assert_eq!(
            Platform::new("aarch64-mingw-ucrt").unwrap().generic(),
            Platform::universal_mingw()
        );
    }
}
