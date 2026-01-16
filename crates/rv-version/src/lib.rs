#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum VersionError {
    #[error("Malformed version number string {version}")]
    MalformedVersion { version: String },
    #[error("Invalid segment in version: {segment}")]
    InvalidSegment { segment: String },
    #[error("Version cannot contain newlines: {version}")]
    ContainsNewlines { version: String },
    #[error("Version cannot contain consecutive dots: {version}")]
    ConsecutiveDots { version: String },
    #[error("Version cannot be pure alphabetic: {version}")]
    PureAlphabetic { version: String },
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum VersionSegment {
    Number(u32),
    String(String),
}

impl VersionSegment {
    pub fn is_zero(&self) -> bool {
        matches!(self, Self::Number(0))
    }

    pub fn is_string(&self) -> bool {
        matches!(self, Self::String(_))
    }

    pub fn is_number(&self) -> bool {
        matches!(self, Self::Number(_))
    }
}

impl std::fmt::Display for VersionSegment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VersionSegment::Number(n) => write!(f, "{n}"),
            VersionSegment::String(s) => write!(f, "{s}"),
        }
    }
}

#[derive(Debug, Clone, Eq)]
pub struct Version {
    pub version: String,
    pub segments: Vec<VersionSegment>,
}

impl Version {
    pub fn new(version: impl AsRef<str>) -> Result<Self, VersionError> {
        let normalized = Self::normalize_version(version.as_ref())?;
        let segments = Self::parse_segments(&normalized)?;
        Ok(Self {
            version: normalized,
            segments,
        })
    }

    fn normalize_version(version: &str) -> Result<String, VersionError> {
        match version.trim() {
            "" => Ok("0".into()),

            // Check for invalid characters and patterns
            v if v.lines().count() > 1 => Err(VersionError::ContainsNewlines { version: v.into() }),
            v if v.contains("..") => Err(VersionError::ConsecutiveDots { version: v.into() }),

            // Check for obvious junk
            v if v.chars().all(|c| c.is_alphabetic()) => {
                Err(VersionError::PureAlphabetic { version: v.into() })
            }

            // Check for trailing dots or spaces within
            v if v.ends_with('.') || v.contains(' ') => {
                Err(VersionError::MalformedVersion { version: v.into() })
            }

            v => Ok(v.into()),
        }
    }

    fn parse_segments(version: &str) -> Result<Vec<VersionSegment>, VersionError> {
        let mut segments = Vec::new();
        let mut current_segment = String::new();
        let chars = version.chars().peekable();

        for ch in chars {
            match ch {
                '.' => {
                    if !current_segment.is_empty() {
                        segments.push(Self::parse_segment(&current_segment)?);
                        current_segment.clear();
                    }
                }
                '-' => {
                    if !current_segment.is_empty() {
                        segments.push(Self::parse_segment(&current_segment)?);
                        current_segment.clear();
                    }
                    // Dash indicates prerelease, add "pre" marker
                    segments.push(VersionSegment::String("pre".to_string()));
                }
                _ => {
                    current_segment.push(ch);
                }
            }
        }

        if !current_segment.is_empty() {
            segments.push(Self::parse_segment(&current_segment)?);
        }

        if segments.is_empty() {
            segments.push(VersionSegment::Number(0));
        }

        Ok(segments)
    }

    fn parse_segment(segment: &str) -> Result<VersionSegment, VersionError> {
        if let Ok(num) = segment.parse::<u32>() {
            Ok(VersionSegment::Number(num))
        } else if segment.chars().all(|c| c.is_alphanumeric()) {
            Ok(VersionSegment::String(segment.to_string()))
        } else {
            Err(VersionError::InvalidSegment {
                segment: segment.to_string(),
            })
        }
    }

    pub fn is_prerelease(&self) -> bool {
        self.segments.iter().any(|seg| seg.is_string())
    }

    pub fn canonical_segments(&self) -> Vec<VersionSegment> {
        // Step 1: Split on the first string segment
        let index = self
            .segments
            .iter()
            .position(|s| s.is_string())
            .unwrap_or(self.segments.len());

        let parts: [_; 2] = self.segments.split_at(index).into();

        // Step 2: seek behind from each tail and remove contigous zero chains.
        parts
            .iter()
            .flat_map(|part| {
                let mut part = part.to_vec();
                let last_nonzero_index = part.iter().rposition(|s| !s.is_zero()).unwrap_or(0);
                part.truncate(1 + last_nonzero_index); // `1 +` to keep at least one element.
                part
            })
            .collect::<Vec<_>>()
    }

    pub fn release(&self) -> Self {
        let segments = self
            .segments
            .clone()
            .into_iter()
            .take_while(|s| s.is_number())
            .collect::<Vec<_>>();

        Self::from(segments)
    }

    pub fn bump(&self) -> Version {
        let mut segments = self.segments.clone();

        // Remove all trailing string segments (prerelease parts)
        while segments.last().is_some_and(|s| s.is_string()) {
            segments.pop();
        }

        // If there's more than one segment left, remove the last one
        if segments.len() > 1 {
            segments.pop();
        }

        // Increment the last remaining segment
        if let Some(last_segment) = segments.last_mut()
            && let VersionSegment::Number(num) = last_segment
        {
            *last_segment = VersionSegment::Number(*num + 1);
        }

        Self::from(segments)
    }

    fn from(segments: Vec<VersionSegment>) -> Self {
        if segments.is_empty() {
            Self::default()
        } else {
            let version = segments
                .iter()
                .map(|s| s.to_string())
                .collect::<Vec<_>>()
                .join(".");

            Self { version, segments }
        }
    }

    fn split_alphanumeric(s: &str) -> Vec<String> {
        let mut parts = Vec::new();
        let mut current = String::new();
        let mut last_was_digit = false;

        for ch in s.chars() {
            let is_digit = ch.is_ascii_digit();

            if !current.is_empty() && last_was_digit != is_digit {
                parts.push(current.clone());
                current.clear();
            }

            current.push(ch);
            last_was_digit = is_digit;
        }

        if !current.is_empty() {
            parts.push(current);
        }

        parts
    }
}

impl Default for Version {
    fn default() -> Self {
        Version {
            version: "0".to_string(),
            segments: vec![VersionSegment::Number(0)],
        }
    }
}

impl PartialEq for Version {
    fn eq(&self, other: &Self) -> bool {
        self.canonical_segments() == other.canonical_segments()
    }
}

impl std::hash::Hash for Version {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.canonical_segments().hash(state);
    }
}

impl std::fmt::Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.version)
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Version {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        use std::cmp::Ordering;

        let self_segments = self.canonical_segments();
        let other_segments = other.canonical_segments();

        let max_len = self_segments.len().max(other_segments.len());

        for i in 0..max_len {
            let self_seg = self_segments.get(i).unwrap_or(&VersionSegment::Number(0));
            let other_seg = other_segments.get(i).unwrap_or(&VersionSegment::Number(0));

            match (self_seg, other_seg) {
                (VersionSegment::Number(a), VersionSegment::Number(b)) => match a.cmp(b) {
                    Ordering::Equal => continue,
                    other => return other,
                },
                (VersionSegment::Number(_), VersionSegment::String(_)) => return Ordering::Greater,
                (VersionSegment::String(_), VersionSegment::Number(_)) => return Ordering::Less,
                (VersionSegment::String(a), VersionSegment::String(b)) => {
                    // Handle mixed alphanumeric comparison like "a10" vs "a9"
                    // TODO: there should be no mixed alphanumeric segments
                    let a_parts = Self::split_alphanumeric(a);
                    let b_parts = Self::split_alphanumeric(b);

                    for (a_part, b_part) in a_parts.iter().zip(b_parts.iter()) {
                        match (a_part.parse::<u32>(), b_part.parse::<u32>()) {
                            (Ok(num_a), Ok(num_b)) => match num_a.cmp(&num_b) {
                                Ordering::Equal => continue,
                                other => return other,
                            },
                            _ => match a_part.cmp(b_part) {
                                Ordering::Equal => continue,
                                other => return other,
                            },
                        }
                    }

                    // If all parts are equal, compare length
                    match a_parts.len().cmp(&b_parts.len()) {
                        Ordering::Equal => continue,
                        other => return other,
                    }
                }
            }
        }

        Ordering::Equal
    }
}

impl std::str::FromStr for Version {
    type Err = VersionError;

    fn from_str(s: &str) -> Result<Self, VersionError> {
        Version::new(s)
    }
}

#[cfg(test)]
mod tests {
    use std::cmp::Ordering;

    use super::*;

    #[track_caller]
    fn v(version: &str) -> Version {
        Version::new(version).unwrap()
    }

    #[test]
    fn test_version_creation() {
        assert_eq!(v("1.0").version, "1.0");
        assert_eq!(v("1.2.3").version, "1.2.3");
        assert_eq!(v("5.2.4").version, "5.2.4");
    }

    #[test]
    fn test_whitespace_handling() {
        assert_eq!(v("1.0 ").version, "1.0");
        assert_eq!(v(" 1.0 ").version, "1.0");
        assert_eq!(v("1.0\n").version, "1.0");
        assert_eq!(v("\n1.0\n").version, "1.0");
    }

    #[test]
    fn test_empty_string_defaults_to_zero() {
        assert_eq!(v("").version, "0");
        assert_eq!(v("   ").version, "0");
        assert_eq!(v(" ").version, "0");
        assert_eq!(v("\t").version, "0");
    }

    #[test]
    fn test_invalid_versions() {
        assert!(Version::new("junk").is_err());
        assert!(Version::new("1.0\n2.0").is_err());
        assert!(Version::new("1..2").is_err());
        assert!(Version::new("1.2 3.4").is_err());
        assert!(
            Version::new("2.3422222.222.222222222.22222.ads0as.dasd0.ddd2222.2.qd3e.").is_err()
        );
    }

    #[test]
    fn test_version_equality() {
        assert_eq!(v("1.0"), v("1.0.0"));
        assert_eq!(v(""), v("0"));
    }

    #[test]
    fn test_version_ordering() {
        assert!(v("1.8.2") > v("0.0.0"));
        assert!(v("1.8.2") > v("1.8.2.a"));
        assert!(v("1.8.2.b") > v("1.8.2.a"));
        assert!(v("1.8.2.a10") > v("1.8.2.a9"));
    }

    #[test]
    fn test_prerelease_detection() {
        assert!(v("1.2.0.a").is_prerelease());
        assert!(v("2.9.b").is_prerelease());
        assert!(v("22.1.50.0.d").is_prerelease());
        assert!(v("1.2.d.42").is_prerelease());
        assert!(v("1.A").is_prerelease());
        assert!(v("1-1").is_prerelease());
        assert!(v("1-a").is_prerelease());

        assert!(!v("1.2.0").is_prerelease());
        assert!(!v("2.9").is_prerelease());
        assert!(!v("22.1.50.0").is_prerelease());
    }

    #[test]
    fn test_segments() {
        assert_eq!(
            v("9.8.7").segments,
            vec![
                VersionSegment::Number(9),
                VersionSegment::Number(8),
                VersionSegment::Number(7)
            ]
        );
        assert_eq!(
            v("1.0.0").segments,
            vec![
                VersionSegment::Number(1),
                VersionSegment::Number(0),
                VersionSegment::Number(0)
            ]
        );
        assert_eq!(
            v("1.0.0.a.1.0").segments,
            vec![
                VersionSegment::Number(1),
                VersionSegment::Number(0),
                VersionSegment::Number(0),
                VersionSegment::String("a".to_string()),
                VersionSegment::Number(1),
                VersionSegment::Number(0),
            ]
        );
        assert_eq!(
            v("1.2.3-1").segments,
            vec![
                VersionSegment::Number(1),
                VersionSegment::Number(2),
                VersionSegment::Number(3),
                VersionSegment::String("pre".to_string()),
                VersionSegment::Number(1),
            ]
        );
    }

    #[test]
    fn test_canonical_segments() {
        assert_eq!(v("0").canonical_segments(), vec![VersionSegment::Number(0)]);
        assert_eq!(
            v("0-rc").canonical_segments(),
            vec![
                VersionSegment::Number(0),
                VersionSegment::String("pre".to_string()),
                VersionSegment::String("rc".to_string())
            ]
        );
        assert_eq!(
            v("1.0.0").canonical_segments(),
            vec![VersionSegment::Number(1)]
        );
        assert_eq!(
            v("1.0.1").canonical_segments(),
            vec![
                VersionSegment::Number(1),
                VersionSegment::Number(0),
                VersionSegment::Number(1)
            ]
        );
        assert_eq!(
            v("1.0.0.a.1.0").canonical_segments(),
            vec![
                VersionSegment::Number(1),
                VersionSegment::String("a".to_string()),
                VersionSegment::Number(1)
            ]
        );
        assert_eq!(
            v("1.0.1-rc1").canonical_segments(),
            vec![
                VersionSegment::Number(1),
                VersionSegment::Number(0),
                VersionSegment::Number(1),
                VersionSegment::String("pre".to_string()),
                VersionSegment::String("rc1".to_string()),
            ]
        );
        assert_eq!(
            v("0.0.beta.1").canonical_segments(),
            vec![
                VersionSegment::Number(0),
                VersionSegment::String("beta".to_string()),
                VersionSegment::Number(1),
            ]
        );
        assert_eq!(
            v("1.2.3-1").canonical_segments(),
            vec![
                VersionSegment::Number(1),
                VersionSegment::Number(2),
                VersionSegment::Number(3),
                VersionSegment::String("pre".to_string()),
                VersionSegment::Number(1)
            ]
        );
    }

    #[test]
    fn test_release_conversion() {
        assert_eq!(v("1.2.0.a").release(), v("1.2.0"));
        assert_eq!(v("1.1.rc10").release(), v("1.1"));
        assert_eq!(v("1.9.3.alpha.5").release(), v("1.9.3"));
        assert_eq!(v("1.9.3").release(), v("1.9.3"));
    }

    #[test]
    fn test_version_bump() {
        assert_eq!(v("5.2.4").bump(), v("5.3"));
        assert_eq!(v("5.2.4.a").bump(), v("5.3"));
        assert_eq!(v("5.2.4.a10").bump(), v("5.3"));
        assert_eq!(v("5.0.0").bump(), v("5.1"));
        assert_eq!(v("5").bump(), v("6"));
    }

    #[test]
    fn test_semver_style_comparisons() {
        assert!(v("1.0.0-alpha") < v("1.0.0"));
        assert!(v("1.0.0-alpha.1") < v("1.0.0-beta.2"));
        assert!(v("1.0.0-beta.2") < v("1.0.0-beta.11"));
        assert!(v("1.0.0-beta.11") < v("1.0.0-rc.1"));
        assert!(v("1.0.0-rc1") < v("1.0.0"));
    }

    #[test]
    fn test_ord() {
        assert_eq!(Ordering::Equal, v("1.0").cmp(&v("1.0.0")));
        assert_eq!(Ordering::Greater, v("1.0").cmp(&v("1.0.a")));
        assert_eq!(Ordering::Greater, v("1.8.2").cmp(&v("0.0.0")));
        assert_eq!(Ordering::Greater, v("1.8.2").cmp(&v("1.8.2.a")));
        assert_eq!(Ordering::Greater, v("1.8.2.b").cmp(&v("1.8.2.a")));
        assert_eq!(Ordering::Less, v("1.8.2.a").cmp(&v("1.8.2")));
        assert_eq!(Ordering::Greater, v("1.8.2.a10").cmp(&v("1.8.2.a9")));
        assert_eq!(Ordering::Equal, v("").cmp(&v("0")));

        assert_eq!(Ordering::Equal, v("0.beta.1").cmp(&v("0.0.beta.1")));
        assert_eq!(Ordering::Less, v("0.0.beta").cmp(&v("0.0.beta.1")));
        assert_eq!(Ordering::Less, v("0.0.beta").cmp(&v("0.beta.1")));

        assert_eq!(Ordering::Less, v("5.a").cmp(&v("5.0.0.rc2")));
        assert_eq!(Ordering::Greater, v("5.x").cmp(&v("5.0.0.rc2")));
    }
}
