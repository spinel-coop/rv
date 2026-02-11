use current_platform::CURRENT_PLATFORM;
#[cfg(test)]
use proptest::prelude::*;
#[cfg(test)]
use proptest_derive::Arbitrary;

/// Error returned when the current platform is not supported by rv.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("rv does not (yet) support your platform ({platform}). Sorry :(")]
pub struct UnsupportedPlatformError {
    pub platform: String,
}

/// Represents the host platforms that rv supports.
///
/// Using an enum with no wildcard fallback ensures the compiler enforces
/// exhaustive handling. Adding a new platform variant (e.g., `WindowsAarch64`)
/// will produce compiler errors at every call site until all methods handle it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(test, derive(Arbitrary))]
pub enum HostPlatform {
    MacosAarch64,
    MacosX86_64,
    LinuxX86_64,
    LinuxMuslX86_64,
    LinuxAarch64,
    LinuxMuslAarch64,
    WindowsX86_64,
}

impl HostPlatform {
    /// Detect the current host platform.
    ///
    /// Checks the `RV_TEST_PLATFORM` env var first (for testing), then falls
    /// back to the compile-time `CURRENT_PLATFORM`.
    pub fn current() -> Result<Self, UnsupportedPlatformError> {
        if let Ok(platform) = std::env::var("RV_TEST_PLATFORM") {
            Self::from_target_triple(&platform)
        } else {
            Self::from_target_triple(CURRENT_PLATFORM)
        }
    }

    /// Parse a Rust target triple into a `HostPlatform`.
    pub fn from_target_triple(triple: &str) -> Result<Self, UnsupportedPlatformError> {
        match triple {
            "aarch64-apple-darwin" => Ok(Self::MacosAarch64),
            "x86_64-apple-darwin" => Ok(Self::MacosX86_64),
            "x86_64-unknown-linux-gnu" => Ok(Self::LinuxX86_64),
            "x86_64-unknown-linux-musl" => Ok(Self::LinuxMuslX86_64),
            "aarch64-unknown-linux-gnu" => Ok(Self::LinuxAarch64),
            "aarch64-unknown-linux-musl" => Ok(Self::LinuxMuslAarch64),
            "x86_64-pc-windows-msvc" => Ok(Self::WindowsX86_64),
            other => Err(UnsupportedPlatformError {
                platform: other.to_string(),
            }),
        }
    }

    /// The normalized OS name used for filtering ruby releases.
    pub fn os(&self) -> &'static str {
        match self {
            Self::MacosAarch64 | Self::MacosX86_64 => "macos",
            Self::LinuxX86_64 | Self::LinuxAarch64 => "linux",
            Self::LinuxMuslX86_64 | Self::LinuxMuslAarch64 => "linux-musl",
            Self::WindowsX86_64 => "windows",
        }
    }

    /// The normalized architecture name used for filtering ruby releases.
    pub fn arch(&self) -> &'static str {
        match self {
            Self::MacosAarch64 | Self::LinuxAarch64 | Self::LinuxMuslAarch64 => "aarch64",
            Self::MacosX86_64 | Self::LinuxX86_64 | Self::LinuxMuslX86_64 | Self::WindowsX86_64 => {
                "x86_64"
            }
        }
    }

    /// The architecture string used in ruby release asset filenames.
    ///
    /// For example, `ruby-3.4.5.arm64_sonoma.tar.gz` has arch str `"arm64_sonoma"`.
    pub fn ruby_arch_str(&self) -> &'static str {
        match self {
            Self::MacosAarch64 => "arm64_sonoma",
            Self::MacosX86_64 => "ventura",
            Self::LinuxX86_64 => "x86_64_linux",
            Self::LinuxMuslX86_64 => "x86_64_linux_musl",
            Self::LinuxAarch64 => "arm64_linux",
            Self::LinuxMuslAarch64 => "arm64_linux_musl",
            Self::WindowsX86_64 => "x64",
        }
    }

    /// The archive file extension for this platform's ruby downloads.
    pub fn archive_ext(&self) -> &'static str {
        match self {
            Self::MacosAarch64
            | Self::MacosX86_64
            | Self::LinuxX86_64
            | Self::LinuxMuslX86_64
            | Self::LinuxAarch64
            | Self::LinuxMuslAarch64 => "tar.gz",
            Self::WindowsX86_64 => "7z",
        }
    }

    /// Whether this is a Windows platform.
    pub fn is_windows(&self) -> bool {
        matches!(self, Self::WindowsX86_64)
    }

    /// Parse from a ruby release asset arch string (e.g., `"arm64_sonoma"`, `"x64"`).
    pub fn from_ruby_arch_str(s: &str) -> Result<Self, UnsupportedPlatformError> {
        match s {
            "arm64_sonoma" => Ok(Self::MacosAarch64),
            "ventura" | "sequoia" => Ok(Self::MacosX86_64),
            "x86_64_linux" => Ok(Self::LinuxX86_64),
            "x86_64_linux_musl" => Ok(Self::LinuxMuslX86_64),
            "arm64_linux" => Ok(Self::LinuxAarch64),
            "arm64_linux_musl" => Ok(Self::LinuxMuslAarch64),
            "x64" => Ok(Self::WindowsX86_64),
            other => Err(UnsupportedPlatformError {
                platform: other.to_string(),
            }),
        }
    }

    /// All supported platforms.
    ///
    /// **Maintainer note:** When adding a new variant, add it here too.
    /// The exhaustive matches in every other method will force a compiler
    /// error when you add a variant, bringing you into this file.
    pub fn all() -> &'static [Self] {
        &[
            Self::MacosAarch64,
            Self::MacosX86_64,
            Self::LinuxX86_64,
            Self::LinuxMuslX86_64,
            Self::LinuxAarch64,
            Self::LinuxMuslAarch64,
            Self::WindowsX86_64,
        ]
    }

    /// The Rust target triple for this platform.
    pub fn target_triple(&self) -> &'static str {
        match self {
            Self::MacosAarch64 => "aarch64-apple-darwin",
            Self::MacosX86_64 => "x86_64-apple-darwin",
            Self::LinuxX86_64 => "x86_64-unknown-linux-gnu",
            Self::LinuxMuslX86_64 => "x86_64-unknown-linux-musl",
            Self::LinuxAarch64 => "aarch64-unknown-linux-gnu",
            Self::LinuxMuslAarch64 => "aarch64-unknown-linux-musl",
            Self::WindowsX86_64 => "x86_64-pc-windows-msvc",
        }
    }

    /// The full archive suffix for this platform (e.g., `".arm64_sonoma.tar.gz"`).
    pub fn archive_suffix(&self) -> &'static str {
        match self {
            Self::MacosAarch64 => ".arm64_sonoma.tar.gz",
            Self::MacosX86_64 => ".ventura.tar.gz",
            Self::LinuxX86_64 => ".x86_64_linux.tar.gz",
            Self::LinuxMuslX86_64 => ".x86_64_linux_musl.tar.gz",
            Self::LinuxAarch64 => ".arm64_linux.tar.gz",
            Self::LinuxMuslAarch64 => ".arm64_linux_musl.tar.gz",
            Self::WindowsX86_64 => ".x64.7z",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_target_triple_all_platforms() {
        let cases = [
            ("aarch64-apple-darwin", HostPlatform::MacosAarch64),
            ("x86_64-apple-darwin", HostPlatform::MacosX86_64),
            ("x86_64-unknown-linux-gnu", HostPlatform::LinuxX86_64),
            ("aarch64-unknown-linux-gnu", HostPlatform::LinuxAarch64),
            ("x86_64-pc-windows-msvc", HostPlatform::WindowsX86_64),
        ];
        for (triple, expected) in cases {
            assert_eq!(
                HostPlatform::from_target_triple(triple).unwrap(),
                expected,
                "Failed for triple: {triple}"
            );
        }
    }

    #[test]
    fn test_from_target_triple_unknown_returns_error() {
        let err = HostPlatform::from_target_triple("sparc-sun-solaris").unwrap_err();
        assert_eq!(err.platform, "sparc-sun-solaris");
    }

    #[test]
    fn test_current_respects_rv_test_platform() {
        // SAFETY: Single-threaded test context.
        unsafe { std::env::set_var("RV_TEST_PLATFORM", "x86_64-pc-windows-msvc") };
        let hp = HostPlatform::current().unwrap();
        unsafe { std::env::remove_var("RV_TEST_PLATFORM") };

        assert_eq!(hp, HostPlatform::WindowsX86_64);
    }

    #[test]
    fn test_round_trip_target_triple() {
        for hp in HostPlatform::all() {
            let round_tripped = HostPlatform::from_target_triple(hp.target_triple()).unwrap();
            assert_eq!(*hp, round_tripped);
        }
    }

    #[test]
    fn test_os() {
        assert_eq!(HostPlatform::MacosAarch64.os(), "macos");
        assert_eq!(HostPlatform::MacosX86_64.os(), "macos");
        assert_eq!(HostPlatform::LinuxX86_64.os(), "linux");
        assert_eq!(HostPlatform::LinuxAarch64.os(), "linux");
        assert_eq!(HostPlatform::WindowsX86_64.os(), "windows");
    }

    #[test]
    fn test_arch() {
        assert_eq!(HostPlatform::MacosAarch64.arch(), "aarch64");
        assert_eq!(HostPlatform::MacosX86_64.arch(), "x86_64");
        assert_eq!(HostPlatform::LinuxX86_64.arch(), "x86_64");
        assert_eq!(HostPlatform::LinuxAarch64.arch(), "aarch64");
        assert_eq!(HostPlatform::WindowsX86_64.arch(), "x86_64");
    }

    #[test]
    fn test_ruby_arch_str() {
        assert_eq!(HostPlatform::MacosAarch64.ruby_arch_str(), "arm64_sonoma");
        assert_eq!(HostPlatform::MacosX86_64.ruby_arch_str(), "ventura");
        assert_eq!(HostPlatform::LinuxX86_64.ruby_arch_str(), "x86_64_linux");
        assert_eq!(HostPlatform::LinuxAarch64.ruby_arch_str(), "arm64_linux");
        assert_eq!(HostPlatform::WindowsX86_64.ruby_arch_str(), "x64");
    }

    #[test]
    fn test_archive_ext() {
        assert_eq!(HostPlatform::MacosAarch64.archive_ext(), "tar.gz");
        assert_eq!(HostPlatform::MacosX86_64.archive_ext(), "tar.gz");
        assert_eq!(HostPlatform::LinuxX86_64.archive_ext(), "tar.gz");
        assert_eq!(HostPlatform::LinuxAarch64.archive_ext(), "tar.gz");
        assert_eq!(HostPlatform::WindowsX86_64.archive_ext(), "7z");
    }

    #[test]
    fn test_is_windows() {
        assert!(!HostPlatform::MacosAarch64.is_windows());
        assert!(!HostPlatform::LinuxX86_64.is_windows());
        assert!(HostPlatform::WindowsX86_64.is_windows());
    }

    #[test]
    fn test_from_ruby_arch_str() {
        let cases = [
            ("arm64_sonoma", HostPlatform::MacosAarch64),
            ("ventura", HostPlatform::MacosX86_64),
            ("sequoia", HostPlatform::MacosX86_64),
            ("x86_64_linux", HostPlatform::LinuxX86_64),
            ("arm64_linux", HostPlatform::LinuxAarch64),
            ("x64", HostPlatform::WindowsX86_64),
        ];
        for (arch_str, expected) in cases {
            assert_eq!(
                HostPlatform::from_ruby_arch_str(arch_str).unwrap(),
                expected,
                "Failed for arch_str: {arch_str}"
            );
        }
    }

    #[test]
    fn test_from_ruby_arch_str_unknown_returns_error() {
        let err = HostPlatform::from_ruby_arch_str("unknown_platform").unwrap_err();
        assert_eq!(err.platform, "unknown_platform");
    }

    #[test]
    fn test_all_has_no_duplicates_and_round_trips() {
        let all = HostPlatform::all();
        // If you add a variant, update all() — this catches duplicates and
        // verifies every entry is a valid, distinct platform.
        let mut seen = std::collections::HashSet::new();
        for hp in all {
            assert!(seen.insert(hp), "Duplicate in all(): {hp:?}");
            assert_eq!(
                HostPlatform::from_target_triple(hp.target_triple()).unwrap(),
                *hp
            );
        }
    }

    #[test]
    fn test_archive_suffix() {
        assert_eq!(
            HostPlatform::MacosAarch64.archive_suffix(),
            ".arm64_sonoma.tar.gz"
        );
        assert_eq!(
            HostPlatform::MacosX86_64.archive_suffix(),
            ".ventura.tar.gz"
        );
        assert_eq!(
            HostPlatform::LinuxX86_64.archive_suffix(),
            ".x86_64_linux.tar.gz"
        );
        assert_eq!(
            HostPlatform::LinuxAarch64.archive_suffix(),
            ".arm64_linux.tar.gz"
        );
        assert_eq!(HostPlatform::WindowsX86_64.archive_suffix(), ".x64.7z");
    }

    #[test]
    fn test_round_trip_ruby_arch_str() {
        for hp in HostPlatform::all() {
            let round_tripped = HostPlatform::from_ruby_arch_str(hp.ruby_arch_str()).unwrap();
            assert_eq!(*hp, round_tripped);
        }
    }

    proptest! {
        /// If this test fails, you forgot to add your new variant of `HostPlatform`
        /// to `HostPlatform::all`
        #[test]
        fn platform_in_list_of_all_platforms(host_platform: HostPlatform) {
            assert!(HostPlatform::all().contains(&host_platform))
        }
    }
}
