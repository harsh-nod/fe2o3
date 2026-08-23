//! Canonical, bounded generated source input for retained Verus execution.
//!
//! Construction authenticates content shape and identity only. The compiler-specific generator,
//! retained runtime, exact output parser, and refinement join remain separate owners.

use std::{error::Error, fmt};

use sha2::{Digest, Sha256};

/// Hard bound for one compiler-generated Verus proof harness.
pub const MAX_GENERATED_VERUS_PROOF_SOURCE_BYTES_V3: usize = 2 * 1024 * 1024;

const IDENTITY_DOMAIN_V3: &[u8] = b"fe2o3-generated-verus-proof-input-v3\0";

/// Domain-separated identity of one exact generated Verus source file.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
pub struct GeneratedVerusProofInputIdentityV3([u8; 32]);

impl GeneratedVerusProofInputIdentityV3 {
    /// Returns the exact identity bytes.
    pub const fn as_bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Move-only canonical generated source admitted for sealed execution.
#[derive(Debug, Eq, PartialEq)]
pub struct CanonicalGeneratedVerusProofInputV3 {
    source: Box<[u8]>,
    identity: GeneratedVerusProofInputIdentityV3,
}

impl CanonicalGeneratedVerusProofInputV3 {
    /// Validates bounded canonical ASCII Rust source and derives its exact identity.
    pub fn new(source: impl Into<Vec<u8>>) -> Result<Self, GeneratedVerusProofInputErrorV3> {
        let source = source.into();
        validate_source(&source)?;
        let identity = source_identity(&source);
        Ok(Self {
            source: source.into_boxed_slice(),
            identity,
        })
    }

    /// Returns the exact generated source bytes.
    pub fn source(&self) -> &[u8] {
        &self.source
    }

    /// Returns the exact source length.
    pub const fn byte_len(&self) -> u64 {
        self.source.len() as u64
    }

    /// Returns the domain-separated source identity.
    pub const fn identity(&self) -> GeneratedVerusProofInputIdentityV3 {
        self.identity
    }

    /// Content admission does not execute Verus or prove a compiler relationship.
    pub const fn authenticates_verus_execution(&self) -> bool {
        false
    }

    /// Content admission grants no artifact or runtime authority.
    pub const fn grants_artifact_or_runtime_authority(&self) -> bool {
        false
    }
}

fn validate_source(source: &[u8]) -> Result<(), GeneratedVerusProofInputErrorV3> {
    if source.is_empty() {
        return Err(GeneratedVerusProofInputErrorV3::Empty);
    }
    if source.len() > MAX_GENERATED_VERUS_PROOF_SOURCE_BYTES_V3 {
        return Err(GeneratedVerusProofInputErrorV3::TooLarge);
    }
    if !source.ends_with(b"\n") || source.ends_with(b"\n\n") {
        return Err(GeneratedVerusProofInputErrorV3::NonCanonicalFraming);
    }
    if source
        .iter()
        .any(|byte| !matches!(byte, b'\n' | b'\t' | 0x20..=0x7e))
    {
        return Err(GeneratedVerusProofInputErrorV3::NonCanonicalAscii);
    }
    for line in source.split(|byte| *byte == b'\n') {
        if line.last().is_some_and(|byte| byte.is_ascii_whitespace()) {
            return Err(GeneratedVerusProofInputErrorV3::TrailingWhitespace);
        }
    }
    Ok(())
}

fn source_identity(source: &[u8]) -> GeneratedVerusProofInputIdentityV3 {
    let mut digest = Sha256::new();
    digest.update(IDENTITY_DOMAIN_V3);
    digest.update((source.len() as u64).to_le_bytes());
    digest.update(source);
    GeneratedVerusProofInputIdentityV3(digest.finalize().into())
}

/// Canonical generated-source admission failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum GeneratedVerusProofInputErrorV3 {
    Empty,
    TooLarge,
    NonCanonicalFraming,
    NonCanonicalAscii,
    TrailingWhitespace,
}

impl fmt::Display for GeneratedVerusProofInputErrorV3 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "generated Verus proof input rejected: {self:?}")
    }
}

impl Error for GeneratedVerusProofInputErrorV3 {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_source_has_stable_content_identity_without_authority() {
        let first =
            CanonicalGeneratedVerusProofInputV3::new(b"verus! { proof fn p() {} }\n".to_vec())
                .unwrap();
        let second = CanonicalGeneratedVerusProofInputV3::new(first.source().to_vec()).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.byte_len(), 27);
        assert_ne!(first.identity().as_bytes(), [0; 32]);
        assert!(!first.authenticates_verus_execution());
        assert!(!first.grants_artifact_or_runtime_authority());

        let changed =
            CanonicalGeneratedVerusProofInputV3::new(b"verus! { proof fn q() {} }\n".to_vec())
                .unwrap();
        assert_ne!(first.identity(), changed.identity());
    }

    #[test]
    fn malformed_noncanonical_and_unbounded_sources_fail_closed() {
        for (source, expected) in [
            (Vec::new(), GeneratedVerusProofInputErrorV3::Empty),
            (
                b"verus! { proof fn p() {} }".to_vec(),
                GeneratedVerusProofInputErrorV3::NonCanonicalFraming,
            ),
            (
                b"verus! { proof fn p() {} }\n\n".to_vec(),
                GeneratedVerusProofInputErrorV3::NonCanonicalFraming,
            ),
            (
                b"verus! { proof fn p() {} }\r\n".to_vec(),
                GeneratedVerusProofInputErrorV3::NonCanonicalAscii,
            ),
            (
                b"verus! { proof fn p() {} } \n".to_vec(),
                GeneratedVerusProofInputErrorV3::TrailingWhitespace,
            ),
        ] {
            assert_eq!(
                CanonicalGeneratedVerusProofInputV3::new(source).unwrap_err(),
                expected
            );
        }
        assert_eq!(
            CanonicalGeneratedVerusProofInputV3::new(vec![
                b'x';
                MAX_GENERATED_VERUS_PROOF_SOURCE_BYTES_V3
                    + 1
            ])
            .unwrap_err(),
            GeneratedVerusProofInputErrorV3::TooLarge
        );
    }
}
