use std::fmt::Display;

use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use contract::IntegrityError;
use sha1::Sha1;
use sha2::{Digest, Sha256, Sha384, Sha512};

/// Ordered weakest to strongest, so the strongest listed algorithm is the max.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Algorithm {
    Sha1,
    Sha256,
    Sha384,
    Sha512,
}

/// The expected hash of a tarball, from a packument's `dist.integrity` or `dist.shasum`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Integrity {
    algorithm: Algorithm,
    digests: Vec<String>,
}

impl Integrity {
    /// Prefers the SRI `integrity` string, like npm, falling back to the legacy sha1 `shasum`.
    pub fn from_dist(
        integrity: Option<&str>,
        shasum: Option<&str>,
    ) -> Result<Option<Self>, IntegrityError> {
        match (integrity, shasum) {
            (Some(integrity), _) => Self::parse(integrity).map(Some),
            (None, Some(shasum)) => Self::from_shasum(shasum).map(Some),
            (None, None) => Ok(None),
        }
    }

    /// Parses an SRI string such as `sha512-<base64> sha1-<base64>`. Only hashes of the strongest
    /// supported algorithm are kept; any one of them matching is enough.
    pub fn parse(sri: &str) -> Result<Self, IntegrityError> {
        let hashes: Vec<(Algorithm, &str)> =
            sri.split_whitespace().filter_map(parse_hash).collect();
        let algorithm = hashes
            .iter()
            .map(|(algorithm, _)| *algorithm)
            .max()
            .ok_or_else(|| invalid(sri))?;
        let digests = hashes
            .into_iter()
            .filter(|(hash_algorithm, _)| *hash_algorithm == algorithm)
            .map(|(_, digest)| digest.to_string())
            .collect();

        Ok(Self { algorithm, digests })
    }

    pub fn sha512_of(bytes: &[u8]) -> Self {
        Self {
            algorithm: Algorithm::Sha512,
            digests: vec![digest(Algorithm::Sha512, bytes)],
        }
    }

    pub fn from_shasum(shasum: &str) -> Result<Self, IntegrityError> {
        let bytes = decode_hex(shasum).ok_or_else(|| invalid(shasum))?;
        Ok(Self {
            algorithm: Algorithm::Sha1,
            digests: vec![BASE64.encode(bytes)],
        })
    }

    pub fn verify(&self, bytes: &[u8]) -> Result<(), IntegrityError> {
        let mut hasher = self.hasher();
        hasher.update(bytes);
        hasher.verify()
    }

    /// Checks content fed in pieces, so a download can be verified while it streams to disk.
    pub fn hasher(&self) -> IntegrityHasher<'_> {
        let state = match self.algorithm {
            Algorithm::Sha1 => HasherState::Sha1(Sha1::new()),
            Algorithm::Sha256 => HasherState::Sha256(Sha256::new()),
            Algorithm::Sha384 => HasherState::Sha384(Sha384::new()),
            Algorithm::Sha512 => HasherState::Sha512(Sha512::new()),
        };
        IntegrityHasher {
            integrity: self,
            state,
        }
    }
}

pub struct IntegrityHasher<'a> {
    integrity: &'a Integrity,
    state: HasherState,
}

enum HasherState {
    Sha1(Sha1),
    Sha256(Sha256),
    Sha384(Sha384),
    Sha512(Sha512),
}

impl IntegrityHasher<'_> {
    pub fn update(&mut self, bytes: &[u8]) {
        match &mut self.state {
            HasherState::Sha1(hasher) => hasher.update(bytes),
            HasherState::Sha256(hasher) => hasher.update(bytes),
            HasherState::Sha384(hasher) => hasher.update(bytes),
            HasherState::Sha512(hasher) => hasher.update(bytes),
        }
    }

    pub fn verify(self) -> Result<(), IntegrityError> {
        let actual = match self.state {
            HasherState::Sha1(hasher) => BASE64.encode(hasher.finalize()),
            HasherState::Sha256(hasher) => BASE64.encode(hasher.finalize()),
            HasherState::Sha384(hasher) => BASE64.encode(hasher.finalize()),
            HasherState::Sha512(hasher) => BASE64.encode(hasher.finalize()),
        };
        if self.integrity.digests.contains(&actual) {
            return Ok(());
        }

        Err(IntegrityError::Mismatch {
            expected: self.integrity.to_string(),
            actual: format!("{}-{actual}", self.integrity.algorithm),
        })
    }
}

impl Display for Integrity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let digest = self.digests.first().map(String::as_str).unwrap_or_default();
        write!(f, "{}-{digest}", self.algorithm)
    }
}

impl Display for Algorithm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            Algorithm::Sha1 => "sha1",
            Algorithm::Sha256 => "sha256",
            Algorithm::Sha384 => "sha384",
            Algorithm::Sha512 => "sha512",
        };
        f.write_str(name)
    }
}

/// One `algorithm-digest[?options]` entry; unknown algorithms are skipped, as SRI requires.
fn parse_hash(hash: &str) -> Option<(Algorithm, &str)> {
    let (algorithm, digest) = hash.split_once('-')?;
    let digest = digest.split('?').next().unwrap_or(digest);
    let algorithm = match algorithm {
        "sha1" => Algorithm::Sha1,
        "sha256" => Algorithm::Sha256,
        "sha384" => Algorithm::Sha384,
        "sha512" => Algorithm::Sha512,
        _ => return None,
    };
    Some((algorithm, digest))
}

fn digest(algorithm: Algorithm, bytes: &[u8]) -> String {
    match algorithm {
        Algorithm::Sha1 => BASE64.encode(Sha1::digest(bytes)),
        Algorithm::Sha256 => BASE64.encode(Sha256::digest(bytes)),
        Algorithm::Sha384 => BASE64.encode(Sha384::digest(bytes)),
        Algorithm::Sha512 => BASE64.encode(Sha512::digest(bytes)),
    }
}

fn decode_hex(hex: &str) -> Option<Vec<u8>> {
    let is_even_length = hex.len().is_multiple_of(2);
    if !is_even_length {
        return None;
    }

    (0..hex.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(hex.get(index..index + 2)?, 16).ok())
        .collect()
}

fn invalid(integrity: &str) -> IntegrityError {
    IntegrityError::Invalid {
        integrity: integrity.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HELLO_SHA512: &str = "sha512-m3HSJL1i83hdltRq0+o9czGb+8KJDKra4t/3JRlnPKcjI8PZm6XBHXx6zG4UuMXaDEZjR1wuXDre9G9zvN7AQw==";
    const HELLO_SHA256: &str = "sha256-LPJNul+wow4m6DsqxbninhsWHlwfp0JecwQzYpOLmCQ=";
    const HELLO_SHASUM: &str = "aaf4c61ddcc5e8a2dabede0f3b482cd9aea9434d";

    #[test]
    fn test_verifies_sri_hashes() {
        assert!(
            Integrity::parse(HELLO_SHA512)
                .unwrap()
                .verify(b"hello")
                .is_ok()
        );
        assert!(
            Integrity::parse(HELLO_SHA256)
                .unwrap()
                .verify(b"hello")
                .is_ok()
        );
        assert!(
            Integrity::parse(HELLO_SHA512)
                .unwrap()
                .verify(b"hellO")
                .is_err()
        );
    }

    #[test]
    fn test_sha512_of_matches_sri() {
        assert_eq!(Integrity::sha512_of(b"hello").to_string(), HELLO_SHA512);
    }

    #[test]
    fn test_hashes_content_fed_in_pieces() {
        let integrity = Integrity::parse(HELLO_SHA512).unwrap();
        let mut hasher = integrity.hasher();
        hasher.update(b"he");
        hasher.update(b"llo");
        assert!(hasher.verify().is_ok());

        let mut wrong = integrity.hasher();
        wrong.update(b"hell");
        assert!(wrong.verify().is_err());
    }

    #[test]
    fn test_verifies_legacy_shasum() {
        let integrity = Integrity::from_dist(None, Some(HELLO_SHASUM))
            .unwrap()
            .unwrap();
        assert!(integrity.verify(b"hello").is_ok());
        assert!(integrity.verify(b"hellO").is_err());
    }

    #[test]
    fn test_prefers_integrity_over_shasum() {
        let wrong_shasum = "0000000000000000000000000000000000000000";
        let integrity = Integrity::from_dist(Some(HELLO_SHA512), Some(wrong_shasum))
            .unwrap()
            .unwrap();
        assert!(integrity.verify(b"hello").is_ok());
    }

    #[test]
    fn test_uses_the_strongest_listed_algorithm() {
        let sri = format!("sha1-wrong {HELLO_SHA512}?opt unknown-xyz");
        let integrity = Integrity::parse(&sri).unwrap();
        assert_eq!(integrity.to_string(), HELLO_SHA512);
        assert!(integrity.verify(b"hello").is_ok());
    }

    #[test]
    fn test_mismatch_names_both_hashes() {
        let error = Integrity::parse(HELLO_SHA256)
            .unwrap()
            .verify(b"hellO")
            .unwrap_err();
        let message = error.to_string();
        assert!(message.starts_with("tarball integrity mismatch: expected sha256-LPJN"));
        assert!(message.contains(", got sha256-"));
    }

    #[test]
    fn test_rejects_unsupported_or_malformed_values() {
        assert!(Integrity::parse("md5-abc").is_err());
        assert!(Integrity::parse("").is_err());
        assert!(Integrity::from_shasum("xyz").is_err());
        assert!(Integrity::from_shasum("abc").is_err());
        assert_eq!(Integrity::from_dist(None, None).unwrap(), None);
    }
}
