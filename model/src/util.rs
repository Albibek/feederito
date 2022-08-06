#[cfg(feature = "backend")]
use xxhash_rust::xxh3::xxh3_64;

#[cfg(feature = "backend")]
pub fn hash(data: &[u8]) -> u64 {
    xxh3_64(data)
}
