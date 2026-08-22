use std::{error::Error, fmt};

use sha2::{Digest, Sha256};

use crate::{
    KERNEL_IR_VERSION_V5, KernelIrDecodeError, KernelIrEncodeError, Module, VerificationErrors,
    decode_module_v5, encode_module_v5, verify_module,
};

/// Policy version for exact, semantically verified Kernel IR V5 ownership.
pub const VERIFIED_CANONICAL_KERNEL_IR_POLICY_V5: u16 = 1;

const IDENTITY_DOMAIN_V5: &[u8] = b"FE2O3/VERIFIED-CANONICAL-KERNEL-IR/V5\0";
const VERSION_OFFSET: usize = 8;

/// Typed identity of exact canonical Kernel IR V5 bytes accepted by the verifier.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct VerifiedCanonicalKernelIrIdentityV5([u8; 32]);

impl VerifiedCanonicalKernelIrIdentityV5 {
    pub const fn digest(&self) -> &[u8; 32] {
        &self.0
    }
}

/// An owned exact V5 encoding whose decoded module passed semantic verification.
///
/// This owner deliberately does not implement `Clone`. It establishes local
/// Kernel IR validation and byte identity, not proof discharge or runtime
/// launch authority.
#[derive(Debug, Eq, PartialEq)]
pub struct VerifiedCanonicalKernelIrV5 {
    canonical_bytes: Vec<u8>,
    identity: VerifiedCanonicalKernelIrIdentityV5,
}

impl VerifiedCanonicalKernelIrV5 {
    /// Verifies a module, canonicalizes it as V5, then decodes and verifies the
    /// exact bytes again before creating the owner.
    pub fn from_module(module: Module) -> Result<Self, VerifiedCanonicalKernelIrErrorV5> {
        verify_module(&module).map_err(VerifiedCanonicalKernelIrErrorV5::Verification)?;
        let canonical_bytes =
            encode_module_v5(&module).map_err(VerifiedCanonicalKernelIrErrorV5::Encode)?;
        let decoded = decode_exact_v5(&canonical_bytes)?;
        verify_module(&decoded).map_err(VerifiedCanonicalKernelIrErrorV5::Verification)?;
        if decoded != module {
            return Err(VerifiedCanonicalKernelIrErrorV5::RoundTripMismatch);
        }
        Ok(Self::from_validated_bytes(canonical_bytes))
    }

    /// Validates caller-provided bytes as exact canonical V5 and reruns the
    /// semantic verifier before creating the owner.
    ///
    /// [`decode_module_v5`] already requires an exact decode/re-encode match,
    /// so this path does not perform a second full encoding.
    pub fn from_canonical_bytes(
        canonical_bytes: Vec<u8>,
    ) -> Result<Self, VerifiedCanonicalKernelIrErrorV5> {
        let decoded = decode_exact_v5(&canonical_bytes)?;
        verify_module(&decoded).map_err(VerifiedCanonicalKernelIrErrorV5::Verification)?;
        Ok(Self::from_validated_bytes(canonical_bytes))
    }

    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }

    pub const fn identity(&self) -> &VerifiedCanonicalKernelIrIdentityV5 {
        &self.identity
    }

    /// Rechecks exact canonical decoding, semantic validity, and the stored
    /// identity. This is suitable for custody-boundary validation.
    pub fn revalidate(&self) -> Result<(), VerifiedCanonicalKernelIrErrorV5> {
        let decoded = decode_exact_v5(&self.canonical_bytes)?;
        verify_module(&decoded).map_err(VerifiedCanonicalKernelIrErrorV5::Verification)?;
        if canonical_identity(&self.canonical_bytes) != self.identity {
            return Err(VerifiedCanonicalKernelIrErrorV5::IdentityMismatch);
        }
        Ok(())
    }

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

fn decode_exact_v5(bytes: &[u8]) -> Result<Module, VerifiedCanonicalKernelIrErrorV5> {
    let decoded = decode_module_v5(bytes).map_err(VerifiedCanonicalKernelIrErrorV5::Decode)?;
    let version_bytes = bytes.get(VERSION_OFFSET..VERSION_OFFSET + 2).ok_or(
        VerifiedCanonicalKernelIrErrorV5::Decode(KernelIrDecodeError::Truncated),
    )?;
    let version = u16::from_le_bytes([version_bytes[0], version_bytes[1]]);
    if version != KERNEL_IR_VERSION_V5 {
        return Err(VerifiedCanonicalKernelIrErrorV5::NotExactV5 { version });
    }
    Ok(decoded)
}

fn canonical_identity(bytes: &[u8]) -> VerifiedCanonicalKernelIrIdentityV5 {
    let mut digest = Sha256::new();
    digest.update((IDENTITY_DOMAIN_V5.len() as u32).to_le_bytes());
    digest.update(IDENTITY_DOMAIN_V5);
    digest.update(VERIFIED_CANONICAL_KERNEL_IR_POLICY_V5.to_le_bytes());
    digest.update((bytes.len() as u64).to_le_bytes());
    digest.update(bytes);
    VerifiedCanonicalKernelIrIdentityV5(digest.finalize().into())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VerifiedCanonicalKernelIrErrorV5 {
    Encode(KernelIrEncodeError),
    Decode(KernelIrDecodeError),
    Verification(VerificationErrors),
    NotExactV5 { version: u16 },
    RoundTripMismatch,
    IdentityMismatch,
}

impl fmt::Display for VerifiedCanonicalKernelIrErrorV5 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Encode(error) => {
                write!(formatter, "cannot encode canonical Kernel IR V5: {error}")
            }
            Self::Decode(error) => {
                write!(formatter, "cannot decode canonical Kernel IR V5: {error}")
            }
            Self::Verification(error) => error.fmt(formatter),
            Self::NotExactV5 { version } => {
                write!(
                    formatter,
                    "expected exact Kernel IR V5 bytes, found V{version}"
                )
            }
            Self::RoundTripMismatch => {
                formatter.write_str("Kernel IR V5 round trip changed bytes or semantics")
            }
            Self::IdentityMismatch => {
                formatter.write_str("canonical Kernel IR V5 identity mismatch")
            }
        }
    }
}

impl Error for VerifiedCanonicalKernelIrErrorV5 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Encode(error) => Some(error),
            Self::Decode(error) => Some(error),
            Self::Verification(error) => Some(error),
            Self::NotExactV5 { .. } | Self::RoundTripMismatch | Self::IdentityMismatch => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        BasicBlock, BlockId, Function, Kernel, LaunchDomain, LaunchExtent, Signature, Terminator,
        encode_module_v4,
    };

    fn fixture(id: &str) -> Module {
        let mut block = BasicBlock::new(BlockId(0));
        block.terminator = Some(Terminator::Return { values: vec![] });
        let function =
            Function::kernel_entry("entry", Signature::new(vec![], vec![]), vec![], vec![block]);
        let mut module = Module::new(id);
        module.functions.push(function);
        module.kernels.push(Kernel::new(
            "kernel",
            "entry",
            LaunchDomain::D1 {
                x: LaunchExtent::Dynamic,
            },
        ));
        module
    }

    #[test]
    fn exact_v5_owner_is_deterministic_and_revalidates() {
        let first = VerifiedCanonicalKernelIrV5::from_module(fixture("module")).unwrap();
        let second = VerifiedCanonicalKernelIrV5::from_module(fixture("module")).unwrap();
        assert_eq!(first.canonical_bytes(), second.canonical_bytes());
        assert_eq!(first.identity(), second.identity());
        first.revalidate().unwrap();

        let recovered =
            VerifiedCanonicalKernelIrV5::from_canonical_bytes(first.canonical_bytes().to_vec())
                .unwrap();
        assert_eq!(recovered.canonical_bytes(), first.canonical_bytes());
        assert_eq!(recovered.identity(), first.identity());
    }

    #[test]
    fn exact_v5_owner_rejects_invalid_modules_and_legacy_bytes() {
        assert!(matches!(
            VerifiedCanonicalKernelIrV5::from_module(Module::new("")),
            Err(VerifiedCanonicalKernelIrErrorV5::Verification(_))
        ));

        let legacy = encode_module_v4(&fixture("legacy")).unwrap();
        assert_eq!(
            VerifiedCanonicalKernelIrV5::from_canonical_bytes(legacy),
            Err(VerifiedCanonicalKernelIrErrorV5::NotExactV5 { version: 4 })
        );
    }

    #[test]
    fn exact_v5_owner_rejects_canonical_but_semantically_invalid_bytes() {
        let invalid_bytes = encode_module_v5(&Module::new("")).unwrap();
        assert!(decode_module_v5(&invalid_bytes).is_ok());
        assert!(matches!(
            VerifiedCanonicalKernelIrV5::from_canonical_bytes(invalid_bytes),
            Err(VerifiedCanonicalKernelIrErrorV5::Verification(_))
        ));
    }

    #[test]
    fn exact_v5_identity_covers_exact_semantics() {
        let baseline = VerifiedCanonicalKernelIrV5::from_module(fixture("first")).unwrap();
        let mutation = VerifiedCanonicalKernelIrV5::from_module(fixture("second")).unwrap();
        assert_ne!(baseline.canonical_bytes(), mutation.canonical_bytes());
        assert_ne!(baseline.identity(), mutation.identity());
    }

    #[test]
    fn exact_v5_verified_policy_identity_is_frozen() {
        let owner = VerifiedCanonicalKernelIrV5::from_module(fixture("module")).unwrap();
        assert_eq!(
            owner.identity().digest(),
            &[
                0x7f, 0x1e, 0xeb, 0x61, 0x2c, 0xa5, 0xed, 0x2b, 0x92, 0x5e, 0x38, 0xd5, 0x45, 0x53,
                0xc8, 0x7e, 0x6c, 0x1c, 0x45, 0x00, 0x53, 0xf3, 0x6d, 0x6d, 0xf8, 0x33, 0xd8, 0xce,
                0x54, 0x23, 0x7e, 0x73,
            ]
        );
    }

    #[test]
    fn exact_v5_bytes_reject_truncation_trailing_and_unknown_versions() {
        let owner = VerifiedCanonicalKernelIrV5::from_module(fixture("module")).unwrap();
        let bytes = owner.canonical_bytes();
        for end in 0..bytes.len() {
            assert!(
                VerifiedCanonicalKernelIrV5::from_canonical_bytes(bytes[..end].to_vec()).is_err(),
                "prefix {end}"
            );
        }

        let mut trailing = bytes.to_vec();
        trailing.push(0);
        assert!(matches!(
            VerifiedCanonicalKernelIrV5::from_canonical_bytes(trailing),
            Err(VerifiedCanonicalKernelIrErrorV5::Decode(
                KernelIrDecodeError::TrailingBytes
            ))
        ));

        let mut unknown = bytes.to_vec();
        unknown[VERSION_OFFSET..VERSION_OFFSET + 2].copy_from_slice(&6_u16.to_le_bytes());
        assert!(matches!(
            VerifiedCanonicalKernelIrV5::from_canonical_bytes(unknown),
            Err(VerifiedCanonicalKernelIrErrorV5::Decode(
                KernelIrDecodeError::UnknownVersion(6)
            ))
        ));
    }
}
