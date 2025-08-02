# rsfs

A zero cost wrapper around std::fs.

The FS struct is an empty struct. All methods on it use std::fs functions. The intent of this module is to set the filesystem you use to rsfs::disk::FS in main.rs and to set the filesystem to rsfs::mem::test::FS (once it exists) in your tests.

Examples

```
// Use rsfs to access the physical filesystem
use rsfs::*;
use rsfs::unix_ext::*;

let fs = rsfs::disk::FS;

let meta = fs.metadata("/").unwrap();
assert!(meta.is_dir());
assert_eq!(meta.permissions().mode(), 0o755);
```

```
// Use rsfs to access an in-memory filesystem
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::PathBuf;

use rsfs::*;
use rsfs::mem::FS;

let fs = FS::new();
assert!(fs.create_dir_all("a/b/c").is_ok());

let mut wf = fs.create_file("a/f").unwrap();
assert_eq!(wf.write(b"hello").unwrap(), 5);

let mut rf = fs.open_file("a/f").unwrap();
let mut output = [0u8; 5];
assert_eq!(rf.read(&mut output).unwrap(), 5);
assert_eq!(&output, b"hello");
```
