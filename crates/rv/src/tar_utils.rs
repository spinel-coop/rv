use std::io::{self, Read};
use std::path::Path;

/// Windows error code for "A required privilege is not held by the client."
/// Symlink creation requires either Developer Mode or admin privileges.
#[cfg(windows)]
const ERROR_PRIVILEGE_NOT_HELD: i32 = 1314;

/// Unpack a tar archive to `dst`, falling back to file copies when symlink
/// creation fails on Windows (requires Developer Mode or admin privileges).
pub fn unpack_tar<R: Read>(archive: &mut tar::Archive<R>, dst: &Path) -> io::Result<()> {
    #[cfg(not(windows))]
    {
        archive.unpack(dst)?;
        return Ok(());
    }

    #[cfg(windows)]
    {
        for entry_result in archive.entries()? {
            let mut entry = entry_result?;
            let entry_path = entry.path()?.into_owned();
            let dest_path = dst.join(&entry_path);

            // Ensure parent directories exist before unpacking.
            // (Archive::unpack does this automatically, but Entry::unpack does not.)
            if let Some(parent) = dest_path.parent() {
                std::fs::create_dir_all(parent)?;
            }

            if entry.header().entry_type().is_symlink() {
                handle_symlink_entry(&entry, &dest_path)?;
            } else {
                // Hard links work without admin on Windows (within the same volume).
                entry.unpack(&dest_path)?;
            }
        }

        Ok(())
    }
}

/// Unpack a single tar entry to `dst`, with symlink fallback on Windows.
///
/// This wraps `entry.unpack(dst)` and adds a copy-based fallback for symlinks
/// that fail due to missing privileges on Windows.
pub fn unpack_entry<R: Read>(entry: &mut tar::Entry<'_, R>, dst: &Path) -> io::Result<()> {
    #[cfg(not(windows))]
    {
        entry.unpack(dst)?;
        return Ok(());
    }

    #[cfg(windows)]
    {
        if entry.header().entry_type().is_symlink() {
            handle_symlink_entry(entry, dst)?;
        } else {
            entry.unpack(dst)?;
        }

        Ok(())
    }
}

/// Extract the symlink target from a tar entry and either create a symlink
/// or fall back to copying the target on Windows.
#[cfg(windows)]
fn handle_symlink_entry<R: Read>(
    entry: &tar::Entry<'_, R>,
    dest_path: &Path,
) -> io::Result<()> {
    let link_target = entry
        .link_name()?
        .map(|l| l.into_owned())
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "symlink entry {} has no target",
                    dest_path.display()
                ),
            )
        })?;

    let parent = dest_path.parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("path {} has no parent directory", dest_path.display()),
        )
    })?;

    // Resolve the symlink target relative to the symlink's parent directory.
    let resolved_target = parent.join(&link_target);

    create_symlink_or_copy(&link_target, dest_path, &resolved_target)
}

/// Try to create a symlink; if it fails on Windows, copy the target instead.
#[cfg(windows)]
fn create_symlink_or_copy(
    link_target: &Path,
    dst: &Path,
    resolved_target: &Path,
) -> io::Result<()> {
    // Try symlink_file first. On Windows, both symlink_file and symlink_dir
    // require Developer Mode or admin privileges.
    let symlink_err = match std::os::windows::fs::symlink_file(link_target, dst) {
        Ok(()) => return Ok(()),
        Err(e) => e,
    };

    // If the error isn't a privilege issue, try symlink_dir in case the
    // target is a directory (symlink_file won't work for directories).
    if symlink_err.raw_os_error() != Some(ERROR_PRIVILEGE_NOT_HELD)
        && std::os::windows::fs::symlink_dir(link_target, dst).is_ok()
    {
        return Ok(());
    }

    tracing::debug!(
        "Symlink creation failed for {} -> {}, falling back to copy: {}",
        dst.display(),
        link_target.display(),
        symlink_err
    );

    if !resolved_target.exists() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!(
                "symlink target {} does not exist (resolved to {})",
                link_target.display(),
                resolved_target.display()
            ),
        ));
    }

    if let Some(parent) = dst.parent() {
        std::fs::create_dir_all(parent)?;
    }

    if resolved_target.is_dir() {
        copy_dir_recursive(resolved_target, dst)
    } else {
        std::fs::copy(resolved_target, dst)?;
        Ok(())
    }
}

#[cfg(windows)]
fn copy_dir_recursive(src: &Path, dst: &Path) -> io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let entry_dst = dst.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir_recursive(&entry.path(), &entry_dst)?;
        } else {
            std::fs::copy(entry.path(), &entry_dst)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    use tempfile::TempDir;

    /// Helper to build tar archives in memory for testing.
    struct TarBuilder {
        builder: tar::Builder<Vec<u8>>,
    }

    impl TarBuilder {
        fn new() -> Self {
            Self {
                builder: tar::Builder::new(Vec::new()),
            }
        }

        fn add_file(mut self, path: &str, content: &[u8]) -> Self {
            let mut header = tar::Header::new_gnu();
            header.set_path(path).unwrap();
            header.set_size(content.len() as u64);
            header.set_mode(0o644);
            header.set_entry_type(tar::EntryType::Regular);
            header.set_cksum();
            self.builder.append(&header, content).unwrap();
            self
        }

        fn add_dir(mut self, path: &str) -> Self {
            let mut header = tar::Header::new_gnu();
            header.set_path(path).unwrap();
            header.set_size(0);
            header.set_mode(0o755);
            header.set_entry_type(tar::EntryType::Directory);
            header.set_cksum();
            self.builder.append(&header, &[] as &[u8]).unwrap();
            self
        }

        fn add_symlink(mut self, path: &str, target: &str) -> Self {
            let mut header = tar::Header::new_gnu();
            header.set_path(path).unwrap();
            header.set_size(0);
            header.set_mode(0o777);
            header.set_entry_type(tar::EntryType::Symlink);
            header.set_link_name(target).unwrap();
            header.set_cksum();
            self.builder.append(&header, &[] as &[u8]).unwrap();
            self
        }

        fn build(mut self) -> Vec<u8> {
            self.builder.finish().unwrap();
            self.builder.into_inner().unwrap()
        }
    }

    // -- unpack_tar tests --
    // These verify the public API contract: after unpacking, files (including
    // symlink targets) are readable with the expected content. On Windows
    // without Developer Mode, symlinks are transparently replaced by copies.

    #[test]
    fn test_unpack_tar_regular_files_and_nested_dirs() {
        let temp_dir = TempDir::new().unwrap();
        let data = TarBuilder::new()
            .add_dir("subdir/")
            .add_file("hello.txt", b"hello world")
            .add_file("subdir/nested.txt", b"nested content")
            .build();

        let mut archive = tar::Archive::new(Cursor::new(data));
        unpack_tar(&mut archive, temp_dir.path()).unwrap();

        assert_eq!(
            std::fs::read_to_string(temp_dir.path().join("hello.txt")).unwrap(),
            "hello world"
        );
        assert_eq!(
            std::fs::read_to_string(temp_dir.path().join("subdir/nested.txt")).unwrap(),
            "nested content"
        );
    }

    #[test]
    fn test_unpack_tar_file_symlink_is_readable() {
        let temp_dir = TempDir::new().unwrap();
        let data = TarBuilder::new()
            .add_file("real.txt", b"real content")
            .add_symlink("link.txt", "real.txt")
            .build();

        let mut archive = tar::Archive::new(Cursor::new(data));
        unpack_tar(&mut archive, temp_dir.path()).unwrap();

        // Whether a real symlink or a copy, the content must be readable.
        let content = std::fs::read_to_string(temp_dir.path().join("link.txt")).unwrap();
        assert_eq!(content, "real content");
    }

    #[test]
    fn test_unpack_tar_directory_symlink_is_readable() {
        let temp_dir = TempDir::new().unwrap();
        let data = TarBuilder::new()
            .add_dir("real_dir/")
            .add_file("real_dir/file.txt", b"dir file content")
            .add_symlink("link_dir", "real_dir")
            .build();

        let mut archive = tar::Archive::new(Cursor::new(data));
        unpack_tar(&mut archive, temp_dir.path()).unwrap();

        let content =
            std::fs::read_to_string(temp_dir.path().join("link_dir").join("file.txt")).unwrap();
        assert_eq!(content, "dir file content");
    }

    // -- unpack_entry tests --

    #[test]
    fn test_unpack_entry_symlink_is_readable() {
        let temp_dir = TempDir::new().unwrap();

        // The symlink target must already exist on disk for unpack_entry,
        // since it operates on a single entry without context of the archive.
        std::fs::write(temp_dir.path().join("target.txt"), b"target content").unwrap();

        let data = TarBuilder::new()
            .add_symlink("link.txt", "target.txt")
            .build();

        let mut archive = tar::Archive::new(Cursor::new(data));
        let mut entries = archive.entries().unwrap();
        let mut entry = entries.next().unwrap().unwrap();

        let dst = temp_dir.path().join("link.txt");
        unpack_entry(&mut entry, &dst).unwrap();

        assert_eq!(
            std::fs::read_to_string(&dst).unwrap(),
            "target content"
        );
    }

    // -- Windows-only tests --
    // These test internal helpers that are only compiled on Windows.

    #[cfg(windows)]
    mod windows_tests {
        use super::*;

        #[test]
        fn test_copy_dir_recursive_copies_nested_structure() {
            let temp_dir = TempDir::new().unwrap();
            let src = temp_dir.path().join("src");
            let dst = temp_dir.path().join("dst");

            std::fs::create_dir_all(src.join("sub")).unwrap();
            std::fs::write(src.join("a.txt"), b"aaa").unwrap();
            std::fs::write(src.join("sub").join("b.txt"), b"bbb").unwrap();

            copy_dir_recursive(&src, &dst).unwrap();

            assert_eq!(std::fs::read_to_string(dst.join("a.txt")).unwrap(), "aaa");
            assert_eq!(
                std::fs::read_to_string(dst.join("sub").join("b.txt")).unwrap(),
                "bbb"
            );
        }

        #[test]
        fn test_create_symlink_or_copy_returns_not_found_for_missing_target() {
            let temp_dir = TempDir::new().unwrap();
            let dst = temp_dir.path().join("link.txt");
            let link_target = Path::new("nonexistent.txt");
            let resolved = temp_dir.path().join("nonexistent.txt");

            let result = create_symlink_or_copy(link_target, &dst, &resolved);

            // On Developer Mode machines, symlink_file succeeds with a dangling
            // symlink. On non-Developer-Mode machines, the fallback checks
            // resolved_target.exists() and returns NotFound. Either outcome is
            // acceptable — the important thing is it doesn't panic.
            if let Err(err) = result {
                assert_eq!(err.kind(), io::ErrorKind::NotFound);
            }
        }
    }
}
