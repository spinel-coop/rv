use crate::{checksum::Checksums, entry::DataReader, source::PackageSource, Error, Result};
use flate2::read::GzDecoder;
use rv_gem_types::Specification;
use saphyr::{LoadableYamlNode, Yaml};
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;
use tar::Archive;

/// A .gem package that can be read and analyzed
pub struct Package<S: PackageSource> {
    source: S,
    spec: Option<Specification>,
    checksums: Option<Checksums>,
}

impl Package<std::fs::File> {
    /// Open a .gem file from the filesystem
    /// Use ::from_source here, instead of duplicating logic
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let mut file = std::fs::File::open(path)?;

        // Check for old-style gem format by reading the first few bytes
        let mut buffer = [0u8; 32];
        file.read_exact(&mut buffer)?;
        file.seek(SeekFrom::Start(0))?;

        // Check if it's an old-style gem (contains "MD5SUM =")
        if buffer.windows(8).any(|window| window == b"MD5SUM =") {
            return Err(Error::OldFormatError);
        }

        Ok(Self {
            source: file,
            spec: None,
            checksums: None,
        })
    }
}

impl<S: PackageSource> Package<S> {
    /// Create a new package from any source
    pub fn from_source(mut source: S) -> Result<Self> {
        // Check for old-style gem format by reading the first few bytes
        let mut buffer = [0u8; 32];
        source.read_exact(&mut buffer)?;
        source.seek(SeekFrom::Start(0))?;

        // Check if it's an old-style gem (contains "MD5SUM =")
        if buffer.windows(8).any(|window| window == b"MD5SUM =") {
            return Err(Error::OldFormatError);
        }

        Ok(Self {
            source,
            spec: None,
            checksums: None,
        })
    }

    /// Get the gem specification (lazy loaded)
    pub fn spec(&mut self) -> Result<&Specification> {
        if self.spec.is_none() {
            self.load_spec()?;
        }
        Ok(self.spec.as_ref().unwrap())
    }

    /// Get access to the data.tar.gz contents for streaming
    pub fn data(&mut self) -> Result<DataReader<GzDecoder<std::io::Cursor<Vec<u8>>>>> {
        self.source.seek(SeekFrom::Start(0))?;
        let mut archive = Archive::new(&mut self.source);

        for entry in archive.entries()? {
            let mut entry = entry.map_err(|e| Error::tar_error(e.to_string()))?;
            let path = entry
                .header()
                .path()
                .map_err(|e| Error::tar_error(e.to_string()))?;
            let path_str = path.to_string_lossy();

            if path_str == "data.tar.gz" {
                let mut data = Vec::new();
                entry.read_to_end(&mut data)?;
                let cursor = std::io::Cursor::new(data);
                let gzip_decoder = GzDecoder::new(cursor);
                return Ok(DataReader::new(gzip_decoder));
            }
        }

        Err(Error::format_error("No data.tar.gz found in gem"))
    }

    /// Verify the package checksums
    pub fn verify(&mut self) -> Result<()> {
        todo!("implement in phase 5")
    }

    /// Get the checksums (lazy loaded)
    pub fn checksums(&mut self) -> Result<&Checksums> {
        if self.checksums.is_none() {
            self.load_checksums()?;
        }
        Ok(self.checksums.as_ref().unwrap())
    }

    /// Load the gem specification from metadata.gz
    fn load_spec(&mut self) -> Result<()> {
        self.source.seek(SeekFrom::Start(0))?;
        let mut archive = Archive::new(&mut self.source);

        for entry in archive.entries()? {
            let mut entry = entry.map_err(|e| Error::tar_error(e.to_string()))?;
            let path = entry
                .header()
                .path()
                .map_err(|e| Error::tar_error(e.to_string()))?;
            let path_str = path.to_string_lossy();

            match path_str.as_ref() {
                "metadata.gz" => {
                    let mut content = Vec::new();
                    let mut decoder = GzDecoder::new(&mut entry);
                    decoder.read_to_end(&mut content)?;

                    let yaml_str = String::from_utf8(content).map_err(|e| {
                        Error::format_error(format!("Invalid UTF-8 in metadata.gz: {e}"))
                    })?;

                    self.spec = Some(
                        rv_gem_specification_yaml::parse(&yaml_str)
                            .map_err(|e| Error::YamlError(e.to_string()))?,
                    );
                    return Ok(());
                }
                "metadata" => {
                    let mut content = Vec::new();
                    entry.read_to_end(&mut content)?;

                    let yaml_str = String::from_utf8(content).map_err(|e| {
                        Error::format_error(format!("Invalid UTF-8 in metadata: {e}"))
                    })?;

                    self.spec = Some(
                        rv_gem_specification_yaml::parse(&yaml_str)
                            .map_err(|e| Error::YamlError(e.to_string()))?,
                    );
                    return Ok(());
                }
                _ => continue,
            }
        }

        Err(Error::format_error("No metadata found in gem"))
    }

    /// Load checksums from checksums.yaml.gz
    fn load_checksums(&mut self) -> Result<()> {
        self.source.seek(SeekFrom::Start(0))?;
        let mut archive = Archive::new(&mut self.source);

        for entry in archive.entries()? {
            let mut entry = entry.map_err(|e| Error::tar_error(e.to_string()))?;
            let path = entry
                .header()
                .path()
                .map_err(|e| Error::tar_error(e.to_string()))?;
            let path_str = path.to_string_lossy();

            if path_str == "checksums.yaml.gz" {
                let mut content = Vec::new();
                let mut decoder = GzDecoder::new(&mut entry);
                decoder.read_to_end(&mut content)?;

                let yaml_str = String::from_utf8(content).map_err(|e| {
                    Error::format_error(format!("Invalid UTF-8 in checksums.yaml.gz: {e}"))
                })?;

                // Parse the YAML manually since it's a simple structure
                self.checksums = Some(self.parse_checksums_yaml(&yaml_str)?);
                return Ok(());
            }
        }

        // Checksums are optional in older gems
        self.checksums = Some(Checksums::new());
        Ok(())
    }

    /// Parse checksums YAML format
    fn parse_checksums_yaml(&self, yaml_str: &str) -> Result<Checksums> {
        // Use saphyr for parsing the checksums structure
        let docs = Yaml::load_from_str(yaml_str)
            .map_err(|e| Error::format_error(format!("Invalid checksums YAML: {e}")))?;

        let doc = docs
            .first()
            .ok_or_else(|| Error::format_error("Empty checksums YAML"))?;

        let mut checksums = Checksums::new();

        // Iterate over the top-level mapping (algorithm -> files)
        if let Some(top_mapping) = doc.as_mapping() {
            for (algorithm_key, files_value) in top_mapping {
                if let (Some(algorithm), Some(files_mapping)) =
                    (algorithm_key.as_str(), files_value.as_mapping())
                {
                    // Iterate over files for this algorithm
                    for (file_key, checksum_value) in files_mapping {
                        if let (Some(file), Some(checksum)) =
                            (file_key.as_str(), checksum_value.as_str())
                        {
                            checksums.add_checksum(algorithm, file, checksum);
                        }
                    }
                }
            }
        }

        Ok(checksums)
    }
}
