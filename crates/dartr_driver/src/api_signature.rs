// Dart source: pkg/analyzer/lib/src/summary/api_signature.dart

//! [`ApiSignature`]: an MD5 hash over a sequence of typed values, used for
//! API signatures of files and library cycles and for cache keys.

use md5::{Digest, Md5};

/// Dart `ApiSignature`. Values are appended in little endian; the result is
/// the MD5 of the bytes.
#[derive(Clone, Debug, Default)]
pub struct ApiSignature {
    data: Vec<u8>,
}

/// Dart `ApiSignature._version`.
const VERSION: u32 = 0;

impl ApiSignature {
    /// Dart `ApiSignature()`: starts with the version.
    pub fn new() -> ApiSignature {
        let mut signature = ApiSignature::unversioned();
        signature.add_int(VERSION);
        signature
    }

    /// Dart `ApiSignature.unversioned()`.
    pub fn unversioned() -> ApiSignature {
        ApiSignature {
            data: Vec::with_capacity(4096),
        }
    }

    /// Dart `addBool`.
    pub fn add_bool(&mut self, b: bool) {
        self.data.push(b as u8);
    }

    /// Dart `addBytes`.
    pub fn add_bytes(&mut self, bytes: &[u8]) {
        self.data.extend_from_slice(bytes);
    }

    /// Dart `addInt` (a 32-bit unsigned value).
    pub fn add_int(&mut self, i: u32) {
        self.data.extend_from_slice(&i.to_le_bytes());
    }

    /// Dart `addLanguageVersion`.
    pub fn add_language_version(&mut self, major: u32, minor: u32) {
        self.add_int(major);
        self.add_int(minor);
    }

    /// Dart `addFeatureSet`: one bool per known experimental feature, in the
    /// order of `ExperimentStatus.knownFeatures`.
    pub fn add_feature_set(&mut self, is_enabled: impl Fn(&str) -> bool) {
        let known = dartr_project::experiments::KNOWN_FEATURES;
        self.add_int(known.len() as u32);
        for feature in known {
            self.add_bool(is_enabled(feature.enable_string));
        }
    }

    /// Dart `addString`: the length (in UTF-16 code units for ASCII, else in
    /// UTF-8 bytes) and the UTF-8 bytes. Both cases write the byte length
    /// for ASCII text, so this is the UTF-8 length and bytes.
    pub fn add_string(&mut self, s: &str) {
        self.add_int(s.len() as u32);
        self.data.extend_from_slice(s.as_bytes());
    }

    /// Dart `addStringList`.
    pub fn add_string_list(&mut self, values: &[impl AsRef<str>]) {
        self.add_int(values.len() as u32);
        for value in values {
            self.add_string(value.as_ref());
        }
    }

    /// Dart `addUint32List`.
    pub fn add_uint32_list(&mut self, data: &[u32]) {
        for v in data {
            self.data.extend_from_slice(&v.to_le_bytes());
        }
    }

    /// Dart `toByteList`: the MD5 of the bytes.
    pub fn to_byte_list(&self) -> [u8; 16] {
        let digest = Md5::digest(&self.data);
        let mut out = [0u8; 16];
        out.copy_from_slice(&digest);
        out
    }

    /// Dart `toHex`.
    pub fn to_hex(&self) -> String {
        hex(&self.to_byte_list())
    }

    /// Dart `toUint32List`.
    pub fn to_uint32_list(&self) -> [u32; 4] {
        let bytes = self.to_byte_list();
        std::array::from_fn(|i| u32::from_le_bytes(bytes[i * 4..i * 4 + 4].try_into().unwrap()))
    }
}

/// Dart `hex.encode` (lowercase).
pub fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        let _ = write!(s, "{b:02x}");
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn md5_of_version_and_string() {
        // Dart: (ApiSignature()..addString('abc')).toHex()
        // = md5([0,0,0,0, 3,0,0,0, 0x61,0x62,0x63]).
        let mut s = ApiSignature::new();
        s.add_string("abc");
        let expected = {
            let bytes = [0u8, 0, 0, 0, 3, 0, 0, 0, b'a', b'b', b'c'];
            hex(&Md5::digest(bytes))
        };
        assert_eq!(s.to_hex(), expected);
    }
}
