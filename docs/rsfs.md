# Crate Documentation

**Version:** 0.4.1

**Format Version:** 45

# Module `rsfs`

A generic filesystem with disk and in-memory implementations.

# Reason for existence

The [`std::fs`] module provides functions to manipulate the filesytem, and these functions are
good. However, if you have code that uses `std::fs`, it is difficult to ensure that your code
handles errors properly because generally, you are not testing on an already broken machine.
You could attempt to set up FUSE, which, although doable, is involved.

This crate provides a generic filesystem with various implementations. At the moment, only
normal (non-injectable) disk and in-memory implementations are provided. In the future, an
error-injectable shim around the in-memory file system will be provided to help trigger
filesystem errors in unit tests.

The intent of this crate is for you to use the generic [`rsfs::GenFS`] everywhere where you use
`std::fs` in your code. Your `main.rs` can use [`rsfs::disk::FS`] to get default disk behavior
while your tests use `rsfs::mem::test::FS` (once it exists) to get an in-memory filesystem that
can have errors injected.

# An in-memory filesystem

There existed no complete in-process in-memory filesystem when I wrote this crate; the
implementation in [`rsfs::mem`] should suffice most needs.

`rsfs::mem` is a platform specific module that `pub use`s the proper module based off the
builder's platform. To get a platform agnostic module, you need to use the in-memory platform
you desire. Thus, if you use [`rsfs::mem::unix`], you will get an in-memory system that follows
Unix semantics. If you use `rsfs::mem::windows`, you will get an in-memory system that follows
Windows semantics (however, you would have to write that module first).

This means that `rsfs::mem` aims to essentially be an in-memory drop in for `std::fs` and
forces you to structure your code in a cross-platform way. `rsfs::mem::unix` aims to be a Unix
specific drop in that buys you Unix semantics on all platforms.

# Caveats

The current in-memory filesystems are only implemented for Unix. This means that the only
cross-platform in-memory filesystem is specifically `rsfs::mem::unix`. Window's users can help
by implementing the in-memory analog for Windows.

The in-memory filesystem is implemented using some unsafe code. I deemed this necessary after
working with the recursive data structure that is a filesystem through an `Arc`/`RwLock` for
too long. The code is pretty well tested; there should be no problems. The usage of unsafe, in
my opinion, makes the code much clearer, but it did require special care in some functions.

# Documentation credit

This crate copies _a lot_ of the documentation and examples that currently exist in `std::fs`.
It not only makes it easier for people to migrate straight to this crate, but makes this crate
much more understandable. This crate includes Rust's MIT license in its repo for further
attribution purposes.

[`std::fs`]: https://doc.rust-lang.org/std/fs/
[`rsfs::GenFS`]: trait.GenFS.html
[`rsfs::disk::FS`]: disk/struct.FS.html
[`rsfs::mem`]: mem/index.html
[`rsfs::mem::unix`]: mem/unix/index.html

## Modules

## Module `disk`

A zero cost wrapper around [`std::fs`].

The [`FS`] struct is an empty struct. All methods on it use `std::fs` functions. The intent of
this module is to set the filesystem you use to `rsfs::disk::FS` in `main.rs` and to set the
filesystem to `rsfs::mem::test::FS` (once it exists) in your tests.

[`std::fs`]: https://doc.rust-lang.org/std/fs/
[`FS`]: struct.FS.html

# Examples

```
use rsfs::*;
use rsfs::unix_ext::*;

let fs = rsfs::disk::FS;

let meta = fs.metadata("/").unwrap();
assert!(meta.is_dir());
assert_eq!(meta.permissions().mode(), 0o755);
```

```rust
pub mod disk { /* ... */ }
```

### Types

#### Struct `DirBuilder`

A builder used to create directories in various manners.

This builder is a single element tuple containing a [`std::fs::DirBuilder`] that implements [`rsfs::DirBuilder`] and supports [unix extensions].

[`std::fs::DirBuilder`]: https://doc.rust-lang.org/std/fs/struct.DirBuilder.html
[`rsfs::DirBuilder`]: ../trait.DirBuilder.html
[unix extensions]: ../unix_ext/trait.DirBuilderExt.html

# Examples
 
```
# use rsfs::*;
# fn foo() -> std::io::Result<()> {
let fs = rsfs::disk::FS;
let db = fs.new_dirbuilder();
db.create("dir")?;
# Ok(())
# }
```

```rust
pub struct DirBuilder(/* private field */);
```

##### Fields

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `private` | *Private field* |

##### Implementations

###### Trait Implementations

- **UnwindSafe**
- **Send**
- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Erased**
- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **Freeze**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Sync**
- **DirBuilderExt**
  - ```rust
    fn mode(self: &mut Self, mode: u32) -> &mut Self { /* ... */ }
    ```

- **RefUnwindSafe**
- **Unpin**
- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **DirBuilder**
  - ```rust
    fn recursive(self: &mut Self, recursive: bool) -> &mut Self { /* ... */ }
    ```

  - ```rust
    fn create<P: AsRef<Path>>(self: &Self, path: P) -> Result<()> { /* ... */ }
    ```

#### Struct `DirEntry`

Entries returned by the [`ReadDir`] iterator.

An instance of `DirEntry` implements [`rsfs::DirEntry`] and represents an entry inside a
directory on the in-memory filesystem. This struct is a single element tuple containing a
[`std::fs::DirEntry`].

[`ReadDir`]: struct.ReadDir.html
[`rsfs::DirEntry`]: ../trait.DirEntry.html
[`std::fs::DirEntry`]: https://doc.rust-lang.org/std/fs/struct.DirEntry.html

# Examples

```
# use rsfs::*;
# fn foo() -> std::io::Result<()> {
let fs = rsfs::disk::FS;
for entry in fs.read_dir(".")? {
    let entry = entry?;
    println!("{:?}: {:?}", entry.path(), entry.metadata()?.permissions());
}
# Ok(())
# }
```

```rust
pub struct DirEntry(/* private field */);
```

##### Fields

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `private` | *Private field* |

##### Implementations

###### Trait Implementations

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **RefUnwindSafe**
- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **Sync**
- **Freeze**
- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **DirEntry**
  - ```rust
    fn path(self: &Self) -> PathBuf { /* ... */ }
    ```

  - ```rust
    fn metadata(self: &Self) -> Result<<Self as >::Metadata> { /* ... */ }
    ```

  - ```rust
    fn file_type(self: &Self) -> Result<<Self as >::FileType> { /* ... */ }
    ```

  - ```rust
    fn file_name(self: &Self) -> OsString { /* ... */ }
    ```

- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Send**
- **Unpin**
- **Erased**
- **UnwindSafe**
#### Struct `FileType`

Returned from [`Metadata::file_type`], this structure represents the type of a file.

This structure is a single element tuple containing a [`std::fs::FileType`] that implements [`rsfs::FileType`].

[`Metadata::file_type`]: ../trait.Metadata.html#tymethod.file_type
[`std::fs::FileType`]: https://doc.rust-lang.org/std/fs/struct.FileType.html
[`rsfs::FileType`]: ../trait.FileType.html

# Examples

```
# use rsfs::*;
# fn foo() -> std::io::Result<()> {
let fs = rsfs::disk::FS;
let f = fs.create_file("f")?;
assert!(fs.metadata("f")?.file_type().is_file());
# Ok(())
# }
```

```rust
pub struct FileType(/* private field */);
```

##### Fields

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `private` | *Private field* |

##### Implementations

###### Trait Implementations

- **Send**
- **FileType**
  - ```rust
    fn is_dir(self: &Self) -> bool { /* ... */ }
    ```

  - ```rust
    fn is_file(self: &Self) -> bool { /* ... */ }
    ```

  - ```rust
    fn is_symlink(self: &Self) -> bool { /* ... */ }
    ```

- **StructuralPartialEq**
- **Eq**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **UnwindSafe**
- **Sync**
- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **Erased**
- **Unpin**
- **RefUnwindSafe**
- **Freeze**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Copy**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> FileType { /* ... */ }
    ```

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &FileType) -> bool { /* ... */ }
    ```

- **Hash**
  - ```rust
    fn hash<__H: $crate::hash::Hasher>(self: &Self, state: &mut __H) { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

#### Struct `File`

A view into a file on the filesystem.

An instance of `File` can be read or written to depending on the options it was opened with.
Files also implement `Seek` to alter the logical cursor position of the internal file.

This struct is a single element tuple containing a [`std::fs::File`] that implements
[`rsfs::File`] and has [unix extensions].

[`std::fs::File`]: https://doc.rust-lang.org/std/fs/struct.File.html
[`rsfs::File`]: ../trait.File.html
[unix extensions]: ../unix_ext/trait.FileExt.html

# Examples

```
# use rsfs::*;
# use std::io::Write;
# fn foo() -> std::io::Result<()> {
let fs = rsfs::disk::FS;
let mut f = fs.create_file("f")?;
assert_eq!(f.write(&[1, 2, 3])?, 3);
# Ok(())
# }
```

```rust
pub struct File(/* private field */);
```

##### Fields

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `private` | *Private field* |

##### Implementations

###### Trait Implementations

- **Unpin**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Sync**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Erased**
- **FileExt**
  - ```rust
    fn read_at(self: &Self, buf: &mut [u8], offset: u64) -> Result<usize> { /* ... */ }
    ```

  - ```rust
    fn write_at(self: &Self, buf: &[u8], offset: u64) -> Result<usize> { /* ... */ }
    ```

- **Freeze**
- **Read**
  - ```rust
    fn read(self: &mut Self, buf: &mut [u8]) -> Result<usize> { /* ... */ }
    ```

  - ```rust
    fn read(self: &mut Self, buf: &mut [u8]) -> Result<usize> { /* ... */ }
    ```

- **Seek**
  - ```rust
    fn seek(self: &mut Self, pos: SeekFrom) -> Result<u64> { /* ... */ }
    ```

  - ```rust
    fn seek(self: &mut Self, pos: SeekFrom) -> Result<u64> { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **Send**
- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **RefUnwindSafe**
- **UnwindSafe**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **File**
  - ```rust
    fn sync_all(self: &Self) -> Result<()> { /* ... */ }
    ```

  - ```rust
    fn sync_data(self: &Self) -> Result<()> { /* ... */ }
    ```

  - ```rust
    fn set_len(self: &Self, size: u64) -> Result<()> { /* ... */ }
    ```

  - ```rust
    fn metadata(self: &Self) -> Result<<Self as >::Metadata> { /* ... */ }
    ```

  - ```rust
    fn try_clone(self: &Self) -> Result<Self> { /* ... */ }
    ```

  - ```rust
    fn set_permissions(self: &Self, perm: <Self as >::Permissions) -> Result<()> { /* ... */ }
    ```

- **Write**
  - ```rust
    fn write(self: &mut Self, buf: &[u8]) -> Result<usize> { /* ... */ }
    ```

  - ```rust
    fn flush(self: &mut Self) -> Result<()> { /* ... */ }
    ```

  - ```rust
    fn write(self: &mut Self, buf: &[u8]) -> Result<usize> { /* ... */ }
    ```

  - ```rust
    fn flush(self: &mut Self) -> Result<()> { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

#### Struct `Metadata`

Metadata information about a file.

This structure, a single element tuple containing a [`std::fs::Metadata`] that implements
[`rsfs::Metadata`], is returned from the [`metadata`] or [`symlink_metadata`] methods and
represents known metadata information about a file at the instant in time this structure is
instantiated.

[`std::fs::Metadata`]: https://doc.rust-lang.org/std/fs/struct.Metadata.html
[`rsfs::Metadata`]: ../trait.Metadata.html
[`metadata`]: ../trait.GenFS.html#tymethod.metadata
[`symlink_metadata`]: ../trait.GenFS.html#tymethod.symlink_metadata

# Examples

```
# use rsfs::*;
# fn foo() -> std::io::Result<()> {
let fs = rsfs::disk::FS;
fs.create_file("f")?;
println!("{:?}", fs.metadata("f")?);
# Ok(())
# }

```rust
pub struct Metadata(/* private field */);
```

##### Fields

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `private` | *Private field* |

##### Implementations

###### Trait Implementations

- **UnwindSafe**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **Freeze**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Erased**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Unpin**
- **Sync**
- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Metadata { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **Metadata**
  - ```rust
    fn file_type(self: &Self) -> <Self as >::FileType { /* ... */ }
    ```

  - ```rust
    fn is_dir(self: &Self) -> bool { /* ... */ }
    ```

  - ```rust
    fn is_file(self: &Self) -> bool { /* ... */ }
    ```

  - ```rust
    fn len(self: &Self) -> u64 { /* ... */ }
    ```

  - ```rust
    fn permissions(self: &Self) -> <Self as >::Permissions { /* ... */ }
    ```

  - ```rust
    fn modified(self: &Self) -> Result<SystemTime> { /* ... */ }
    ```

  - ```rust
    fn accessed(self: &Self) -> Result<SystemTime> { /* ... */ }
    ```

  - ```rust
    fn created(self: &Self) -> Result<SystemTime> { /* ... */ }
    ```

#### Struct `OpenOptions`

Options and flags which can be used to configure how a file is opened.

This builder, created from `GenFS`s [`new_openopts`], exposes the ability to configure how a
[`File`] is opened and what operations are permitted on the open file. `GenFS`s [`open_file`]
and [`create_file`] methods are aliases for commonly used options with this builder.

This builder is a single element tuple containing a [`std::fs::OpenOptions`] that implements
[`rsfs::OpenOptions`] and supports [unix extensions].

[`new_openopts`]: ../trait.GenFS.html#tymethod.new_openopts
[`open_file`]: ../trait.GenFS.html#tymethod.open_file
[`create_file`]: ../trait.GenFS.html#tymethod.create_file
[`std::fs::OpenOptions`]: https://doc.rust-lang.org/std/fs/struct.OpenOptions.html
[`rsfs::OpenOptions`]: ../trait.OpenOptions.html
[unix extensions]: ../unix_ext/trait.OpenOptionsExt.html

# Examples

Opening a file to read:

```
# use rsfs::*;
# fn foo() -> std::io::Result<()> {
# let fs = rsfs::disk::FS;
let f = fs.new_openopts()
          .read(true)
          .open("f")?;
# Ok(())
# }
```

Opening a file for both reading and writing, as well as creating it if it doesn't exist:

```
# use rsfs::*;
# fn foo() -> std::io::Result<()> {
# let fs = rsfs::disk::FS;
let mut f = fs.new_openopts()
              .read(true)
              .write(true)
              .create(true)
              .open("f")?;
# Ok(())
# }
```

```rust
pub struct OpenOptions(/* private field */);
```

##### Fields

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `private` | *Private field* |

##### Implementations

###### Trait Implementations

- **Freeze**
- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **OpenOptions**
  - ```rust
    fn read(self: &mut Self, read: bool) -> &mut Self { /* ... */ }
    ```

  - ```rust
    fn write(self: &mut Self, write: bool) -> &mut Self { /* ... */ }
    ```

  - ```rust
    fn append(self: &mut Self, append: bool) -> &mut Self { /* ... */ }
    ```

  - ```rust
    fn truncate(self: &mut Self, truncate: bool) -> &mut Self { /* ... */ }
    ```

  - ```rust
    fn create(self: &mut Self, create: bool) -> &mut Self { /* ... */ }
    ```

  - ```rust
    fn create_new(self: &mut Self, create_new: bool) -> &mut Self { /* ... */ }
    ```

  - ```rust
    fn open<P: AsRef<Path>>(self: &Self, path: P) -> Result<<Self as >::File> { /* ... */ }
    ```

- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **OpenOptionsExt**
  - ```rust
    fn mode(self: &mut Self, mode: u32) -> &mut Self { /* ... */ }
    ```

  - ```rust
    fn custom_flags(self: &mut Self, flags: i32) -> &mut Self { /* ... */ }
    ```

- **UnwindSafe**
- **RefUnwindSafe**
- **Unpin**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Erased**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> OpenOptions { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **Sync**
- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Send**
#### Struct `Permissions`

Representation of the various permissions on a file.

This struct is a single element tuple containing a [`std::fs::Permissions`] that implements
[`rsfs::Permissions`] and has [unix extensions].

[`std::fs::Permissions`]: https://doc.rust-lang.org/std/fs/struct.Permissions.html
[`rsfs::Permissions`]: ../trait.Permissions.html
[unix extensions]: ../unix_ext/trait.PermissionsExt.html

# Examples

```
# use rsfs::*;
# use rsfs::mem::FS;
use rsfs::unix_ext::*;
use rsfs::mem::Permissions;
# fn foo() -> std::io::Result<()> {
# let fs = FS::new();
# fs.create_file("foo.txt")?;

fs.set_permissions("foo.txt", Permissions::from_mode(0o400))?;
# Ok(())
# }
```

```rust
pub struct Permissions(/* private field */);
```

##### Fields

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `private` | *Private field* |

##### Implementations

###### Trait Implementations

- **Unpin**
- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **Permissions**
  - ```rust
    fn readonly(self: &Self) -> bool { /* ... */ }
    ```

  - ```rust
    fn set_readonly(self: &mut Self, readonly: bool) { /* ... */ }
    ```

- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Erased**
- **Sync**
- **RefUnwindSafe**
- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Permissions { /* ... */ }
    ```

- **PermissionsExt**
  - ```rust
    fn mode(self: &Self) -> u32 { /* ... */ }
    ```

  - ```rust
    fn set_mode(self: &mut Self, mode: u32) { /* ... */ }
    ```

  - ```rust
    fn from_mode(mode: u32) -> Self { /* ... */ }
    ```

- **StructuralPartialEq**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **UnwindSafe**
- **Freeze**
- **Send**
- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Permissions) -> bool { /* ... */ }
    ```

- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **Eq**
#### Struct `ReadDir`

Iterator over entries in a directory.

This is returned from the [`read_dir`] method of `GenFS` and yields instances of
`io::Result<DirEntry>`. Through a [`DirEntry`], information about contents of a directory can
be learned.

This struct is as ingle element tuple containing a [`std::fs::ReadDir`].

[`read_dir`]: struct.FS.html#method.read_dir
[`DirEntry`]: struct.DirEntry.html
[`std::fs::ReadDir`]: https://doc.rust-lang.org/std/fs/struct.ReadDir.html

```rust
pub struct ReadDir(/* private field */);
```

##### Fields

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `private` | *Private field* |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **RefUnwindSafe**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **IntoIterator**
  - ```rust
    fn into_iter(self: Self) -> I { /* ... */ }
    ```

- **Send**
- **Sync**
- **Unpin**
- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Iterator**
  - ```rust
    fn next(self: &mut Self) -> Option<<Self as >::Item> { /* ... */ }
    ```

- **Erased**
- **Freeze**
- **UnwindSafe**
- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

#### Struct `FS`

An empty struct that satisfies [`rsfs::FS`] by calling [`std::fs`] functions.

Because this is an empty struct, it is inherently thread safe and copyable. The power of using
`rsfs` comes from the ability to choose what filesystem you want to use where: your main can
use a disk backed filesystem, but your tests can use a test filesystem with injected errors.

Alternatively, the in-memory filesystem could suit your needs without forcing you to use disk.

[`rsfs::FS`]: ../trait.FS.html
[`std::fs`]: https://doc.rust-lang.org/std/fs/

# Examples
 
```
use rsfs::*;

let fs = rsfs::disk::FS;
```

```rust
pub struct FS;
```

##### Implementations

###### Trait Implementations

- **Sync**
- **RefUnwindSafe**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Copy**
- **GenFSExt**
  - ```rust
    fn symlink<P: AsRef<Path>, Q: AsRef<Path>>(self: &Self, src: P, dst: Q) -> Result<()> { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> FS { /* ... */ }
    ```

- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Send**
- **Erased**
- **Unpin**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **UnwindSafe**
- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **GenFS**
  - ```rust
    fn canonicalize<P: AsRef<Path>>(self: &Self, path: P) -> Result<PathBuf> { /* ... */ }
    ```

  - ```rust
    fn copy<P: AsRef<Path>, Q: AsRef<Path>>(self: &Self, from: P, to: Q) -> Result<u64> { /* ... */ }
    ```

  - ```rust
    fn create_dir<P: AsRef<Path>>(self: &Self, path: P) -> Result<()> { /* ... */ }
    ```

  - ```rust
    fn create_dir_all<P: AsRef<Path>>(self: &Self, path: P) -> Result<()> { /* ... */ }
    ```

  - ```rust
    fn hard_link<P: AsRef<Path>, Q: AsRef<Path>>(self: &Self, src: P, dst: Q) -> Result<()> { /* ... */ }
    ```

  - ```rust
    fn metadata<P: AsRef<Path>>(self: &Self, path: P) -> Result<<Self as >::Metadata> { /* ... */ }
    ```

  - ```rust
    fn read_dir<P: AsRef<Path>>(self: &Self, path: P) -> Result<<Self as >::ReadDir> { /* ... */ }
    ```

  - ```rust
    fn read_link<P: AsRef<Path>>(self: &Self, path: P) -> Result<PathBuf> { /* ... */ }
    ```

  - ```rust
    fn remove_dir<P: AsRef<Path>>(self: &Self, path: P) -> Result<()> { /* ... */ }
    ```

  - ```rust
    fn remove_dir_all<P: AsRef<Path>>(self: &Self, path: P) -> Result<()> { /* ... */ }
    ```

  - ```rust
    fn remove_file<P: AsRef<Path>>(self: &Self, path: P) -> Result<()> { /* ... */ }
    ```

  - ```rust
    fn rename<P: AsRef<Path>, Q: AsRef<Path>>(self: &Self, from: P, to: Q) -> Result<()> { /* ... */ }
    ```

  - ```rust
    fn set_permissions<P: AsRef<Path>>(self: &Self, path: P, perm: <Self as >::Permissions) -> Result<()> { /* ... */ }
    ```

  - ```rust
    fn symlink_metadata<P: AsRef<Path>>(self: &Self, path: P) -> Result<<Self as >::Metadata> { /* ... */ }
    ```

  - ```rust
    fn new_openopts(self: &Self) -> <Self as >::OpenOptions { /* ... */ }
    ```

  - ```rust
    fn new_dirbuilder(self: &Self) -> <Self as >::DirBuilder { /* ... */ }
    ```

  - ```rust
    fn open_file<P: AsRef<Path>>(self: &Self, path: P) -> Result<<Self as >::File> { /* ... */ }
    ```

  - ```rust
    fn create_file<P: AsRef<Path>>(self: &Self, path: P) -> Result<<Self as >::File> { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **Freeze**
## Module `mem`

An in-memory filesystem.

The [`FS`] provides an in-memory file system. Only a Unix implementation is currently
available. Errors returned attempt to mimic true operating sytsem error codes, but may not
catch subtle differences between operating systems.

This module is platform specific and uses the proper in-memory semantics via a `pub use`
depending on the builder's operating system. As mentioned above, only Unix is currently
supported, meaning _this_ module will not work on Windows. To get a platform agnostic in-memory
filesystem, use the proper platform specific module. For example, if you use
[`rsfs::mem::unix`], you will have a cross-platform in-memory filesystem that obeys Unix
semantics. When `rsfs::mem::windows` is written, that can be used to get a cross-platform
Windows specific in-memory filesystem (additionally, once it is written, _this_ module will
work on Windows systems).

This module should provide a decent alternative to FUSE if there is no need to use your in
memory filesystem outside of your process.

# Example

```
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
```

[`FS`]: struct.FS.html
[`rsfs::mem::unix`]: unix/index.html
[`errors`]: ../errors/index.html

```rust
pub mod mem { /* ... */ }
```

### Modules

## Module `unix`

This module provides an in-memory filesystem that follows Unix semantics.

This module, when used directly, is cross-platform. This module imitates a Unix filesystem as
closely as possible, meaning if you create a directory without executable permissions, you
cannot do anything inside of it.

# Example

```
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::PathBuf;

use rsfs::*;
use rsfs::unix_ext::*;
use rsfs::mem::unix::FS;

let fs = FS::new();
assert!(fs.create_dir_all("a/b/c").is_ok());

// writing past the end of a file zero-extends to the write position
let mut wf = fs.create_file("a/f").unwrap();
assert_eq!(wf.write_at(b"hello", 100).unwrap(), 5);

let mut rf = fs.open_file("a/f").unwrap();
let mut output = [1u8; 5];
assert_eq!(rf.read(&mut output).unwrap(), 5);
assert_eq!(&output, &[0, 0, 0, 0, 0]);

assert_eq!(rf.seek(SeekFrom::Start(100)).unwrap(), 100);
assert_eq!(rf.read(&mut output).unwrap(), 5);
assert_eq!(&output, b"hello");
```

```rust
pub mod unix { /* ... */ }
```

### Types

#### Struct `DirBuilder`

A builder used to create directories in various manners.

This builder implements [`rsfs::DirBuilder`] and supports [unix extensions].

[`rsfs::DirBuilder`]: https://docs.rs/rsfs/0.4.1/rsfs/trait.DirBuilder.html
[unix extensions]: https://docs.rs/rsfs/0.4.1/rsfs/unix_ext/trait.DirBuilderExt.html

# Examples

```
# use rsfs::*;
# use rsfs::mem::FS;
# fn foo() -> std::io::Result<()> {
let fs = FS::new();
let db = fs.new_dirbuilder();
db.create("dir")?;
# Ok(())
# }
```

```rust
pub struct DirBuilder {
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Trait Implementations

- **RefUnwindSafe**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **Sync**
- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **DirBuilder**
  - ```rust
    fn recursive(self: &mut Self, recursive: bool) -> &mut Self { /* ... */ }
    ```

  - ```rust
    fn create<P: AsRef<Path>>(self: &Self, path: P) -> Result<()> { /* ... */ }
    ```

- **Erased**
- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Freeze**
- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **Unpin**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> DirBuilder { /* ... */ }
    ```

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Send**
- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **DirBuilderExt**
  - ```rust
    fn mode(self: &mut Self, mode: u32) -> &mut Self { /* ... */ }
    ```

- **UnwindSafe**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

#### Struct `DirEntry`

Entries returned by the [`ReadDir`] iterator.

An instance of `DirEntry` implements [`rsfs::DirEntry`] and represents an entry inside a
directory on the in-memory filesystem.

[`rsfs::DirEntry`]: https://docs.rs/rsfs/0.4.1/rsfs/trait.DirEntry.html
[`ReadDir`]: struct.ReadDir.html

# Examples

```
# use rsfs::*;
# use rsfs::mem::FS;
# fn foo() -> std::io::Result<()> {
let fs = FS::new();
for entry in fs.read_dir(".")? {
    let entry = entry?;
    println!("{:?}: {:?}", entry.path(), entry.metadata()?.permissions());
}
# Ok(())
# }
```

```rust
pub struct DirEntry {
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Trait Implementations

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Sync**
- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **Unpin**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **UnwindSafe**
- **Erased**
- **DirEntry**
  - ```rust
    fn path(self: &Self) -> PathBuf { /* ... */ }
    ```

  - ```rust
    fn metadata(self: &Self) -> Result<<Self as >::Metadata> { /* ... */ }
    ```

  - ```rust
    fn file_type(self: &Self) -> Result<<Self as >::FileType> { /* ... */ }
    ```

  - ```rust
    fn file_name(self: &Self) -> OsString { /* ... */ }
    ```

- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **RefUnwindSafe**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Freeze**
- **Send**
- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

#### Struct `File`

A view into a file on the filesystem.

An instance of `File` can be read or written to depending on the options it was opened with.
Files also implement `Seek` to alter the logical cursor position of the internal file.

This struct implements [`rsfs::File`] and has [unix extensions].

[`rsfs::File`]: https://docs.rs/rsfs/0.4.1/rsfs/trait.File.html
[unix extensions]: https://docs.rs/rsfs/0.4.1/rsfs/unix_ext/trait.FileExt.html

# Examples

```
# use rsfs::*;
# use rsfs::mem::FS;
# use std::io::Write;
# fn foo() -> std::io::Result<()> {
let fs = FS::new();
let mut f = fs.create_file("f")?;
assert_eq!(f.write(&[1, 2, 3])?, 3);
# Ok(())
# }
```

```rust
pub struct File {
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Trait Implementations

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **Erased**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Sync**
- **UnwindSafe**
- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **FileExt**
  - ```rust
    fn read_at(self: &Self, buf: &mut [u8], offset: u64) -> Result<usize> { /* ... */ }
    ```

  - ```rust
    fn write_at(self: &Self, buf: &[u8], offset: u64) -> Result<usize> { /* ... */ }
    ```

- **Unpin**
- **Seek**
  - ```rust
    fn seek(self: &mut Self, pos: SeekFrom) -> Result<u64> { /* ... */ }
    ```

  - ```rust
    fn seek(self: &mut Self, pos: SeekFrom) -> Result<u64> { /* ... */ }
    ```

- **Freeze**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **File**
  - ```rust
    fn sync_all(self: &Self) -> Result<()> { /* ... */ }
    ```

  - ```rust
    fn sync_data(self: &Self) -> Result<()> { /* ... */ }
    ```

  - ```rust
    fn set_len(self: &Self, size: u64) -> Result<()> { /* ... */ }
    ```

  - ```rust
    fn metadata(self: &Self) -> Result<<Self as >::Metadata> { /* ... */ }
    ```

  - ```rust
    fn try_clone(self: &Self) -> Result<Self> { /* ... */ }
    ```

  - ```rust
    fn set_permissions(self: &Self, perms: <Self as >::Permissions) -> Result<()> { /* ... */ }
    ```

- **Send**
- **Write**
  - ```rust
    fn write(self: &mut Self, buf: &[u8]) -> Result<usize> { /* ... */ }
    ```

  - ```rust
    fn flush(self: &mut Self) -> Result<()> { /* ... */ }
    ```

  - ```rust
    fn write(self: &mut Self, buf: &[u8]) -> Result<usize> { /* ... */ }
    ```

  - ```rust
    fn flush(self: &mut Self) -> Result<()> { /* ... */ }
    ```

- **RefUnwindSafe**
- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Read**
  - ```rust
    fn read(self: &mut Self, buf: &mut [u8]) -> Result<usize> { /* ... */ }
    ```

  - ```rust
    fn read(self: &mut Self, buf: &mut [u8]) -> Result<usize> { /* ... */ }
    ```

#### Struct `FileType`

Returned from [`Metadata::file_type`], this structure represents the type of a file.

This structure implements [`rsfs::FileType`]

[`Metadata::file_type`]: https://docs.rs/rsfs/0.4.1/rsfs/trait.Metadata.html#tymethod.file_type
[`rsfs::FileType`]: https://docs.rs/rsfs/0.4.1/rsfs/trait.FileType.html

# Examples

```
# use rsfs::*;
# use rsfs::mem::FS;
# fn foo() -> std::io::Result<()> {
let fs = FS::new();
let f = fs.create_file("f")?;
assert!(fs.metadata("f")?.file_type().is_file());
# Ok(())
# }
```

```rust
pub struct FileType(/* private field */);
```

##### Fields

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `private` | *Private field* |

##### Implementations

###### Trait Implementations

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Unpin**
- **Freeze**
- **Erased**
- **Sync**
- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Copy**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &FileType) -> bool { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Clone**
  - ```rust
    fn clone(self: &Self) -> FileType { /* ... */ }
    ```

- **FileType**
  - ```rust
    fn is_dir(self: &Self) -> bool { /* ... */ }
    ```

  - ```rust
    fn is_file(self: &Self) -> bool { /* ... */ }
    ```

  - ```rust
    fn is_symlink(self: &Self) -> bool { /* ... */ }
    ```

- **StructuralPartialEq**
- **UnwindSafe**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **Eq**
- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **Hash**
  - ```rust
    fn hash<__H: $crate::hash::Hasher>(self: &Self, state: &mut __H) { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

#### Struct `Metadata`

Metadata information about a file.

This structure, which implements [`rsfs::Metadata`], is returned from the [`metadata`] or
[`symlink_metadata`] methods and represents known metadata information about a file at the
instant in time this structure is instantiated.

[`rsfs::Metadata`]: https://docs.rs/rsfs/0.4.1/rsfs/trait.Metadata.html
[`metadata`]: https://docs.rs/rsfs/0.4.1/rsfs/trait.GenFS.html#tymethod.metadata
[`symlink_metadata`]: https://docs.rs/rsfs/0.4.1/rsfs/trait.GenFS.html#tymethod.symlink_metadata

# Examples

```
# use rsfs::*;
# use rsfs::mem::FS;
# fn foo() -> std::io::Result<()> {
let fs = FS::new();
fs.create_file("f")?;
println!("{:?}", fs.metadata("f")?);
# Ok(())
# }

```rust
pub struct Metadata(/* private field */);
```

##### Fields

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `private` | *Private field* |

##### Implementations

###### Trait Implementations

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **UnwindSafe**
- **Erased**
- **Unpin**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **RefUnwindSafe**
- **Send**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **Metadata**
  - ```rust
    fn file_type(self: &Self) -> <Self as >::FileType { /* ... */ }
    ```

  - ```rust
    fn is_dir(self: &Self) -> bool { /* ... */ }
    ```

  - ```rust
    fn is_file(self: &Self) -> bool { /* ... */ }
    ```

  - ```rust
    fn len(self: &Self) -> u64 { /* ... */ }
    ```

  - ```rust
    fn permissions(self: &Self) -> <Self as >::Permissions { /* ... */ }
    ```

  - ```rust
    fn modified(self: &Self) -> Result<SystemTime> { /* ... */ }
    ```

  - ```rust
    fn accessed(self: &Self) -> Result<SystemTime> { /* ... */ }
    ```

  - ```rust
    fn created(self: &Self) -> Result<SystemTime> { /* ... */ }
    ```

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Sync**
- **Freeze**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Metadata { /* ... */ }
    ```

#### Struct `OpenOptions`

Options and flags which can be used to configure how a file is opened.

This builder, created from `GenFS`s [`new_openopts`], exposes the ability to configure how a
[`File`] is opened and what operations are permitted on the open file. `GenFS`s [`open_file`]
and [`create_file`] methods are aliases for commonly used options with this builder.

This builder implements [`rsfs::OpenOptions`] and supports [unix extensions].

[`new_openopts`]: https://docs.rs/rsfs/0.4.1/rsfs/trait.GenFS.html#tymethod.new_openopts
[`open_file`]: https://docs.rs/rsfs/0.4.1/rsfs/trait.GenFS.html#tymethod.open_file
[`create_file`]: https://docs.rs/rsfs/0.4.1/rsfs/trait.GenFS.html#tymethod.create_file
[`rsfs::OpenOptions`]: https://docs.rs/rsfs/0.4.1/rsfs/trait.OpenOptions.html
[unix extensions]: https://docs.rs/rsfs/0.4.1/rsfs/unix_ext/trait.OpenOptionsExt.html

# Examples

Opening a file to read:

```
# use rsfs::*;
# use rsfs::mem::FS;
# fn foo() -> std::io::Result<()> {
# let fs = FS::new();
let f = fs.new_openopts()
          .read(true)
          .open("f")?;
# Ok(())
# }
```

Opening a file for both reading and writing, as well as creating it if it doesn't exist:

```
# use rsfs::*;
# use rsfs::mem::FS;
# fn foo() -> std::io::Result<()> {
# let fs = FS::new();
let mut f = fs.new_openopts()
              .read(true)
              .write(true)
              .create(true)
              .open("f")?;
# Ok(())
# }
```

```rust
pub struct OpenOptions {
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Trait Implementations

- **Unpin**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> OpenOptions { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Sync**
- **Freeze**
- **UnwindSafe**
- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **OpenOptions**
  - ```rust
    fn read(self: &mut Self, read: bool) -> &mut Self { /* ... */ }
    ```

  - ```rust
    fn write(self: &mut Self, write: bool) -> &mut Self { /* ... */ }
    ```

  - ```rust
    fn append(self: &mut Self, append: bool) -> &mut Self { /* ... */ }
    ```

  - ```rust
    fn truncate(self: &mut Self, truncate: bool) -> &mut Self { /* ... */ }
    ```

  - ```rust
    fn create(self: &mut Self, create: bool) -> &mut Self { /* ... */ }
    ```

  - ```rust
    fn create_new(self: &mut Self, create_new: bool) -> &mut Self { /* ... */ }
    ```

  - ```rust
    fn open<P: AsRef<Path>>(self: &Self, path: P) -> Result<<Self as >::File> { /* ... */ }
    ```

- **OpenOptionsExt**
  - ```rust
    fn mode(self: &mut Self, mode: u32) -> &mut Self { /* ... */ }
    ```

  - ```rust
    fn custom_flags(self: &mut Self, _: i32) -> &mut Self { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **Erased**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **RefUnwindSafe**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Send**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

#### Struct `Permissions`

Representation of the various permissions on a file.

This struct implements [`rsfs::Permissions`] and has [unix extensions].

[`rsfs::Permissions`]: https://docs.rs/rsfs/0.4.1/rsfs/trait.Permissions.html
[unix extensions]: https://docs.rs/rsfs/0.4.1/rsfs/unix_ext/trait.PermissionsExt.html

# Examples

```
# use rsfs::*;
# use rsfs::mem::FS;
use rsfs::unix_ext::*;
use rsfs::mem::Permissions;
# fn foo() -> std::io::Result<()> {
# let fs = FS::new();
# fs.create_file("foo.txt")?;

fs.set_permissions("foo.txt", Permissions::from_mode(0o400))?;
# Ok(())
# }
```

```rust
pub struct Permissions(/* private field */);
```

##### Fields

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `private` | *Private field* |

##### Implementations

###### Trait Implementations

- **Clone**
  - ```rust
    fn clone(self: &Self) -> Permissions { /* ... */ }
    ```

- **UnwindSafe**
- **RefUnwindSafe**
- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **PermissionsExt**
  - ```rust
    fn mode(self: &Self) -> u32 { /* ... */ }
    ```

  - ```rust
    fn set_mode(self: &mut Self, mode: u32) { /* ... */ }
    ```

  - ```rust
    fn from_mode(mode: u32) -> Self { /* ... */ }
    ```

- **StructuralPartialEq**
- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Erased**
- **PartialEq**
  - ```rust
    fn eq(self: &Self, other: &Permissions) -> bool { /* ... */ }
    ```

- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **Sync**
- **Copy**
- **Permissions**
  - ```rust
    fn readonly(self: &Self) -> bool { /* ... */ }
    ```

  - ```rust
    fn set_readonly(self: &mut Self, readonly: bool) { /* ... */ }
    ```

- **Eq**
- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Unpin**
- **Send**
- **Freeze**
- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

#### Struct `ReadDir`

Iterator over entries in a directory.

This is returned from the [`read_dir`] method of `GenFS` and yields instances of
`io::Result<DirEntry>`. Through a [`DirEntry`], information about contents of a directory can
be learned.

[`read_dir`]: struct.FS.html#method.read_dir
[`DirEntry`]: struct.DirEntry.html

```rust
pub struct ReadDir {
    // Some fields omitted
}
```

##### Fields

| Name | Type | Documentation |
|------|------|---------------|
| *private fields* | ... | *Some fields have been omitted* |

##### Implementations

###### Trait Implementations

- **Iterator**
  - ```rust
    fn next(self: &mut Self) -> Option<<Self as >::Item> { /* ... */ }
    ```

- **Sync**
- **Freeze**
- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **RefUnwindSafe**
- **Send**
- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **IntoIterator**
  - ```rust
    fn into_iter(self: Self) -> I { /* ... */ }
    ```

- **UnwindSafe**
- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **Unpin**
- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **Erased**
- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

#### Struct `FS`

An in-memory struct that satisfies [`rsfs::GenFS`].

`FS` is thread safe and copyable. It operates internally with an `Arc<Mutex<FileSystem>>`
(`FileSystem` not being exported) and forces all filesystem calls to go through the mutex. `FS`
attempts to mimic all real errors that could occur on a filesystem. Generally, unless a `FS` is
setup with restrictive permissions, errors will only be encountered when operating on
non-existent filesystem entries or performing invalid oprations.

See the module [documentation] or every struct's documentation for more examples of using an
`FS`.

[`rsfs::GenFS`]: https://docs.rs/rsfs/0.4.1/rsfs/trait.GenFS.html
[documentation]: index.html

# Examples

```
use rsfs::*;
use rsfs::mem::FS;

let fs = FS::new();
```

```rust
pub struct FS(/* private field */);
```

##### Fields

| Index | Type | Documentation |
|-------|------|---------------|
| 0 | `private` | *Private field* |

##### Implementations

###### Methods

- ```rust
  pub fn new() -> FS { /* ... */ }
  ```
  Creates an empty `FS` with mode `0o777`.

- ```rust
  pub fn with_mode(mode: u32) -> FS { /* ... */ }
  ```
  Creates an empty `FS` with the given mode.

###### Trait Implementations

- **RefUnwindSafe**
- **TryFrom**
  - ```rust
    fn try_from(value: U) -> Result<T, <T as TryFrom<U>>::Error> { /* ... */ }
    ```

- **Into**
  - ```rust
    fn into(self: Self) -> U { /* ... */ }
    ```
    Calls `U::from(self)`.

- **GenFS**
  - ```rust
    fn canonicalize<P: AsRef<Path>>(self: &Self, path: P) -> Result<PathBuf> { /* ... */ }
    ```

  - ```rust
    fn copy<P: AsRef<Path>, Q: AsRef<Path>>(self: &Self, from: P, to: Q) -> Result<u64> { /* ... */ }
    ```

  - ```rust
    fn create_dir<P: AsRef<Path>>(self: &Self, path: P) -> Result<()> { /* ... */ }
    ```

  - ```rust
    fn create_dir_all<P: AsRef<Path>>(self: &Self, path: P) -> Result<()> { /* ... */ }
    ```

  - ```rust
    fn hard_link<P: AsRef<Path>, Q: AsRef<Path>>(self: &Self, src: P, dst: Q) -> Result<()> { /* ... */ }
    ```

  - ```rust
    fn metadata<P: AsRef<Path>>(self: &Self, path: P) -> Result<<Self as >::Metadata> { /* ... */ }
    ```

  - ```rust
    fn read_dir<P: AsRef<Path>>(self: &Self, path: P) -> Result<<Self as >::ReadDir> { /* ... */ }
    ```

  - ```rust
    fn read_link<P: AsRef<Path>>(self: &Self, path: P) -> Result<PathBuf> { /* ... */ }
    ```

  - ```rust
    fn remove_dir<P: AsRef<Path>>(self: &Self, path: P) -> Result<()> { /* ... */ }
    ```

  - ```rust
    fn remove_dir_all<P: AsRef<Path>>(self: &Self, path: P) -> Result<()> { /* ... */ }
    ```

  - ```rust
    fn remove_file<P: AsRef<Path>>(self: &Self, path: P) -> Result<()> { /* ... */ }
    ```

  - ```rust
    fn rename<P: AsRef<Path>, Q: AsRef<Path>>(self: &Self, from: P, to: Q) -> Result<()> { /* ... */ }
    ```

  - ```rust
    fn set_permissions<P: AsRef<Path>>(self: &Self, path: P, perms: <Self as >::Permissions) -> Result<()> { /* ... */ }
    ```

  - ```rust
    fn symlink_metadata<P: AsRef<Path>>(self: &Self, path: P) -> Result<<Self as >::Metadata> { /* ... */ }
    ```

  - ```rust
    fn new_openopts(self: &Self) -> <Self as >::OpenOptions { /* ... */ }
    ```

  - ```rust
    fn new_dirbuilder(self: &Self) -> <Self as >::DirBuilder { /* ... */ }
    ```

  - ```rust
    fn open_file<P: AsRef<Path>>(self: &Self, path: P) -> Result<<Self as >::File> { /* ... */ }
    ```

  - ```rust
    fn create_file<P: AsRef<Path>>(self: &Self, path: P) -> Result<<Self as >::File> { /* ... */ }
    ```

- **Borrow**
  - ```rust
    fn borrow(self: &Self) -> &T { /* ... */ }
    ```

- **GenFSExt**
  - ```rust
    fn symlink<P: AsRef<Path>, Q: AsRef<Path>>(self: &Self, src: P, dst: Q) -> Result<()> { /* ... */ }
    ```

- **Default**
  - ```rust
    fn default() -> Self { /* ... */ }
    ```

- **Send**
- **Any**
  - ```rust
    fn type_id(self: &Self) -> TypeId { /* ... */ }
    ```

- **From**
  - ```rust
    fn from(t: T) -> T { /* ... */ }
    ```
    Returns the argument unchanged.

- **Debug**
  - ```rust
    fn fmt(self: &Self, f: &mut $crate::fmt::Formatter<''_>) -> $crate::fmt::Result { /* ... */ }
    ```

- **UnwindSafe**
- **ToOwned**
  - ```rust
    fn to_owned(self: &Self) -> T { /* ... */ }
    ```

  - ```rust
    fn clone_into(self: &Self, target: &mut T) { /* ... */ }
    ```

- **Erased**
- **Clone**
  - ```rust
    fn clone(self: &Self) -> FS { /* ... */ }
    ```

- **Freeze**
- **BorrowMut**
  - ```rust
    fn borrow_mut(self: &mut Self) -> &mut T { /* ... */ }
    ```

- **Sync**
- **Unpin**
- **TryInto**
  - ```rust
    fn try_into(self: Self) -> Result<U, <U as TryFrom<T>>::Error> { /* ... */ }
    ```

- **CloneToUninit**
  - ```rust
    unsafe fn clone_to_uninit(self: &Self, dest: *mut u8) { /* ... */ }
    ```

### Re-exports

#### Re-export `self::fs::*`

```rust
pub use self::fs::*;
```

## Module `unix_ext`

Unix specific traits that extend the traits in [`rsfs`].

These traits are separate from `rsfs` traits to ensure users of these traits opt-in to Unix
specific functionality.

# Examples

This module allows checking and using filesystem modes:

```
use rsfs::*;
use rsfs::unix_ext::*;
# fn foo() -> std::io::Result<()> {
let fs = rsfs::disk::FS;

assert_eq!(fs.metadata("/")?.permissions().mode(), 0o755);
# Ok(())
# }
```

We can also symlink files:

```
use rsfs::*;
use rsfs::unix_ext::*;
use rsfs::mem::FS;
# fn foo() -> std::io::Result<()> {
let fs = FS::new();

fs.symlink("a.txt", "b.txt")?;
# Ok(())
# }
```

There are even more useful Unix extensions!

[`rsfs`]: ../index.html

```rust
pub mod unix_ext { /* ... */ }
```

### Traits

#### Trait `DirBuilderExt`

Unix specific [`rsfs::DirBuilder`] extensions.

[`rsfs::DirBuilder`]: ../trait.DirBuilder.html

```rust
pub trait DirBuilderExt {
    /* Associated items */
}
```

> This trait is not object-safe and cannot be used in dynamic trait objects.

##### Required Items

###### Required Methods

- `mode`: Sets the mode bits to create new directories with. This option defaults to 0o777.

##### Implementations

This trait is implemented for the following types:

- `DirBuilder`
- `DirBuilder`
- `DirBuilder`

#### Trait `FileExt`

Unix specific [`rsfs::File`] extensions.

[`rsfs::File`]: ../trait.File.html

```rust
pub trait FileExt {
    /* Associated items */
}
```

##### Required Items

###### Required Methods

- `read_at`: Reads a number of bytes starting from the given offset, returning the number of bytes read.
- `write_at`: Writes a number of bytes starting from the given offset, returning the number of bytes

##### Implementations

This trait is implemented for the following types:

- `File`
- `File`
- `File`

#### Trait `OpenOptionsExt`

Unix specific [`rsfs::OpenOptions`] extensions.

[`rsfs::OpenOptions`]: ../trait.OpenOptions.html

```rust
pub trait OpenOptionsExt {
    /* Associated items */
}
```

> This trait is not object-safe and cannot be used in dynamic trait objects.

##### Required Items

###### Required Methods

- `mode`: Sets the mode bits that a new file will be opened with.
- `custom_flags`: Pass custom flags to the `flags` argument of `open`.

##### Implementations

This trait is implemented for the following types:

- `OpenOptions`
- `OpenOptions`
- `OpenOptions`

#### Trait `PermissionsExt`

Unix specific [`rsfs::Permissions`] extensions.

[`rsfs::Permissions`]: ../trait.Permissions.html

```rust
pub trait PermissionsExt {
    /* Associated items */
}
```

> This trait is not object-safe and cannot be used in dynamic trait objects.

##### Required Items

###### Required Methods

- `mode`: Returns the underlying Unix mode of these permissions.
- `set_mode`: Sets the underlying Unix mode for these permissions.
- `from_mode`: Creates a new Permissions from the given Unix mode.

##### Implementations

This trait is implemented for the following types:

- `Permissions`
- `Permissions`
- `Permissions`

#### Trait `GenFSExt`

Unix specific [`rsfs::GenFS`] extensions.

[`rsfs::GenFS`]: ../trait.GenFS.html

```rust
pub trait GenFSExt {
    /* Associated items */
}
```

> This trait is not object-safe and cannot be used in dynamic trait objects.

##### Required Items

###### Required Methods

- `symlink`: Creates a new symbolic link on the filesystem.

##### Implementations

This trait is implemented for the following types:

- `FS`
- `FS`
- `FS`

## Re-exports

### Re-export `fs::*`

```rust
pub use fs::*;
```

