use std::{error::Error, fmt};

use sha2::{Digest, Sha256};

use crate::{
    KERNEL_IR_MAGIC_V1, KERNEL_IR_VERSION_V7, KernelIrDecodeError, KernelIrEncodeError,
    MAX_MODULE_BYTES_V1, Module, VerificationErrors, decode_module_v7, encode_module_v7,
    verify_module,
};

/// Exact domain bytes for verified canonical Kernel IR V7 policy identities.
pub const VERIFIED_CANONICAL_KERNEL_IR_V7_IDENTITY_DOMAIN_V1: &[u8] =
    b"FE2O3/VERIFIED-CANONICAL-KERNEL-IR/V7\0";
/// Frozen policy version for exact, semantically verified Kernel IR V7 ownership.
pub const VERIFIED_CANONICAL_KERNEL_IR_V7_IDENTITY_POLICY_V1: u16 = 1;

const VERSION_OFFSET: usize = 8;
const VERSION_END: usize = VERSION_OFFSET + 2;

/// Typed identity minted only for exact canonical Kernel IR V7 bytes accepted
/// by the semantic verifier.
///
/// The length is retained alongside the digest to make custody comparisons
/// explicit. It is also framed inside the SHA-256 preimage, so neither field
/// can be substituted independently.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct VerifiedCanonicalKernelIrIdentityV7 {
    digest: [u8; 32],
    canonical_length: u64,
}

impl VerifiedCanonicalKernelIrIdentityV7 {
    /// Returns the exact SHA-256 digest bytes.
    pub const fn digest(&self) -> &[u8; 32] {
        &self.digest
    }

    /// Returns the exact canonical byte length committed by the digest.
    pub const fn canonical_length(&self) -> u64 {
        self.canonical_length
    }
}

/// An owned exact V7 encoding whose decoded module passed semantic verification.
///
/// This is the production-authoritative owner for canonical Kernel IR V7. It
/// establishes typed wire canonicality, local Kernel IR semantic validity, and
/// exact policy identity. It does not establish source-to-KIR refinement,
/// proof discharge, artifact publication, executable, or launch authority.
///
/// The owner deliberately does not implement `Clone`; transferring the value
/// transfers custody. Its identity is inert and may be copied.
///
/// ```compile_fail
/// use fe2o3_kernel_ir::VerifiedCanonicalKernelIrV7;
///
/// fn duplicate(owner: VerifiedCanonicalKernelIrV7) {
///     let _duplicate = owner.clone();
/// }
/// ```
#[derive(Debug, Eq, PartialEq)]
pub struct VerifiedCanonicalKernelIrV7 {
    canonical_bytes: Vec<u8>,
    identity: VerifiedCanonicalKernelIrIdentityV7,
}

impl VerifiedCanonicalKernelIrV7 {
    /// Bounds and canonically encodes a module as exact V7, then typed-decodes,
    /// semantically verifies, and compares the round trip before creating the
    /// owner.
    ///
    /// Encoding runs before semantic verification so all existing hard wire
    /// and module-count bounds apply before the verifier builds indexes for a
    /// caller-provided in-memory graph.
    pub fn from_module(module: Module) -> Result<Self, VerifiedCanonicalKernelIrErrorV7> {
        let canonical_bytes =
            encode_module_v7(&module).map_err(VerifiedCanonicalKernelIrErrorV7::Encode)?;
        let decoded = decode_exact_v7(&canonical_bytes)?;
        verify_module(&decoded).map_err(VerifiedCanonicalKernelIrErrorV7::Verification)?;
        if decoded != module {
            return Err(VerifiedCanonicalKernelIrErrorV7::RoundTripMismatch);
        }
        Ok(Self::from_validated_bytes(canonical_bytes))
    }

    /// Takes ownership of caller-provided bytes and admits them only after an
    /// exact V7 decode/re-encode match and semantic verification.
    ///
    /// The supplied allocation becomes the retained canonical allocation; the
    /// owner does not clone it. [`decode_module_v7`] performs the bounded,
    /// byte-for-byte canonical re-encoding check.
    pub fn from_canonical_bytes(
        canonical_bytes: Vec<u8>,
    ) -> Result<Self, VerifiedCanonicalKernelIrErrorV7> {
        Self::from_canonical_bytes_with_module(canonical_bytes).map(|(owner, _)| owner)
    }

    /// Takes ownership of exact canonical V7 bytes and returns both their owner and the same
    /// semantically verified decoded module.
    ///
    /// This avoids a second full decode for consumers that inspect the verified module while
    /// retaining custody of its exact canonical bytes.
    pub fn from_canonical_bytes_with_module(
        canonical_bytes: Vec<u8>,
    ) -> Result<(Self, Module), VerifiedCanonicalKernelIrErrorV7> {
        let decoded = decode_exact_v7(&canonical_bytes)?;
        verify_module(&decoded).map_err(VerifiedCanonicalKernelIrErrorV7::Verification)?;
        Ok((Self::from_validated_bytes(canonical_bytes), decoded))
    }

    /// Borrows the complete exact canonical V7 bytes.
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }

    /// Borrows the frozen policy V1 identity for the retained bytes.
    pub const fn identity(&self) -> &VerifiedCanonicalKernelIrIdentityV7 {
        &self.identity
    }

    /// Rechecks exact V7 canonical decoding, semantic validity, and identity.
    /// This is suitable for validating a retained owner at a custody boundary.
    pub fn revalidate(&self) -> Result<(), VerifiedCanonicalKernelIrErrorV7> {
        let decoded = decode_exact_v7(&self.canonical_bytes)?;
        verify_module(&decoded).map_err(VerifiedCanonicalKernelIrErrorV7::Verification)?;
        if canonical_identity(&self.canonical_bytes) != self.identity {
            return Err(VerifiedCanonicalKernelIrErrorV7::IdentityMismatch);
        }
        Ok(())
    }

    /// Consumes the authority owner and returns its exact canonical V7 bytes.
    pub fn into_canonical_bytes(self) -> Vec<u8> {
        self.canonical_bytes
    }

    fn from_validated_bytes(canonical_bytes: Vec<u8>) -> Self {
        let identity = canonical_identity(&canonical_bytes);
        Self {
            canonical_bytes,
            identity,
        }
    }
}

fn decode_exact_v7(bytes: &[u8]) -> Result<Module, VerifiedCanonicalKernelIrErrorV7> {
    // Reject oversized and non-V7 envelopes before allocating a decoded graph.
    if bytes.len() > MAX_MODULE_BYTES_V1 {
        return Err(VerifiedCanonicalKernelIrErrorV7::Decode(
            KernelIrDecodeError::TooLarge {
                max: MAX_MODULE_BYTES_V1,
            },
        ));
    }
    let magic =
        bytes
            .get(..KERNEL_IR_MAGIC_V1.len())
            .ok_or(VerifiedCanonicalKernelIrErrorV7::Decode(
                KernelIrDecodeError::Truncated,
            ))?;
    if magic != KERNEL_IR_MAGIC_V1 {
        return Err(VerifiedCanonicalKernelIrErrorV7::Decode(
            KernelIrDecodeError::InvalidMagic,
        ));
    }
    let version_bytes =
        bytes
            .get(VERSION_OFFSET..VERSION_END)
            .ok_or(VerifiedCanonicalKernelIrErrorV7::Decode(
                KernelIrDecodeError::Truncated,
            ))?;
    let version = u16::from_le_bytes([version_bytes[0], version_bytes[1]]);
    if version != KERNEL_IR_VERSION_V7 {
        return Err(VerifiedCanonicalKernelIrErrorV7::NotExactV7 { version });
    }

    decode_module_v7(bytes).map_err(VerifiedCanonicalKernelIrErrorV7::Decode)
}

fn canonical_identity(bytes: &[u8]) -> VerifiedCanonicalKernelIrIdentityV7 {
    let canonical_length =
        u64::try_from(bytes.len()).expect("hard-bounded canonical Kernel IR length fits u64");
    let domain_length = u32::try_from(VERIFIED_CANONICAL_KERNEL_IR_V7_IDENTITY_DOMAIN_V1.len())
        .expect("frozen canonical Kernel IR identity domain length fits u32");
    let mut digest = Sha256::new();
    digest.update(domain_length.to_le_bytes());
    digest.update(VERIFIED_CANONICAL_KERNEL_IR_V7_IDENTITY_DOMAIN_V1);
    digest.update(VERIFIED_CANONICAL_KERNEL_IR_V7_IDENTITY_POLICY_V1.to_le_bytes());
    digest.update(canonical_length.to_le_bytes());
    digest.update(bytes);
    VerifiedCanonicalKernelIrIdentityV7 {
        digest: digest.finalize().into(),
        canonical_length,
    }
}

/// Failure to establish or revalidate authoritative canonical Kernel IR V7
/// ownership.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VerifiedCanonicalKernelIrErrorV7 {
    /// The in-memory module could not be represented within exact V7 bounds.
    Encode(KernelIrEncodeError),
    /// The supplied bytes were not a bounded canonical Kernel IR encoding.
    Decode(KernelIrDecodeError),
    /// The decoded module failed structural or local semantic verification.
    Verification(VerificationErrors),
    /// The artifact used a recognized or claimed wire version other than V7.
    NotExactV7 {
        /// Version read from the fixed Kernel IR header.
        version: u16,
    },
    /// Encoding and decoding did not retain the exact in-memory module.
    RoundTripMismatch,
    /// Retained bytes no longer match the identity minted at admission.
    IdentityMismatch,
}

impl fmt::Display for VerifiedCanonicalKernelIrErrorV7 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Encode(error) => {
                write!(formatter, "cannot encode canonical Kernel IR V7: {error}")
            }
            Self::Decode(error) => {
                write!(formatter, "cannot decode canonical Kernel IR V7: {error}")
            }
            Self::Verification(error) => error.fmt(formatter),
            Self::NotExactV7 { version } => {
                write!(
                    formatter,
                    "expected exact Kernel IR V7 bytes, found V{version}"
                )
            }
            Self::RoundTripMismatch => {
                formatter.write_str("Kernel IR V7 round trip changed bytes or semantics")
            }
            Self::IdentityMismatch => {
                formatter.write_str("canonical Kernel IR V7 identity mismatch")
            }
        }
    }
}

impl Error for VerifiedCanonicalKernelIrErrorV7 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Encode(error) => Some(error),
            Self::Decode(error) => Some(error),
            Self::Verification(error) => Some(error),
            Self::NotExactV7 { .. } | Self::RoundTripMismatch | Self::IdentityMismatch => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn from_hex(text: &str) -> Vec<u8> {
        let compact = text
            .bytes()
            .filter(|byte| !byte.is_ascii_whitespace())
            .collect::<Vec<_>>();
        assert_eq!(compact.len() % 2, 0);
        compact
            .chunks_exact(2)
            .map(|pair| {
                let digit = |value: u8| match value {
                    b'0'..=b'9' => value - b'0',
                    b'a'..=b'f' => value - b'a' + 10,
                    _ => panic!("invalid frozen hex"),
                };
                (digit(pair[0]) << 4) | digit(pair[1])
            })
            .collect()
    }

    #[test]
    fn identity_scheme_matches_the_frozen_lineage_v4_vector() {
        let mut synthetic_envelope = vec![0xa5; 32];
        synthetic_envelope[..8].copy_from_slice(&KERNEL_IR_MAGIC_V1);
        synthetic_envelope[8..10].copy_from_slice(&KERNEL_IR_VERSION_V7.to_le_bytes());
        synthetic_envelope[10..12].copy_from_slice(&0_u16.to_le_bytes());
        synthetic_envelope[12..16].copy_from_slice(&32_u32.to_le_bytes());
        synthetic_envelope[16..20].copy_from_slice(&0_u32.to_le_bytes());

        let identity = canonical_identity(&synthetic_envelope);
        assert_eq!(VERIFIED_CANONICAL_KERNEL_IR_V7_IDENTITY_DOMAIN_V1.len(), 38);
        assert_eq!(identity.canonical_length(), 32);
        assert_eq!(
            identity.digest(),
            &[
                0x78, 0xcc, 0x21, 0xf8, 0x15, 0xa0, 0x7d, 0x71, 0x3c, 0xed, 0xeb, 0x6f, 0x37, 0xb2,
                0xbb, 0xc4, 0x24, 0xb9, 0x4e, 0x37, 0xda, 0x95, 0xba, 0x98, 0x48, 0xe6, 0x17, 0x6b,
                0xe6, 0x26, 0x5b, 0x9a,
            ]
        );
    }

    #[test]
    fn revalidation_detects_retained_byte_mutation_before_reusing_identity() {
        let v6 = from_hex(include_str!("../tests/fixtures/checked_add_i128_v6.hex"));
        let module = crate::decode_module_v6(&v6).unwrap();
        let bytes = encode_module_v7(&module).unwrap();
        let mut owner = VerifiedCanonicalKernelIrV7::from_canonical_bytes(bytes).unwrap();
        let module_id_offset = 24;
        assert_eq!(owner.canonical_bytes[module_id_offset], b'c');
        owner.canonical_bytes[module_id_offset] = b'C';
        assert_eq!(
            owner.revalidate(),
            Err(VerifiedCanonicalKernelIrErrorV7::IdentityMismatch)
        );
    }

    #[test]
    fn exact_decode_returns_one_owner_and_the_same_verified_module() {
        let v6 = from_hex(include_str!("../tests/fixtures/checked_add_i128_v6.hex"));
        let expected = crate::decode_module_v6(&v6).unwrap();
        let bytes = encode_module_v7(&expected).unwrap();
        let (owner, decoded) =
            VerifiedCanonicalKernelIrV7::from_canonical_bytes_with_module(bytes.clone()).unwrap();

        assert_eq!(decoded, expected);
        assert_eq!(owner.canonical_bytes(), bytes);
        owner.revalidate().unwrap();
    }

    #[test]
    fn exact_decode_rejects_truncated_bytes() {
        let v6 = from_hex(include_str!("../tests/fixtures/checked_add_i128_v6.hex"));
        let module = crate::decode_module_v6(&v6).unwrap();
        let mut bytes = encode_module_v7(&module).unwrap();
        bytes.truncate(VERSION_END - 1);

        assert_eq!(
            VerifiedCanonicalKernelIrV7::from_canonical_bytes_with_module(bytes).unwrap_err(),
            VerifiedCanonicalKernelIrErrorV7::Decode(KernelIrDecodeError::Truncated)
        );
    }

    #[test]
    fn exact_decode_rejects_bytes_above_the_hard_bound() {
        let bytes = vec![0_u8; MAX_MODULE_BYTES_V1 + 1];

        assert_eq!(
            VerifiedCanonicalKernelIrV7::from_canonical_bytes_with_module(bytes).unwrap_err(),
            VerifiedCanonicalKernelIrErrorV7::Decode(KernelIrDecodeError::TooLarge {
                max: MAX_MODULE_BYTES_V1,
            })
        );
    }
}
