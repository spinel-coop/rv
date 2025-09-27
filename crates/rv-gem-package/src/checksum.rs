use sha1::{Digest as Sha1Digest, Sha1};
use sha2::{Sha256, Sha512};
use std::collections::HashMap;

use crate::error::ChecksumErrorKind;

/// Checksums for files in a gem package
#[derive(Debug, Clone, Default)]
pub struct Checksums {
    /// Map of algorithm name to file checksums
    /// Key: algorithm name (e.g., "SHA256", "SHA512")
    /// Value: map of file path to hexadecimal checksum string
    pub algorithms: HashMap<ChecksumAlgorithm, HashMap<String, String>>,
}

impl Checksums {
    /// Create a new empty checksums collection
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a checksum for a file
    pub fn add_checksum(&mut self, algorithm: ChecksumAlgorithm, file_path: &str, checksum: &str) {
        self.algorithms
            .entry(algorithm)
            .or_default()
            .insert(file_path.to_string(), checksum.to_string());
    }

    /// Get checksum for a specific file and algorithm
    pub fn get_checksum(&self, algorithm: ChecksumAlgorithm, file_path: &str) -> Option<&str> {
        self.algorithms
            .get(&algorithm)?
            .get(file_path)
            .map(|s| s.as_str())
    }

    /// Get all algorithms available
    pub fn algorithms(&self) -> impl Iterator<Item = ChecksumAlgorithm> {
        self.algorithms.keys().copied()
    }

    /// Get all files for a specific algorithm
    pub fn files_for_algorithm(
        &self,
        algorithm: ChecksumAlgorithm,
    ) -> Option<impl Iterator<Item = &str>> {
        self.algorithms
            .get(&algorithm)
            .map(|files| files.keys().map(|s| s.as_str()))
    }

    /// Check if checksums are empty
    pub fn is_empty(&self) -> bool {
        self.algorithms.is_empty()
    }
}

/// Supported checksum algorithms matching Ruby's implementation
#[derive(Debug, Clone, Copy, PartialEq, Hash, Eq)]
pub enum ChecksumAlgorithm {
    Sha1,
    Sha256,
    Sha512,
}

impl std::fmt::Display for ChecksumAlgorithm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Sha1 => "Sha1",
            Self::Sha256 => "Sha256",
            Self::Sha512 => "Sha512",
        }
        .fmt(f)
    }
}

impl std::str::FromStr for ChecksumAlgorithm {
    type Err = ChecksumErrorKind;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let algo = match s.to_lowercase().as_str() {
            "sha1" => Self::Sha1,
            "sha256" => Self::Sha256,
            "sha512" => Self::Sha512,
            other => {
                return Err(ChecksumErrorKind::UnsupportedAlgorithm {
                    algorithm: other.to_owned(),
                });
            }
        };
        Ok(algo)
    }
}

impl ChecksumAlgorithm {
    /// Get the algorithm name as used in Ruby
    pub fn name(&self) -> &'static str {
        match self {
            ChecksumAlgorithm::Sha1 => "SHA1",
            ChecksumAlgorithm::Sha256 => "SHA256",
            ChecksumAlgorithm::Sha512 => "SHA512",
        }
    }

    /// Parse algorithm from string
    pub fn from_name(name: &str) -> Option<Self> {
        match name.to_uppercase().as_str() {
            "SHA1" => Some(ChecksumAlgorithm::Sha1),
            "SHA256" => Some(ChecksumAlgorithm::Sha256),
            "SHA512" => Some(ChecksumAlgorithm::Sha512),
            _ => None,
        }
    }

    /// Get all supported algorithms
    pub fn all() -> &'static [ChecksumAlgorithm] {
        &[
            ChecksumAlgorithm::Sha1,
            ChecksumAlgorithm::Sha256,
            ChecksumAlgorithm::Sha512,
        ]
    }

    /// Calculate checksum for given data
    pub fn calculate(&self, data: &[u8]) -> String {
        match self {
            ChecksumAlgorithm::Sha1 => {
                let mut hasher = Sha1::new();
                hasher.update(data);
                format!("{:x}", hasher.finalize())
            }
            ChecksumAlgorithm::Sha256 => {
                let mut hasher = Sha256::new();
                hasher.update(data);
                format!("{:x}", hasher.finalize())
            }
            ChecksumAlgorithm::Sha512 => {
                let mut hasher = Sha512::new();
                hasher.update(data);
                format!("{:x}", hasher.finalize())
            }
        }
    }
}

/// Checksum calculator that can compute multiple algorithms at once
pub struct ChecksumCalculator {
    sha1: Option<Sha1>,
    sha256: Option<Sha256>,
    sha512: Option<Sha512>,
}

impl ChecksumCalculator {
    /// Create a new calculator for the specified algorithms
    pub fn new(algorithms: &[ChecksumAlgorithm]) -> Self {
        let mut calculator = Self {
            sha1: None,
            sha256: None,
            sha512: None,
        };

        for algorithm in algorithms {
            match algorithm {
                ChecksumAlgorithm::Sha1 => calculator.sha1 = Some(Sha1::new()),
                ChecksumAlgorithm::Sha256 => calculator.sha256 = Some(Sha256::new()),
                ChecksumAlgorithm::Sha512 => calculator.sha512 = Some(Sha512::new()),
            }
        }

        calculator
    }

    /// Update all configured hashers with data
    pub fn update(&mut self, data: &[u8]) {
        if let Some(ref mut hasher) = self.sha1 {
            hasher.update(data);
        }
        if let Some(ref mut hasher) = self.sha256 {
            hasher.update(data);
        }
        if let Some(ref mut hasher) = self.sha512 {
            hasher.update(data);
        }
    }

    /// Finalize and get all checksums
    pub fn finalize(self) -> HashMap<ChecksumAlgorithm, String> {
        let mut results = HashMap::new();

        if let Some(hasher) = self.sha1 {
            results.insert(ChecksumAlgorithm::Sha1, format!("{:x}", hasher.finalize()));
        }
        if let Some(hasher) = self.sha256 {
            results.insert(
                ChecksumAlgorithm::Sha256,
                format!("{:x}", hasher.finalize()),
            );
        }
        if let Some(hasher) = self.sha512 {
            results.insert(
                ChecksumAlgorithm::Sha512,
                format!("{:x}", hasher.finalize()),
            );
        }

        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_checksum() {
        let mut csc = ChecksumCalculator::new(ChecksumAlgorithm::all());
        csc.update(b"abcdefg");
        let hashed = csc.finalize();
        assert!(!hashed.get(&ChecksumAlgorithm::Sha1).unwrap().is_empty());
    }
}
