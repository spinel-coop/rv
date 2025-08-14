use std::hash::Hasher;

use camino::{Utf8Path, Utf8PathBuf};
use seahash::SeaHasher;

/// A trait for types that can be hashed in a stable way across versions and platforms.
pub trait CacheKey {
    fn cache_key(&self, state: &mut CacheKeyHasher);
}

impl CacheKey for str {
    fn cache_key(&self, state: &mut CacheKeyHasher) {
        state.write_usize(self.len());
        state.write(self.as_bytes());
    }
}

impl CacheKey for String {
    fn cache_key(&self, state: &mut CacheKeyHasher) {
        self.as_str().cache_key(state);
    }
}

impl CacheKey for u32 {
    fn cache_key(&self, state: &mut CacheKeyHasher) {
        state.write_u32(*self);
    }
}

impl CacheKey for u64 {
    fn cache_key(&self, state: &mut CacheKeyHasher) {
        state.write_u64(*self);
    }
}

impl CacheKey for i32 {
    fn cache_key(&self, state: &mut CacheKeyHasher) {
        state.write_i32(*self);
    }
}

impl CacheKey for i64 {
    fn cache_key(&self, state: &mut CacheKeyHasher) {
        state.write_i64(*self);
    }
}

impl CacheKey for bool {
    fn cache_key(&self, state: &mut CacheKeyHasher) {
        state.write_u8(if *self { 1 } else { 0 });
    }
}

impl CacheKey for u8 {
    fn cache_key(&self, state: &mut CacheKeyHasher) {
        state.write_u8(*self);
    }
}

impl CacheKey for u16 {
    fn cache_key(&self, state: &mut CacheKeyHasher) {
        state.write_u16(*self);
    }
}

impl CacheKey for u128 {
    fn cache_key(&self, state: &mut CacheKeyHasher) {
        state.write_u128(*self);
    }
}

impl CacheKey for usize {
    fn cache_key(&self, state: &mut CacheKeyHasher) {
        state.write_usize(*self);
    }
}

impl CacheKey for i8 {
    fn cache_key(&self, state: &mut CacheKeyHasher) {
        state.write_i8(*self);
    }
}

impl CacheKey for i16 {
    fn cache_key(&self, state: &mut CacheKeyHasher) {
        state.write_i16(*self);
    }
}

impl CacheKey for i128 {
    fn cache_key(&self, state: &mut CacheKeyHasher) {
        state.write_i128(*self);
    }
}

impl CacheKey for isize {
    fn cache_key(&self, state: &mut CacheKeyHasher) {
        state.write_isize(*self);
    }
}

impl CacheKey for Utf8Path {
    fn cache_key(&self, state: &mut CacheKeyHasher) {
        self.as_str().cache_key(state);
    }
}

impl CacheKey for Utf8PathBuf {
    fn cache_key(&self, state: &mut CacheKeyHasher) {
        self.as_path().cache_key(state);
    }
}

impl<T: ?Sized + CacheKey> CacheKey for &T {
    fn cache_key(&self, state: &mut CacheKeyHasher) {
        (*self).cache_key(state);
    }
}

impl<T: CacheKey> CacheKey for Option<T> {
    fn cache_key(&self, state: &mut CacheKeyHasher) {
        match self {
            None => state.write_u8(0),
            Some(value) => {
                state.write_u8(1);
                value.cache_key(state);
            }
        }
    }
}

impl<T: CacheKey> CacheKey for Vec<T> {
    fn cache_key(&self, state: &mut CacheKeyHasher) {
        state.write_usize(self.len());
        for item in self {
            item.cache_key(state);
        }
    }
}

impl<T: CacheKey> CacheKey for [T] {
    fn cache_key(&self, state: &mut CacheKeyHasher) {
        state.write_usize(self.len());
        for item in self {
            item.cache_key(state);
        }
    }
}

/// A stable hasher for cache keys using SeaHash
#[derive(Clone, Default)]
pub struct CacheKeyHasher {
    inner: SeaHasher,
}

impl CacheKeyHasher {
    pub fn new() -> Self {
        Self {
            inner: SeaHasher::new(),
        }
    }

    /// Generate a stable hash for any cacheable data
    pub fn hash_one<T: CacheKey>(value: T) -> u64 {
        let mut hasher = Self::new();
        value.cache_key(&mut hasher);
        hasher.finish()
    }
}

impl Hasher for CacheKeyHasher {
    fn finish(&self) -> u64 {
        self.inner.finish()
    }

    fn write(&mut self, bytes: &[u8]) {
        self.inner.write(bytes);
    }

    fn write_u8(&mut self, i: u8) {
        self.inner.write_u8(i);
    }

    fn write_u16(&mut self, i: u16) {
        self.inner.write_u16(i);
    }

    fn write_u32(&mut self, i: u32) {
        self.inner.write_u32(i);
    }

    fn write_u64(&mut self, i: u64) {
        self.inner.write_u64(i);
    }

    fn write_u128(&mut self, i: u128) {
        self.inner.write_u128(i);
    }

    fn write_usize(&mut self, i: usize) {
        self.inner.write_usize(i);
    }

    fn write_i8(&mut self, i: i8) {
        self.inner.write_i8(i);
    }

    fn write_i16(&mut self, i: i16) {
        self.inner.write_i16(i);
    }

    fn write_i32(&mut self, i: i32) {
        self.inner.write_i32(i);
    }

    fn write_i64(&mut self, i: i64) {
        self.inner.write_i64(i);
    }

    fn write_i128(&mut self, i: i128) {
        self.inner.write_i128(i);
    }

    fn write_isize(&mut self, i: isize) {
        self.inner.write_isize(i);
    }
}

macro_rules! impl_cache_key_tuple {
    () => (
        impl CacheKey for () {
            #[inline]
            fn cache_key(&self, _state: &mut CacheKeyHasher) {}
        }
    );

    ( $($name:ident)+) => (
        impl<$($name: CacheKey),+> CacheKey for ($($name,)+) where last_type!($($name,)+): ?Sized {
            #[allow(non_snake_case)]
            #[inline]
            fn cache_key(&self, state: &mut CacheKeyHasher) {
                let ($(ref $name,)+) = *self;
                $($name.cache_key(state);)+
            }
        }
    );
}

macro_rules! last_type {
    ($a:ident,) => { $a };
    ($a:ident, $($rest_a:ident,)+) => { last_type!($($rest_a,)+) };
}

impl_cache_key_tuple! {}
impl_cache_key_tuple! { T }
impl_cache_key_tuple! { T B }
impl_cache_key_tuple! { T B C }
impl_cache_key_tuple! { T B C D }
impl_cache_key_tuple! { T B C D E }
impl_cache_key_tuple! { T B C D E F }
impl_cache_key_tuple! { T B C D E F G }
impl_cache_key_tuple! { T B C D E F G H }
impl_cache_key_tuple! { T B C D E F G H I }
impl_cache_key_tuple! { T B C D E F G H I J }
impl_cache_key_tuple! { T B C D E F G H I J K }
impl_cache_key_tuple! { T B C D E F G H I J K L }

/// Generate a cache digest for any cacheable value
pub fn cache_digest<T: CacheKey>(value: T) -> String {
    let hash = CacheKeyHasher::hash_one(value);
    format!("{hash:x}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_digest() {
        let digest1 = cache_digest("test");
        let digest2 = cache_digest("test");
        let digest3 = cache_digest("different");

        assert_eq!(digest1, digest2);
        assert_ne!(digest1, digest3);
    }

    #[test]
    fn test_utf8_path_cache_key() {
        let path1 = Utf8Path::new("/usr/local/bin/ruby");
        let path2 = Utf8Path::new("/usr/local/bin/ruby");
        let path3 = Utf8Path::new("/usr/local/bin/python");

        let hash1 = CacheKeyHasher::hash_one(path1);
        let hash2 = CacheKeyHasher::hash_one(path2);
        let hash3 = CacheKeyHasher::hash_one(path3);

        assert_eq!(hash1, hash2);
        assert_ne!(hash1, hash3);
    }

    #[test]
    fn test_option_cache_key() {
        let some_value = Some("test");
        let none_value: Option<&str> = None;

        let hash1 = CacheKeyHasher::hash_one(some_value);
        let hash2 = CacheKeyHasher::hash_one(none_value);

        assert_ne!(hash1, hash2);
    }

    #[test]
    fn test_vec_cache_key() {
        let vec1 = vec!["a", "b", "c"];
        let vec2 = vec!["a", "b", "c"];
        let vec3 = vec!["a", "b"];

        let hash1 = CacheKeyHasher::hash_one(vec1);
        let hash2 = CacheKeyHasher::hash_one(vec2);
        let hash3 = CacheKeyHasher::hash_one(vec3);

        assert_eq!(hash1, hash2);
        assert_ne!(hash1, hash3);
    }

    #[test]
    fn test_integer_cache_key() {
        let hash1 = CacheKeyHasher::hash_one(42u32);
        let hash2 = CacheKeyHasher::hash_one(42u32);
        let hash3 = CacheKeyHasher::hash_one(43u32);

        assert_eq!(hash1, hash2);
        assert_ne!(hash1, hash3);

        // Different sizes should produce different hashes
        let hash_u32 = CacheKeyHasher::hash_one(42u32);
        let hash_u64 = CacheKeyHasher::hash_one(42u64);
        assert_ne!(hash_u32, hash_u64);

        // Test that some distinctly sized integer types produce different hashes
        let value = 123;
        let hash_u8 = CacheKeyHasher::hash_one(value as u8);
        let hash_u32 = CacheKeyHasher::hash_one(value as u32);
        let hash_u128 = CacheKeyHasher::hash_one(value as u128);

        // These should definitely be different due to different write methods
        assert_ne!(hash_u8, hash_u32);
        assert_ne!(hash_u32, hash_u128);
        assert_ne!(hash_u8, hash_u128);
    }

    #[test]
    fn test_bool_cache_key() {
        let hash_true = CacheKeyHasher::hash_one(true);
        let hash_false = CacheKeyHasher::hash_one(false);

        assert_ne!(hash_true, hash_false);

        // Same values should produce same hashes
        assert_eq!(hash_true, CacheKeyHasher::hash_one(true));
        assert_eq!(hash_false, CacheKeyHasher::hash_one(false));
    }

    #[test]
    fn test_utf8_pathbuf_cache_key() {
        let path1 = Utf8PathBuf::from("/usr/local/bin");
        let path2 = Utf8PathBuf::from("/usr/local/bin");
        let path3 = Utf8PathBuf::from("/usr/local/lib");

        let hash1 = CacheKeyHasher::hash_one(&path1);
        let hash2 = CacheKeyHasher::hash_one(&path2);
        let hash3 = CacheKeyHasher::hash_one(&path3);

        assert_eq!(hash1, hash2);
        assert_ne!(hash1, hash3);
    }

    #[test]
    fn test_tuple_cache_key() {
        // Test empty tuple
        let empty_tuple = ();
        let hash_empty = CacheKeyHasher::hash_one(empty_tuple);
        assert_eq!(hash_empty, CacheKeyHasher::hash_one(empty_tuple));

        // Test single element tuple
        let tuple1 = ("hello",);
        let tuple2 = ("hello",);
        let tuple3 = ("world",);

        let hash1 = CacheKeyHasher::hash_one(tuple1);
        let hash2 = CacheKeyHasher::hash_one(tuple2);
        let hash3 = CacheKeyHasher::hash_one(tuple3);

        assert_eq!(hash1, hash2);
        assert_ne!(hash1, hash3);

        // Test multi-element tuples
        let tuple_multi1 = ("hello", 42u32, true);
        let tuple_multi2 = ("hello", 42u32, true);
        let tuple_multi3 = ("hello", 42u32, false);

        let hash_multi1 = CacheKeyHasher::hash_one(tuple_multi1);
        let hash_multi2 = CacheKeyHasher::hash_one(tuple_multi2);
        let hash_multi3 = CacheKeyHasher::hash_one(tuple_multi3);

        assert_eq!(hash_multi1, hash_multi2);
        assert_ne!(hash_multi1, hash_multi3);

        // Test that order matters
        let tuple_order1 = (42u32, "hello");
        let tuple_order2 = ("hello", 42u32);

        let hash_order1 = CacheKeyHasher::hash_one(tuple_order1);
        let hash_order2 = CacheKeyHasher::hash_one(tuple_order2);

        assert_ne!(hash_order1, hash_order2);
    }

    #[test]
    fn test_slice_cache_key() {
        let slice1: &[&str] = &["a", "b", "c"];
        let slice2: &[&str] = &["a", "b", "c"];
        let slice3: &[&str] = &["a", "b"];

        let hash1 = CacheKeyHasher::hash_one(slice1);
        let hash2 = CacheKeyHasher::hash_one(slice2);
        let hash3 = CacheKeyHasher::hash_one(slice3);

        assert_eq!(hash1, hash2);
        assert_ne!(hash1, hash3);

        // Test empty slice
        let empty_slice: &[&str] = &[];
        let hash_empty1 = CacheKeyHasher::hash_one(empty_slice);
        let hash_empty2 = CacheKeyHasher::hash_one(empty_slice);
        assert_eq!(hash_empty1, hash_empty2);
    }

    #[test]
    fn test_reference_cache_key() {
        let value = "test_string";
        let reference = &value;

        let hash1 = CacheKeyHasher::hash_one(value);
        let hash2 = CacheKeyHasher::hash_one(reference);

        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_nested_collections() {
        let vec_of_vecs1 = vec![vec![1u32, 2u32], vec![3u32, 4u32]];
        let vec_of_vecs2 = vec![vec![1u32, 2u32], vec![3u32, 4u32]];
        let vec_of_vecs3 = vec![vec![1u32, 2u32], vec![3u32, 5u32]];

        let hash1 = CacheKeyHasher::hash_one(&vec_of_vecs1);
        let hash2 = CacheKeyHasher::hash_one(&vec_of_vecs2);
        let hash3 = CacheKeyHasher::hash_one(&vec_of_vecs3);

        assert_eq!(hash1, hash2);
        assert_ne!(hash1, hash3);
    }

    #[test]
    fn test_cache_key_stability() {
        // Test that the same data always produces the same hash
        let data = ("test", 42u64, vec!["a", "b", "c"]);

        let hash1 = CacheKeyHasher::hash_one(&data);
        let hash2 = CacheKeyHasher::hash_one(&data);
        let hash3 = CacheKeyHasher::hash_one(&data);

        assert_eq!(hash1, hash2);
        assert_eq!(hash2, hash3);

        // Hash should be non-zero for non-trivial data
        assert_ne!(hash1, 0);
    }
}
