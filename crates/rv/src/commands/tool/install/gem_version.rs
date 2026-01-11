use std::str::FromStr;

use serde::{Deserialize, Serialize};

#[derive(Ord, PartialOrd, Eq, PartialEq, Serialize, Deserialize, Clone, Debug)]
pub struct GemVersion {
    pub major: u64,
    pub minor: Option<u64>,
    pub patch: Option<u64>,
    pub tiny: Option<u64>,
    pub prerelease: Option<String>,
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
    fn test_parse() {
        for (expected, input) in [
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
}
