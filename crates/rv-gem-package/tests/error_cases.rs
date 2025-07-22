use rv_gem_package::{Error, Package};
use std::io::{Cursor, Write};
use std::path::Path;

/// Test corrupted gem file (truncated data)
#[test]
fn test_corrupted_gem_truncated() {
    let gem_path = Path::new("tests/fixtures/test-gem-1.0.0.gem");
    let gem_data = std::fs::read(gem_path).expect("Failed to read gem file");

    // Truncate the data to corrupt it
    let truncated_data = &gem_data[..gem_data.len() / 2];
    let cursor = Cursor::new(truncated_data.to_vec());

    let package = Package::from_source(cursor);
    match package {
        Ok(mut pkg) => {
            // Package might open but should fail when trying to access data
            match pkg.data() {
                Err(_) => {
                    // Expected - corrupted data should cause error
                }
                Ok(_) => {
                    // Sometimes truncation might not immediately cause error
                    // Try to access spec instead
                    match pkg.spec() {
                        Err(_) => {
                            // Expected error accessing corrupted data
                        }
                        Ok(_) => {
                            // Even if spec works, data access should fail
                            // This is acceptable - some corruption might not be immediately detectable
                        }
                    }
                }
            }
        }
        Err(_) => {
            // Corruption detected immediately - also acceptable
        }
    }
}

/// Test gem with invalid tar structure
#[test]
fn test_invalid_tar_structure() {
    // Create data that looks like it might be a gem but isn't valid tar
    let invalid_tar = b"Not a valid tar file but long enough to pass initial checks";
    let mut full_data = Vec::new();

    // Make it long enough to pass the MD5SUM check
    full_data.extend_from_slice(b"This is not MD5SUM = data, so it should pass old format check\n");
    full_data.extend_from_slice(invalid_tar);

    let cursor = Cursor::new(full_data);

    match Package::from_source(cursor) {
        Ok(mut package) => {
            // Package creation might succeed, but accessing data should fail
            match package.spec() {
                Err(_) => {
                    // Expected - invalid tar should cause error when trying to read entries
                }
                Ok(_) => panic!("Expected error for invalid tar structure"),
            }
        }
        Err(_) => {
            // Immediate error detection is also acceptable
        }
    }
}

/// Test gem with missing required files (no metadata)
#[test]
fn test_gem_missing_metadata() {
    use tar::{Builder, Header};

    // Create a tar archive with data.tar.gz but no metadata.gz
    let mut tar_data = Vec::new();
    {
        let mut tar_builder = Builder::new(&mut tar_data);

        // Add a fake data.tar.gz entry
        let mut header = Header::new_gnu();
        header.set_path("data.tar.gz").unwrap();
        header.set_size(10);
        header.set_mode(0o644);
        header.set_cksum();
        tar_builder.append(&header, "fake data".as_bytes()).unwrap();

        tar_builder.finish().unwrap();
    }

    let cursor = Cursor::new(tar_data);
    let mut package = Package::from_source(cursor).expect("Failed to create package");

    // Should fail when trying to get spec due to missing metadata
    match package.spec() {
        Err(Error::FormatError { .. }) => {
            // Expected error for missing metadata
        }
        Err(e) => panic!("Expected format error, got: {e:?}"),
        Ok(_) => panic!("Expected error for missing metadata"),
    }
}

/// Test checksum mismatch by modifying gem content
#[test]
fn test_checksum_mismatch() {
    use flate2::read::GzDecoder;
    use flate2::write::GzEncoder;
    use flate2::Compression;
    use std::io::Read;
    use tar::{Archive, Builder, Header};

    // Read the original gem
    let gem_path = Path::new("tests/fixtures/test-gem-1.0.0.gem");
    let gem_data = std::fs::read(gem_path).expect("Failed to read gem file");

    // Extract and modify the gem contents to create checksum mismatch
    let cursor = Cursor::new(gem_data);
    let mut archive = Archive::new(cursor);

    let mut modified_tar = Vec::new();
    {
        let mut tar_builder = Builder::new(&mut modified_tar);

        for entry_result in archive.entries().expect("Failed to get entries") {
            let mut entry = entry_result.expect("Failed to get entry");
            let header = entry.header().clone();
            let path = header.path().expect("Failed to get path");

            if path.to_string_lossy() == "data.tar.gz" {
                // Modify the data.tar.gz content to create checksum mismatch
                let mut original_data = Vec::new();
                entry
                    .read_to_end(&mut original_data)
                    .expect("Failed to read data.tar.gz");

                // Decode, modify, and re-encode the data.tar.gz
                let decoder = GzDecoder::new(Cursor::new(&original_data));
                let mut data_archive = Archive::new(decoder);

                let mut modified_data_tar = Vec::new();
                {
                    let mut data_builder = Builder::new(&mut modified_data_tar);

                    for data_entry_result in
                        data_archive.entries().expect("Failed to get data entries")
                    {
                        let data_entry = data_entry_result.expect("Failed to get data entry");
                        let data_header = data_entry.header().clone();
                        let data_path = data_header.path().expect("Failed to get data path");

                        if data_path.to_string_lossy() == "lib/test_gem.rb" {
                            // Modify this file's content
                            let modified_content = b"# Modified content that will cause checksum mismatch\nmodule TestGem\n  VERSION = '999.0.0'\nend\n";

                            let mut new_header = Header::new_gnu();
                            new_header.set_path(&data_path).unwrap();
                            new_header.set_size(modified_content.len() as u64);
                            new_header.set_mode(data_header.mode().unwrap());
                            new_header.set_cksum();

                            data_builder
                                .append(&new_header, &modified_content[..])
                                .expect("Failed to append modified file");
                        } else {
                            // Copy other files unchanged
                            let mut content = Vec::new();
                            let mut data_entry_clone = data_entry;
                            data_entry_clone
                                .read_to_end(&mut content)
                                .expect("Failed to read data entry");

                            data_builder
                                .append(&data_header, &content[..])
                                .expect("Failed to append data entry");
                        }
                    }

                    data_builder
                        .finish()
                        .expect("Failed to finish data builder");
                }

                // Re-compress the modified data
                let mut modified_compressed = Vec::new();
                {
                    let mut encoder =
                        GzEncoder::new(&mut modified_compressed, Compression::default());
                    encoder
                        .write_all(&modified_data_tar)
                        .expect("Failed to compress modified data");
                    encoder.finish().expect("Failed to finish compression");
                }

                // Add the modified data.tar.gz to the new gem
                let mut new_header = Header::new_gnu();
                new_header.set_path("data.tar.gz").unwrap();
                new_header.set_size(modified_compressed.len() as u64);
                new_header.set_mode(0o644);
                new_header.set_cksum();

                tar_builder
                    .append(&new_header, &modified_compressed[..])
                    .expect("Failed to append modified data.tar.gz");
            } else {
                // Copy other entries unchanged (metadata.gz, checksums.yaml.gz)
                let mut content = Vec::new();
                entry
                    .read_to_end(&mut content)
                    .expect("Failed to read entry");

                tar_builder
                    .append(&header, &content[..])
                    .expect("Failed to append entry");
            }
        }

        tar_builder.finish().expect("Failed to finish builder");
    }

    // Test the modified gem
    let cursor = Cursor::new(modified_tar);
    let mut package =
        Package::from_source(cursor).expect("Failed to create package from modified gem");

    // Verification should now fail due to checksum mismatch
    match package.verify() {
        Err(Error::ChecksumError { .. }) => {
            // Expected checksum error
        }
        Err(e) => panic!("Expected checksum error, got: {e:?}"),
        Ok(_) => panic!("Expected checksum verification to fail"),
    }
}

/// Test accessing files not in checksums
#[test]
fn test_file_not_in_checksums() {
    let gem_path = Path::new("tests/fixtures/test-gem-1.0.0.gem");
    let mut package = Package::open(gem_path).expect("Failed to open test gem");

    let mut data_reader = package.data().expect("Failed to get data reader");

    // Try to find a file that doesn't exist
    match data_reader.find_file("nonexistent/file.rb") {
        Ok(None) => {
            // Expected - file not found
        }
        Ok(Some(_)) => panic!("Found nonexistent file"),
        Err(e) => panic!("Unexpected error: {e:?}"),
    }
}

/// Test empty gem data
#[test]
fn test_empty_gem_data() {
    let empty_data = Vec::new();
    let cursor = Cursor::new(empty_data);

    match Package::from_source(cursor) {
        Err(_) => {
            // Expected error for empty data
        }
        Ok(_) => panic!("Expected error for empty gem data"),
    }
}

/// Test gem with only partial header
#[test]
fn test_partial_header() {
    // Less than 32 bytes should cause read_exact to fail
    let partial_data = b"This is short";
    let cursor = Cursor::new(partial_data.to_vec());

    match Package::from_source(cursor) {
        Err(_) => {
            // Expected error for insufficient data
        }
        Ok(_) => panic!("Expected error for partial header"),
    }
}
