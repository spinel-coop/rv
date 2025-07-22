use std::collections::HashMap;

/// Checksums for files in a gem package
#[derive(Debug, Clone, Default)]
pub struct Checksums {
    /// Map of algorithm name to file checksums
    /// Key: algorithm name (e.g., "SHA256", "SHA512")
    /// Value: map of file path to hexadecimal checksum string
    pub algorithms: HashMap<String, HashMap<String, String>>,
}

impl Checksums {
    /// Create a new empty checksums collection
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a checksum for a file
    pub fn add_checksum(&mut self, algorithm: &str, file_path: &str, checksum: &str) {
        self.algorithms
            .entry(algorithm.to_string())
            .or_default()
            .insert(file_path.to_string(), checksum.to_string());
    }

    /// Get checksum for a specific file and algorithm
    pub fn get_checksum(&self, algorithm: &str, file_path: &str) -> Option<&str> {
        self.algorithms
            .get(algorithm)?
            .get(file_path)
            .map(|s| s.as_str())
    }

    /// Get all algorithms available
    pub fn algorithms(&self) -> impl Iterator<Item = &str> {
        self.algorithms.keys().map(|s| s.as_str())
    }

    /// Get all files for a specific algorithm
    pub fn files_for_algorithm(&self, algorithm: &str) -> Option<impl Iterator<Item = &str>> {
        self.algorithms
            .get(algorithm)
            .map(|files| files.keys().map(|s| s.as_str()))
    }

    /// Check if checksums are empty
    pub fn is_empty(&self) -> bool {
        self.algorithms.is_empty()
    }
}

/// Supported checksum algorithms matching Ruby's implementation
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ChecksumAlgorithm {
    Sha1,
    Sha256,
    Sha512,
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
}
