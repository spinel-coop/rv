use super::Error;
use super::Result;
use std::io::{self, Read};

use bytes::Bytes;
use sha2::Digest;
use sha2::Sha256;
use sha2::Sha512;

/// Checksums found in the gem under checksums.yaml
/// Note we do NOT check SHA1 as it is insecure.
#[derive(Debug, Default)]
pub struct ArchiveChecksums {
    pub sha256: Option<ChecksumFiles>,
    pub sha512: Option<ChecksumFiles>,
}

/// Checksums found in the gem under checksums.yaml
#[derive(Debug)]
pub struct ChecksumFiles {
    /// Expected checksum, given by server.
    pub metadata_gz: Vec<u8>,
    /// Expected checksum, given by server.
    pub data_tar_gz: Vec<u8>,
}

fn hex_key(yaml: &saphyr::Yaml<'_>) -> Option<Vec<u8>> {
    if let Some(tag) = yaml.get_tag()
        && tag.handle == "!"
        && tag.suffix == "binary"
        && let Some(tagged) = yaml.get_tagged_node()
        && let Some(s) = tagged.as_str()
    {
        return b64_then_hex(s);
    }
    hex::decode(yaml.as_str()?).ok()
}

fn b64_then_hex(s: &str) -> Option<Vec<u8>> {
    use base64::prelude::*;
    // YAML literal blocks may include newlines, which shouldn't
    // be considered part of the b64.
    let without_newlines: String = s.chars().filter(|c| !c.is_whitespace()).collect();
    base64::engine::general_purpose::STANDARD
        .decode(without_newlines)
        .ok()
        .and_then(|x| String::from_utf8(x).ok())
        .and_then(|x| hex::decode(x).ok())
}

impl ArchiveChecksums {
    pub fn new(file: &str) -> Option<Self> {
        use saphyr::{LoadableYamlNode, Yaml};
        let contents_yaml = Yaml::load_from_str(file).ok()?;
        let root = contents_yaml.first()?;
        let mut out = ArchiveChecksums::default();

        let b64_sha512_tag = "U0hBNTEy";
        let b64_sha256_tag = "U0hBMjU2";
        let _b64_sha1_tag = "U0hBMQ=="; // ignored, sha1 is not secure anymore.
        for (k, v) in root.as_mapping().unwrap() {
            if let Some(tagged_node) = k.get_tagged_node()
                && let Some(tag) = k.get_tag()
                && tag.handle == "!"
                && tag.suffix == "binary"
            {
                if tagged_node.as_str() == Some(b64_sha256_tag) {
                    let metadata = v
                        .as_mapping_get("metadata.gz")
                        .and_then(|yaml| yaml.get_tagged_node())
                        .and_then(|yaml| yaml.as_str())
                        .and_then(b64_then_hex);
                    let data_tar = v
                        .as_mapping_get("data.tar.gz")
                        .and_then(|yaml| yaml.get_tagged_node())
                        .and_then(|yaml| yaml.as_str())
                        .and_then(b64_then_hex);
                    if let (Some(m), Some(d)) = (metadata, data_tar) {
                        out.sha256 = Some(ChecksumFiles {
                            metadata_gz: m,
                            data_tar_gz: d,
                        });
                    }
                } else if tagged_node.as_str() == Some(b64_sha512_tag) {
                    let metadata = v
                        .as_mapping_get("metadata.gz")
                        .and_then(|yaml| yaml.get_tagged_node())
                        .and_then(|yaml| yaml.as_str())
                        .and_then(b64_then_hex);
                    let data_tar = v
                        .as_mapping_get("data.tar.gz")
                        .and_then(|yaml| yaml.get_tagged_node())
                        .and_then(|yaml| yaml.as_str())
                        .and_then(b64_then_hex);
                    if let (Some(m), Some(d)) = (metadata, data_tar) {
                        out.sha512 = Some(ChecksumFiles {
                            metadata_gz: m,
                            data_tar_gz: d,
                        });
                    }
                }
            }
        }

        if let Some(checksums) = root.as_mapping_get("SHA256") {
            out.sha256 = Some(ChecksumFiles {
                metadata_gz: checksums.as_mapping_get("metadata.gz").and_then(hex_key)?,
                data_tar_gz: checksums.as_mapping_get("data.tar.gz").and_then(hex_key)?,
            });
        }
        if let Some(checksums) = root.as_mapping_get("SHA512") {
            out.sha512 = Some(ChecksumFiles {
                metadata_gz: checksums.as_mapping_get("metadata.gz").and_then(hex_key)?,
                data_tar_gz: checksums.as_mapping_get("data.tar.gz").and_then(hex_key)?,
            });
        }
        Some(out)
    }

    pub fn validate_data_tar(&self, gem_name: String, hashed: &Hashed) -> Result<()> {
        if self.sha256.is_none() && self.sha512.is_none() {
            eprintln!("Checksum file for {gem_name} was empty");
        }
        if let Some(sha256) = &self.sha256
            && hashed.digest_256 != sha256.data_tar_gz
        {
            return Err(Error::ArchiveChecksumFail {
                filename: "data.tar.gz".to_owned(),
                gem_name,
                algo: "sha256",
            });
        }
        if let Some(sha512) = &self.sha512
            && hashed.digest_512 != sha512.data_tar_gz
        {
            return Err(Error::ArchiveChecksumFail {
                filename: "data.tar.gz".to_owned(),
                gem_name,
                algo: "sha512",
            });
        }
        Ok(())
    }

    pub fn validate_metadata(&self, gem_name: String, hashed: Hashed) -> Result<()> {
        if self.sha256.is_none() && self.sha512.is_none() {
            eprintln!("Checksum file for {gem_name} was empty");
        }
        if let Some(sha256) = &self.sha256 {
            let expected = &sha256.metadata_gz;
            if hashed.digest_256 != expected {
                return Err(Error::ArchiveChecksumFail {
                    filename: "metadata.gz".to_owned(),
                    gem_name,
                    algo: "sha256",
                });
            }
        }
        if let Some(sha512) = &self.sha512
            && hashed.digest_512 != sha512.metadata_gz
        {
            return Err(Error::ArchiveChecksumFail {
                filename: "metadata.gz".to_owned(),
                gem_name,
                algo: "sha512",
            });
        }
        Ok(())
    }
}

/// Wrapper around some reader type `R`
/// that also computes SHA checksums as it reads.
pub struct HashReader<R> {
    reader: R,
    h256: Sha256,
    h512: Sha512,
}

pub struct Hashed {
    digest_256: Bytes,
    digest_512: Bytes,
}

impl<R> std::io::Read for HashReader<R>
where
    R: Read,
{
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let n = self.reader.read(buf)?;
        if n > 0 {
            self.h256.update(&buf[..n]);
            self.h512.update(&buf[..n]);
        }
        Ok(n)
    }
}

impl<R> HashReader<R> {
    /// Wrap the `reader` into this `HashReader` which both
    /// reads and also computes checksums.
    pub fn new(reader: R) -> Self {
        Self {
            reader,
            h256: Default::default(),
            h512: Default::default(),
        }
    }

    /// Get the final hash.
    pub fn finalize(self) -> Hashed {
        Hashed {
            digest_256: self.h256.finalize().to_vec().into(),
            digest_512: self.h512.finalize().to_vec().into(),
        }
    }
}
