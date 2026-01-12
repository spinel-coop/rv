use std::{cmp::Ordering, str::FromStr};

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct GemVersion {
    pub major: u64,
    pub minor: Option<u64>,
    pub patch: Option<u64>,
    pub tiny: Option<u64>,
    pub prerelease: Option<String>,
}

impl GemVersion {
    /// What's the next major version after this version?
    pub fn next_major(&self) -> Self {
        GemVersion {
            major: self.major + 1,
            minor: Default::default(),
            patch: Default::default(),
            tiny: Default::default(),
            prerelease: Default::default(),
        }
    }

    /// What's the next minor version after this version?
    pub fn next_minor(&self) -> Self {
        GemVersion {
            major: self.major,
            minor: Some(self.minor.unwrap_or_default() + 1),
            patch: Default::default(),
            tiny: Default::default(),
            prerelease: Default::default(),
        }
    }
}

#[cfg(test)]
mod next_version {
    use super::*;

    #[test]
    fn example_next_major() {
        // so ~>2.1.5 allows >=2.1.5, <3.0.0
        let v: GemVersion = "2.1.5".parse().unwrap();
        assert_eq!(v.next_major(), "3.0.0".parse().unwrap());
    }

    #[test]
    fn example_next_minor() {
        // ~> 0.4.3 allows >=0.4.3, <0.5
        let v: GemVersion = "0.4.3".parse().unwrap();
        assert_eq!(v.next_minor(), "0.5".parse().unwrap());
    }
}

impl PartialEq for GemVersion {
    fn eq(&self, other: &Self) -> bool {
        self.major == other.major
            && self.minor.unwrap_or_default() == other.minor.unwrap_or_default()
            && self.patch.unwrap_or_default() == other.patch.unwrap_or_default()
            && self.tiny.unwrap_or_default() == other.tiny.unwrap_or_default()
            && self.prerelease == other.prerelease
    }
}

impl Eq for GemVersion {}

impl Ord for GemVersion {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        use std::cmp::Ordering;

        if self.major != other.major {
            self.major.cmp(&other.major)
        } else if self.minor.unwrap_or_default() != other.minor.unwrap_or_default() {
            self.minor.cmp(&other.minor)
        } else if self.patch.unwrap_or_default() != other.patch.unwrap_or_default() {
            self.patch.cmp(&other.patch)
        } else if self.tiny.unwrap_or_default() != other.tiny.unwrap_or_default() {
            self.tiny.cmp(&other.tiny)
        } else {
            match (&self.prerelease, &other.prerelease) {
                (None, None) => Ordering::Equal,
                (None, Some(_prerelease)) => Ordering::Greater,
                (Some(_prerelease), None) => Ordering::Less,
                (prerelease, other_prerelease) => prerelease.cmp(other_prerelease),
            }
        }
    }
}

impl PartialOrd for GemVersion {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Debug, Clone, Eq, PartialEq, thiserror::Error)]
#[error("The string {input_string} is not a valid gem version because {why}")]
pub struct InvalidGemVersion {
    pub why: String,
    pub input_string: String,
}

impl std::fmt::Display for GemVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.major)?;
        if let Some(x) = &self.minor {
            write!(f, ".{x}")?
        }
        if let Some(x) = &self.patch {
            write!(f, ".{x}")?
        }
        if let Some(x) = &self.tiny {
            write!(f, ".{x}")?
        }
        if let Some(x) = &self.prerelease {
            write!(f, "-{x}")?
        }
        Ok(())
    }
}

impl FromStr for GemVersion {
    type Err = InvalidGemVersion;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut parts = s.split('-');
        let mut numbers = parts
            .next()
            .expect("'split' always has >=1 item")
            .split('.');
        let prerelease = parts.next().map(|s| s.to_owned());

        let major: u64 = numbers
            .next()
            .expect("'split' always has >=1 item")
            .parse()
            .map_err(|e: std::num::ParseIntError| InvalidGemVersion {
                why: e.to_string(),
                input_string: s.to_owned(),
            })?;

        let mut gv = GemVersion {
            major,
            minor: None,
            patch: None,
            tiny: None,
            prerelease,
        };

        // Minor
        let Some(minor) = numbers.next() else {
            return Ok(gv);
        };
        if minor.chars().any(|c| !c.is_numeric()) {
            gv.prerelease = Some(minor.to_owned());
            return Ok(gv);
        }
        gv.minor = Some(
            minor
                .parse()
                .map_err(|e: std::num::ParseIntError| InvalidGemVersion {
                    why: e.to_string(),
                    input_string: s.to_owned(),
                })?,
        );

        // Patch
        let Some(patch) = numbers.next() else {
            return Ok(gv);
        };
        if patch.chars().any(|c| !c.is_numeric()) {
            gv.prerelease = Some(patch.to_owned());
            return Ok(gv);
        }
        gv.patch = Some(
            patch
                .parse()
                .map_err(|e: std::num::ParseIntError| InvalidGemVersion {
                    why: e.to_string(),
                    input_string: s.to_owned(),
                })?,
        );

        // Tiny
        let Some(tiny) = numbers.next() else {
            return Ok(gv);
        };
        if tiny.chars().any(|c| !c.is_numeric()) {
            gv.prerelease = Some(tiny.to_owned());
            return Ok(gv);
        }
        gv.tiny = Some(
            tiny.parse()
                .map_err(|e: std::num::ParseIntError| InvalidGemVersion {
                    why: e.to_string(),
                    input_string: s.to_owned(),
                })?,
        );

        Ok(gv)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_equality() {
        // These should all be equal.
        let a: GemVersion = "1".parse().unwrap();
        let b: GemVersion = "1.0".parse().unwrap();
        let c: GemVersion = "1.0.0".parse().unwrap();
        assert_eq!(a, b);
        assert_eq!(a, c);
        assert_eq!(b, c);
        // So should these.
        let a: GemVersion = "0".parse().unwrap();
        let b: GemVersion = "0.0".parse().unwrap();
        let c: GemVersion = "0.0.0".parse().unwrap();
        assert_eq!(a, b);
        assert_eq!(a, c);
        assert_eq!(b, c);
    }

    #[test]
    fn test_ord() {
        // These should all be equal.
        let a: GemVersion = "1".parse().unwrap();
        let b: GemVersion = "1.0".parse().unwrap();
        let c: GemVersion = "1.0.0".parse().unwrap();
        assert_eq!(a.cmp(&b), std::cmp::Ordering::Equal);
        assert_eq!(a.cmp(&c), std::cmp::Ordering::Equal);
        assert_eq!(b.cmp(&c), std::cmp::Ordering::Equal);
        assert!(GemVersion::from_str("1").unwrap() < (GemVersion::from_str("1.1").unwrap()));
        assert!(GemVersion::from_str("1").unwrap() < (GemVersion::from_str("1.0.1").unwrap()));
        // Prereleases are, by definition, below the real release.
        assert!(GemVersion::from_str("1-a").unwrap() < (GemVersion::from_str("1").unwrap()));
        assert!(GemVersion::from_str("1.0-a").unwrap() < (GemVersion::from_str("1.0").unwrap()));
        assert!(GemVersion::from_str("1.0-a").unwrap() < (GemVersion::from_str("1.0-b").unwrap()));
        // Liste xample
        assert!(
            GemVersion::from_str("1.0.0.pre").unwrap()
                < (GemVersion::from_str("1.0.0.pre2").unwrap())
        );
        assert!(
            GemVersion::from_str("1.0.0.pre2").unwrap()
                < (GemVersion::from_str("1.0.0.rc").unwrap())
        );
        assert!(
            GemVersion::from_str("1.0.0.rc").unwrap()
                < (GemVersion::from_str("1.0.0.rc2").unwrap())
        );
        assert!(
            GemVersion::from_str("1.0.0.rc2").unwrap() < (GemVersion::from_str("1.0.0").unwrap())
        );
        assert!(
            GemVersion::from_str("1.0.0").unwrap() < (GemVersion::from_str("1.1.0.a").unwrap())
        );
        assert!(
            GemVersion::from_str("1.1.0.a").unwrap() < (GemVersion::from_str("1.1.0").unwrap())
        );

        // This should already be sorted.
        let list: Vec<GemVersion> = vec![
            "1.0.0.pre".parse().unwrap(),
            "1.0.0.pre2".parse().unwrap(),
            "1.0.0.rc".parse().unwrap(),
            "1.0.0.rc2".parse().unwrap(),
            "1.0.0".parse().unwrap(),
            "1.1.0.a".parse().unwrap(),
            "1.1.0".parse().unwrap(),
        ];
        let mut list_sorted = list.clone();
        list_sorted.sort();
        assert_eq!(list, list_sorted);
    }

    #[test]
    fn test_inequality() {
        assert_ne!(
            GemVersion::from_str("1").unwrap(),
            GemVersion::from_str("2").unwrap()
        );
        assert_ne!(
            GemVersion::from_str("1.1").unwrap(),
            GemVersion::from_str("1.2").unwrap()
        );
        assert_ne!(
            GemVersion::from_str("1.1.1").unwrap(),
            GemVersion::from_str("1.1.2").unwrap()
        );
        assert_ne!(
            GemVersion::from_str("1.1.1.1").unwrap(),
            GemVersion::from_str("1.1.1.2").unwrap()
        );
        assert_ne!(
            GemVersion::from_str("1-a").unwrap(),
            GemVersion::from_str("2-a").unwrap()
        );
        assert_ne!(
            GemVersion::from_str("1.1-a").unwrap(),
            GemVersion::from_str("1.2-a").unwrap()
        );
        assert_ne!(
            GemVersion::from_str("1.1.1-a").unwrap(),
            GemVersion::from_str("1.1.2-a").unwrap()
        );
        assert_ne!(
            GemVersion::from_str("1.1.1.1-a").unwrap(),
            GemVersion::from_str("1.1.1.2-a").unwrap()
        );
    }

    #[test]
    fn test_parse_weird_versions_found_on_gemserver() {
        for (expected, input) in [
            // These are all regression tests for specific weird versions I've seen on a gemserver somewhere.
            (
                GemVersion {
                    major: 3,
                    minor: Some(0),
                    patch: Some(0),
                    tiny: None,
                    prerelease: Some("beta".to_owned()),
                },
                "3.0.0.beta",
            ),
            (
                GemVersion {
                    major: 5,
                    minor: Some(0),
                    patch: Some(1),
                    tiny: Some(20140414130214),
                    prerelease: None,
                },
                "5.0.1.20140414130214",
            ),
        ] {
            let actual: GemVersion = input.parse().expect("Did not parse version");
            assert_eq!(actual, expected)
        }
    }

    #[test]
    fn parses_all_rails_versions() {
        // Here's every version of Rails, I guess we should just make sure they all parse.
        let data = include_str!("../../../../testdata/all_rails_versions");
        for input in data.lines() {
            let _v = GemVersion::from_str(input.trim()).unwrap();
        }
    }
}
