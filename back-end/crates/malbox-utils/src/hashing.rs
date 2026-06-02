use crc32fast::Hasher;
use md5::compute;
use sha1::Sha1;
use sha2::{Digest, Sha256, Sha512};

// Hashes read their input; they do not mutate it. Taking `&[u8]` lets callers
// hash a borrowed slice (e.g. `Bytes`/`Vec<u8>`) without copying it first.
//
// All digests are rendered as fixed-width, zero-padded lowercase hex. The
// per-byte `format!("{:x}", b)` previously used here dropped the leading zero
// of any byte < 0x10, producing malformed, variable-length hashes; the
// `{:0Nx}`/whole-digest `{:x}` forms below pad correctly.

pub fn get_md5(buf: &[u8]) -> String {
    // md5's `Digest` renders as full 32-char padded hex via its `LowerHex` impl.
    format!("{:x}", compute(buf))
}

pub fn get_sha1(buf: &[u8]) -> String {
    let mut hasher = Sha1::new();
    hasher.update(buf);
    format!("{:x}", hasher.finalize())
}

pub fn get_sha256(buf: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(buf);
    format!("{:x}", hasher.finalize())
}

pub fn get_sha512(buf: &[u8]) -> String {
    let mut hasher = Sha512::new();
    hasher.update(buf);
    format!("{:x}", hasher.finalize())
}

pub fn get_crc32(buf: &[u8]) -> String {
    let mut hasher = Hasher::new();
    hasher.update(buf);
    // CRC-32 is a 32-bit value: render as 8 zero-padded hex chars.
    format!("{:08x}", hasher.finalize())
}

// NOTE: temporarly removing ssdeep crate because of build issues..
// pub fn get_ssdeep(buf: &[u8]) -> String {
//    ssdeep::hash(buf).unwrap()
// }
