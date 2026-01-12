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

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
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
        let data = "2.2.2
            2.3.2
            2.0.5
            2.1.0
            2.1.1
            2.1.2
            1.2.6
            2.0.0
            2.0.1
            2.0.2
            2.0.4
            1.2.1
            1.2.2
            1.2.3
            1.2.4
            1.2.5
            1.2.0
            1.1.2
            1.1.3
            1.1.4
            1.1.5
            1.1.6
            0.9.3
            0.9.4
            0.9.4.1
            0.9.5
            1.0.0
            1.1.0
            1.1.1
            0.14.3
            0.14.4
            0.8.0
            0.8.5
            0.9.0
            0.9.1
            0.9.2
            0.12.1
            0.13.0
            0.13.1
            0.14.1
            0.14.2
            0.10.0
            0.10.1
            0.11.0
            0.11.1
            0.12.0
            2.3.3
            2.3.4
            2.2.3
            2.3.5
            3.0.0-beta
            3.0.0-beta2
            3.0.0-beta3
            2.3.6
            2.3.7
            2.3.8-pre1
            2.3.8
            3.0.0-beta4
            3.0.0-rc
            3.0.0-rc2
            3.0.0
            2.3.9-pre
            2.3.9
            2.3.10
            3.0.1
            3.0.2
            3.0.3
            3.0.4-rc1
            2.3.11
            3.0.4
            3.0.5-rc1
            3.0.5
            3.0.6-rc1
            3.0.6-rc2
            3.0.6
            3.0.7-rc1
            3.0.7-rc2
            3.0.7
            3.1.0-beta1
            3.1.0-rc1
            3.0.8-rc1
            3.0.8-rc2
            3.0.8-rc4
            3.0.8
            3.1.0-rc2
            2.3.12
            3.0.9-rc1
            3.1.0-rc3
            3.0.9-rc3
            3.1.0-rc4
            3.0.9-rc4
            3.0.9-rc5
            3.0.9
            3.1.0-rc5
            3.0.10-rc1
            2.3.14
            3.0.10
            3.1.0-rc6
            3.1.0-rc8
            3.1.0
            3.1.1-rc1
            3.1.1-rc2
            3.1.1-rc3
            3.1.1
            3.1.2-rc1
            3.1.2-rc2
            3.0.11
            3.1.2
            3.1.3
            3.2.0-rc1
            3.2.0-rc2
            3.2.0
            3.2.1
            3.0.12-rc1
            3.1.4-rc1
            3.2.2-rc1
            3.0.12
            3.1.4
            3.2.2
            3.2.3-rc1
            3.2.3-rc2
            3.2.3
            3.0.13-rc1
            3.1.5-rc1
            3.2.4-rc1
            3.0.13
            3.1.5
            3.2.4
            3.2.5
            3.0.14
            3.1.6
            3.2.6
            3.0.15
            3.2.7-rc1
            3.0.16
            3.1.7
            3.2.7
            3.2.8-rc1
            3.2.8-rc2
            3.0.17
            3.1.8
            3.2.8
            3.2.9-rc1
            3.2.9-rc2
            3.2.9-rc3
            3.2.9
            3.0.18
            3.1.9
            3.2.10
            2.3.15
            3.0.19
            3.1.10
            3.2.11
            2.3.16
            3.0.20
            2.3.17
            3.1.11
            3.2.12
            4.0.0-beta1
            3.2.13-rc1
            3.2.13-rc2
            2.3.18
            3.1.12
            3.2.13
            4.0.0-rc1
            4.0.0-rc2
            4.0.0
            3.2.14-rc1
            3.2.14-rc2
            3.2.14
            3.2.15-rc1
            3.2.15-rc2
            3.2.15-rc3
            3.2.15
            4.0.1-rc1
            4.0.1-rc2
            4.0.1-rc3
            4.0.1-rc4
            4.0.1
            3.2.16
            4.0.2
            4.1.0-beta1
            4.0.3
            4.1.0-beta2
            3.2.17
            4.1.0-rc1
            4.0.4-rc1
            4.0.4
            4.1.0-rc2
            4.1.0
            4.1.1
            4.0.5
            3.2.18
            4.0.6-rc1
            4.1.2-rc1
            4.0.6-rc2
            4.1.2-rc2
            4.0.6-rc3
            4.1.2-rc3
            4.1.2
            4.0.6
            3.2.19
            4.0.7
            4.1.3
            4.0.8
            4.1.4
            4.1.5
            4.0.9
            4.0.10-rc1
            4.1.6-rc1
            4.2.0-beta1
            4.0.10-rc2
            4.1.6-rc2
            4.1.6
            4.0.10
            4.2.0-beta2
            3.2.20
            4.0.11
            4.1.7
            4.2.0-beta3
            4.2.0-beta4
            3.2.21
            4.0.12
            4.1.8
            4.0.11.1
            4.1.7.1
            4.2.0-rc1
            4.2.0-rc2
            4.2.0-rc3
            4.2.0
            4.0.13-rc1
            4.1.9-rc1
            4.1.9
            4.0.13
            4.2.1-rc1
            4.1.10-rc1
            4.2.1-rc2
            4.1.10-rc2
            4.2.1-rc3
            4.1.10-rc3
            4.2.1-rc4
            4.1.10-rc4
            4.2.1
            4.1.10
            4.1.11
            4.2.2
            3.2.22
            4.1.12-rc1
            4.2.3-rc1
            4.1.12
            4.2.3
            4.1.13-rc1
            4.2.4-rc1
            4.1.13
            4.2.4
            4.1.14-rc1
            4.2.5-rc1
            4.1.14-rc2
            4.2.5-rc2
            4.2.5
            4.1.14
            5.0.0-beta1
            3.2.22.1
            4.1.14.1
            4.2.5.1
            5.0.0-beta1
            5.0.0-beta2
            5.0.0-beta3
            4.2.5.2
            4.1.14.2
            3.2.22.2
            4.2.6-rc1
            4.1.15-rc1
            4.2.6
            4.1.15
            5.0.0-beta4
            5.0.0-rc1
            5.0.0-racecar1
            5.0.0-rc2
            5.0.0
            4.2.7-rc1
            4.1.16-rc1
            4.1.16
            4.2.7
            3.2.22.3
            4.2.7.1
            5.0.0.1
            3.2.22.4
            3.2.22.5
            5.0.1-rc1
            5.0.1-rc2
            5.0.1
            4.2.8-rc1
            4.2.8
            5.1.0-beta1
            5.0.2-rc1
            5.0.2
            5.1.0-rc1
            5.1.0-rc2
            5.1.0
            5.0.3
            5.1.1
            4.2.9-rc1
            5.0.4-rc1
            5.0.4
            4.2.9-rc2
            5.1.2-rc1
            4.2.9
            5.1.2
            5.1.3-rc1
            5.0.5-rc1
            5.1.3-rc2
            5.0.5-rc2
            5.0.5
            5.1.3-rc3
            5.1.3
            5.0.6-rc1
            5.1.4-rc1
            5.0.6
            5.1.4
            4.2.10-rc1
            4.2.10
            5.2.0-beta1
            5.2.0-beta2
            5.2.0-rc1
            5.1.5-rc1
            5.1.5
            5.2.0-rc2
            5.0.7
            5.1.6
            5.2.0
            5.2.1-rc1
            5.2.1
            4.2.11
            5.0.7.1
            5.1.6.1
            5.2.1.1
            5.2.2-rc1
            5.2.2
            6.0.0-beta1
            6.0.0-beta2
            4.2.11.1
            5.0.7.2
            5.1.6.2
            5.2.2.1
            6.0.0-beta3
            5.2.3-rc1
            5.1.7-rc1
            5.1.7
            5.2.3
            6.0.0-rc1
            6.0.0-rc2
            6.0.0
            6.0.1-rc1
            6.0.1
            5.2.4-rc1
            6.0.2-rc1
            5.2.4
            6.0.2-rc2
            6.0.2
            5.2.4.1
            6.0.2.1
            5.2.4.2
            6.0.2.2
            6.0.3-rc1
            6.0.3
            4.2.11.2
            4.2.11.3
            5.2.4.3
            6.0.3.1
            6.0.3.2
            5.2.4.4
            6.0.3.3
            6.0.3.4
            6.1.0-rc1
            6.1.0-rc2
            6.1.0
            6.1.1
            6.1.2
            5.2.4.5
            6.0.3.5
            6.1.2.1
            6.1.3
            5.2.5
            6.0.3.6
            6.1.3.1
            5.2.4.6
            6.1.3.2
            6.0.3.7
            5.2.6
            6.0.4
            6.1.4
            6.0.4.1
            6.1.4.1
            7.0.0-alpha1
            7.0.0-alpha2
            7.0.0-rc1
            7.0.0-rc2
            6.1.4.2
            6.0.4.2
            6.0.4.3
            6.1.4.3
            7.0.0-rc3
            6.0.4.4
            6.1.4.4
            7.0.0
            7.0.1
            7.0.2
            7.0.2.1
            6.1.4.5
            6.0.4.5
            5.2.6.1
            5.2.6.2
            6.0.4.6
            6.1.4.6
            7.0.2.2
            5.2.6.3
            6.0.4.7
            6.1.4.7
            7.0.2.3
            6.1.5
            5.2.7
            5.2.7.1
            6.0.4.8
            6.1.5.1
            7.0.2.4
            7.0.3
            6.1.6
            6.0.5
            5.2.8
            5.2.8.1
            6.0.5.1
            6.1.6.1
            7.0.3.1
            6.0.6
            6.1.7
            7.0.4
            6.0.6.1
            6.1.7.1
            7.0.4.1
            7.0.4.2
            6.1.7.2
            6.1.7.3
            7.0.4.3
            7.0.5
            6.1.7.4
            7.0.5.1
            7.0.6
            7.0.7
            6.1.7.5
            7.0.7.1
            6.1.7.6
            7.0.7.2
            7.0.8
            7.1.0-beta1
            7.1.0-rc1
            7.1.0-rc2
            7.1.0
            7.1.1
            7.1.2
            7.1.3
            6.1.7.7
            7.0.8.1
            7.1.3.1
            7.1.3.2
            7.0.8.2
            7.1.3.3
            7.0.8.3
            7.2.0-beta1
            6.1.7.8
            7.0.8.4
            7.1.3.4
            7.2.0-beta2
            7.2.0-beta3
            7.2.0-rc1
            7.2.0
            7.2.1
            7.1.4
            8.0.0-beta1
            7.0.8.5
            7.1.4.1
            7.2.1.1
            6.1.7.9
            8.0.0-rc1
            6.1.7.10
            7.0.8.6
            7.1.4.2
            7.2.1.2
            8.0.0-rc2
            7.1.5
            7.2.2
            8.0.0
            7.0.8.7
            7.1.5.1
            7.2.2.1
            8.0.0.1
            8.0.1
            8.0.2
            7.1.5.2
            7.2.2.2
            8.0.2.1
            8.1.0-beta1
            8.0.3
            8.1.0-rc1
            8.1.0
            7.0.10
            7.1.6
            7.2.3
            8.0.4
            8.1.1
            8.1.2";
        for input in data.lines() {
            let _v = GemVersion::from_str(input.trim()).unwrap();
        }
    }
}
