//! Inert native-neutral subject framing, distinct from every legacy KIR identity.

use std::{error::Error, fmt};

use sha2::{Digest, Sha256};

/// Exact bytes in the allocation-free native-neutral subject frame.
pub const NATIVE_NEUTRAL_SUBJECT_BYTES_V1: usize = 96;
/// Exact native-neutral subject wire version.
pub const NATIVE_NEUTRAL_SUBJECT_VERSION_V1: u16 = 1;
/// Exact graph-plus-contract-catalog subject policy.
pub const NATIVE_NEUTRAL_SUBJECT_POLICY_V1: u16 = 1;

const MAGIC: [u8; 8] = *b"F2NNAT1\0";
const DOMAIN: &[u8] = b"FE2O3/NATIVE-NEUTRAL-GRAPH-AND-CONTRACT-CATALOG/V1\0";
const GRAPH_VERSION: u16 = 12;

/// Content identity of a native-neutral subject frame, not of either constituent.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct NativeNeutralSubjectIdentityV1([u8; 32]);

impl NativeNeutralSubjectIdentityV1 {
    /// Returns the domain-separated identity of the complete 96-byte frame.
    pub const fn digest(&self) -> &[u8; 32] {
        &self.0
    }

    /// Returns the fixed length committed by this identity.
    pub const fn canonical_length(self) -> u64 {
        NATIVE_NEUTRAL_SUBJECT_BYTES_V1 as u64
    }
}

/// Native V12 graph and catalog coordinates, with no admission or proof authority.
///
/// Construction and decoding validate framing only. Consumers must separately
/// admit the exact graph, decode the catalog, and check catalog definitions and
/// event operands against that graph. Equal coordinates do not transfer custody.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InertNativeNeutralSubjectV1 {
    canonical: [u8; NATIVE_NEUTRAL_SUBJECT_BYTES_V1],
    identity: NativeNeutralSubjectIdentityV1,
    graph_length: u64,
    graph_digest: [u8; 32],
    catalog_length: u64,
    catalog_digest: [u8; 32],
}

impl InertNativeNeutralSubjectV1 {
    /// Frames already identified graph/catalog bytes without granting custody.
    pub fn new(
        graph_digest: [u8; 32],
        graph_length: u64,
        catalog_digest: [u8; 32],
        catalog_length: u64,
    ) -> Result<Self, NativeNeutralSubjectErrorV1> {
        if graph_digest == [0; 32] || catalog_digest == [0; 32] {
            return Err(NativeNeutralSubjectErrorV1::ZeroIdentity);
        }
        if graph_length == 0 || catalog_length == 0 {
            return Err(NativeNeutralSubjectErrorV1::ZeroLength);
        }
        let mut canonical = [0; NATIVE_NEUTRAL_SUBJECT_BYTES_V1];
        canonical[..8].copy_from_slice(&MAGIC);
        canonical[8..10].copy_from_slice(&NATIVE_NEUTRAL_SUBJECT_VERSION_V1.to_le_bytes());
        canonical[10..12].copy_from_slice(&NATIVE_NEUTRAL_SUBJECT_POLICY_V1.to_le_bytes());
        canonical[12..14].copy_from_slice(&GRAPH_VERSION.to_le_bytes());
        canonical[16..24].copy_from_slice(&graph_length.to_le_bytes());
        canonical[24..56].copy_from_slice(&graph_digest);
        canonical[56..64].copy_from_slice(&catalog_length.to_le_bytes());
        canonical[64..96].copy_from_slice(&catalog_digest);
        let mut hash = Sha256::new();
        hash.update(DOMAIN);
        hash.update((NATIVE_NEUTRAL_SUBJECT_BYTES_V1 as u64).to_le_bytes());
        hash.update(canonical);
        Ok(Self {
            canonical,
            identity: NativeNeutralSubjectIdentityV1(hash.finalize().into()),
            graph_length,
            graph_digest,
            catalog_length,
            catalog_digest,
        })
    }

    /// Strictly decodes this schema only; legacy KIR or evidence is not accepted.
    pub fn decode(bytes: &[u8]) -> Result<Self, NativeNeutralSubjectErrorV1> {
        let canonical: &[u8; NATIVE_NEUTRAL_SUBJECT_BYTES_V1] = bytes
            .try_into()
            .map_err(|_| NativeNeutralSubjectErrorV1::Length)?;
        if canonical[..8] != MAGIC
            || canonical[8..10] != NATIVE_NEUTRAL_SUBJECT_VERSION_V1.to_le_bytes()
            || canonical[10..12] != NATIVE_NEUTRAL_SUBJECT_POLICY_V1.to_le_bytes()
            || canonical[12..14] != GRAPH_VERSION.to_le_bytes()
            || canonical[14..16] != [0; 2]
        {
            return Err(NativeNeutralSubjectErrorV1::Header);
        }
        let mut graph_length = [0; 8];
        graph_length.copy_from_slice(&canonical[16..24]);
        let mut graph_digest = [0; 32];
        graph_digest.copy_from_slice(&canonical[24..56]);
        let mut catalog_length = [0; 8];
        catalog_length.copy_from_slice(&canonical[56..64]);
        let mut catalog_digest = [0; 32];
        catalog_digest.copy_from_slice(&canonical[64..96]);
        Self::new(
            graph_digest,
            u64::from_le_bytes(graph_length),
            catalog_digest,
            u64::from_le_bytes(catalog_length),
        )
    }

    /// Returns the complete canonical graph-plus-catalog frame.
    pub const fn canonical_bytes(&self) -> &[u8; NATIVE_NEUTRAL_SUBJECT_BYTES_V1] {
        &self.canonical
    }

    /// Returns the subject identity, which is not the graph identity.
    pub const fn identity(&self) -> NativeNeutralSubjectIdentityV1 {
        self.identity
    }

    /// Returns the only admitted graph wire version for this framing schema.
    pub const fn graph_wire_version(&self) -> u16 {
        GRAPH_VERSION
    }

    /// Returns the domain-separated canonical V12 graph digest.
    pub const fn graph_digest(&self) -> &[u8; 32] {
        &self.graph_digest
    }

    /// Returns the exact graph byte length.
    pub const fn graph_length(&self) -> u64 {
        self.graph_length
    }

    /// Returns the separately domain-separated contract catalog digest.
    pub const fn catalog_digest(&self) -> &[u8; 32] {
        &self.catalog_digest
    }

    /// Returns the exact contract catalog byte length, including an empty header.
    pub const fn catalog_length(&self) -> u64 {
        self.catalog_length
    }

    /// Framing alone grants no graph, verification, publication, or launch authority.
    pub const fn grants_authority(&self) -> bool {
        false
    }
}

/// Invalid framing or inert constituent coordinates.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeNeutralSubjectErrorV1 {
    /// The input has a truncated or trailing byte.
    Length,
    /// Magic, version, policy, graph version, or reserved bytes differ.
    Header,
    /// A constituent digest is all zero.
    ZeroIdentity,
    /// A constituent has zero byte length.
    ZeroLength,
}

impl fmt::Display for NativeNeutralSubjectErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Length => "native-neutral subject has an invalid length",
            Self::Header => "native-neutral subject has an invalid header",
            Self::ZeroIdentity => "native-neutral subject has a zero constituent identity",
            Self::ZeroLength => "native-neutral subject has a zero constituent length",
        })
    }
}

impl Error for NativeNeutralSubjectErrorV1 {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_commits_each_constituent_and_length() {
        let subject = InertNativeNeutralSubjectV1::new([1; 32], 123, [2; 32], 456).unwrap();
        assert_eq!(
            InertNativeNeutralSubjectV1::decode(subject.canonical_bytes()),
            Ok(subject)
        );
        assert_eq!(subject.graph_wire_version(), 12);
        assert!(!subject.grants_authority());
        assert_eq!(subject.identity().canonical_length(), 96);
        for other in [
            InertNativeNeutralSubjectV1::new([3; 32], 123, [2; 32], 456),
            InertNativeNeutralSubjectV1::new([1; 32], 124, [2; 32], 456),
            InertNativeNeutralSubjectV1::new([1; 32], 123, [3; 32], 456),
            InertNativeNeutralSubjectV1::new([1; 32], 123, [2; 32], 457),
        ] {
            assert_ne!(other.unwrap().identity(), subject.identity());
        }
    }

    #[test]
    fn rejects_all_header_mutations_and_nonexact_lengths() {
        let subject = InertNativeNeutralSubjectV1::new([1; 32], 123, [2; 32], 456).unwrap();
        for index in 0..16 {
            let mut bytes = *subject.canonical_bytes();
            bytes[index] ^= 1;
            assert_eq!(
                InertNativeNeutralSubjectV1::decode(&bytes),
                Err(NativeNeutralSubjectErrorV1::Header)
            );
        }
        for end in 0..96 {
            assert_eq!(
                InertNativeNeutralSubjectV1::decode(&subject.canonical_bytes()[..end]),
                Err(NativeNeutralSubjectErrorV1::Length)
            );
        }
        assert_eq!(
            InertNativeNeutralSubjectV1::decode(&[0; 97]),
            Err(NativeNeutralSubjectErrorV1::Length)
        );
    }

    #[test]
    fn rejects_zero_fields_and_legacy_graph_version() {
        for (graph_digest, graph_length, catalog_digest, catalog_length, error) in [
            (
                [0; 32],
                1,
                [2; 32],
                1,
                NativeNeutralSubjectErrorV1::ZeroIdentity,
            ),
            (
                [1; 32],
                1,
                [0; 32],
                1,
                NativeNeutralSubjectErrorV1::ZeroIdentity,
            ),
            (
                [1; 32],
                0,
                [2; 32],
                1,
                NativeNeutralSubjectErrorV1::ZeroLength,
            ),
            (
                [1; 32],
                1,
                [2; 32],
                0,
                NativeNeutralSubjectErrorV1::ZeroLength,
            ),
        ] {
            assert_eq!(
                InertNativeNeutralSubjectV1::new(
                    graph_digest,
                    graph_length,
                    catalog_digest,
                    catalog_length
                ),
                Err(error)
            );
        }
        let subject = InertNativeNeutralSubjectV1::new([1; 32], 1, [2; 32], 1).unwrap();
        for version in [8_u16, 9, 11] {
            let mut bytes = *subject.canonical_bytes();
            bytes[12..14].copy_from_slice(&version.to_le_bytes());
            assert_eq!(
                InertNativeNeutralSubjectV1::decode(&bytes),
                Err(NativeNeutralSubjectErrorV1::Header)
            );
        }
    }
}
