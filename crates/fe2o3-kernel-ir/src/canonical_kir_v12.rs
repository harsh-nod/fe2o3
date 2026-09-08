use std::{error::Error, fmt};

use sha2::{Digest, Sha256};

use crate::{
    KERNEL_IR_MAGIC_V1, KERNEL_IR_VERSION_V12, KernelIrDecodeError, KernelIrEncodeError,
    MAX_MODULE_BYTES_V1, Module, VerificationErrors, decode_module_v12, encode_module_v12,
    verify_module,
};

/// Exact domain bytes for verified canonical Kernel IR V12 policy identities.
pub const VERIFIED_CANONICAL_KERNEL_IR_V12_IDENTITY_DOMAIN_V1: &[u8] =
    b"FE2O3/VERIFIED-CANONICAL-KERNEL-IR/V12\0";
pub const VERIFIED_CANONICAL_KERNEL_IR_V12_IDENTITY_POLICY_V1: u16 = 1;

const VERSION_OFFSET: usize = 8;
const VERSION_END: usize = VERSION_OFFSET + 2;

/// Typed identity minted only for exact canonical Kernel IR V12 bytes accepted
/// by the semantic verifier.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct VerifiedCanonicalKernelIrIdentityV12 {
    digest: [u8; 32],
    canonical_length: u64,
}

impl VerifiedCanonicalKernelIrIdentityV12 {
    pub const fn digest(&self) -> &[u8; 32] {
        &self.digest
    }

    pub const fn canonical_length(&self) -> u64 {
        self.canonical_length
    }
}

/// Move-only owner of one exact V12 encoding whose decoded module passed
/// semantic verification.
#[derive(Debug, Eq, PartialEq)]
pub struct VerifiedCanonicalKernelIrV12 {
    canonical_bytes: Vec<u8>,
    identity: VerifiedCanonicalKernelIrIdentityV12,
}

impl VerifiedCanonicalKernelIrV12 {
    pub fn from_module(module: Module) -> Result<Self, VerifiedCanonicalKernelIrErrorV12> {
        let canonical_bytes =
            encode_module_v12(&module).map_err(VerifiedCanonicalKernelIrErrorV12::Encode)?;
        let decoded = decode_exact_v12(&canonical_bytes)?;
        verify_module(&decoded).map_err(VerifiedCanonicalKernelIrErrorV12::Verification)?;
        if decoded != module {
            return Err(VerifiedCanonicalKernelIrErrorV12::RoundTripMismatch);
        }
        Ok(Self::from_validated_bytes(canonical_bytes))
    }

    pub fn from_canonical_bytes(
        canonical_bytes: Vec<u8>,
    ) -> Result<Self, VerifiedCanonicalKernelIrErrorV12> {
        Self::from_canonical_bytes_with_module(canonical_bytes).map(|(owner, _)| owner)
    }

    pub fn from_canonical_bytes_with_module(
        canonical_bytes: Vec<u8>,
    ) -> Result<(Self, Module), VerifiedCanonicalKernelIrErrorV12> {
        let decoded = decode_exact_v12(&canonical_bytes)?;
        verify_module(&decoded).map_err(VerifiedCanonicalKernelIrErrorV12::Verification)?;
        Ok((Self::from_validated_bytes(canonical_bytes), decoded))
    }

    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }

    pub const fn identity(&self) -> &VerifiedCanonicalKernelIrIdentityV12 {
        &self.identity
    }

    pub fn revalidate(&self) -> Result<(), VerifiedCanonicalKernelIrErrorV12> {
        let decoded = decode_exact_v12(&self.canonical_bytes)?;
        verify_module(&decoded).map_err(VerifiedCanonicalKernelIrErrorV12::Verification)?;
        if canonical_identity(&self.canonical_bytes) != self.identity {
            return Err(VerifiedCanonicalKernelIrErrorV12::IdentityMismatch);
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

fn decode_exact_v12(bytes: &[u8]) -> Result<Module, VerifiedCanonicalKernelIrErrorV12> {
    if bytes.len() > MAX_MODULE_BYTES_V1 {
        return Err(VerifiedCanonicalKernelIrErrorV12::Decode(
            KernelIrDecodeError::TooLarge {
                max: MAX_MODULE_BYTES_V1,
            },
        ));
    }
    let magic =
        bytes
            .get(..KERNEL_IR_MAGIC_V1.len())
            .ok_or(VerifiedCanonicalKernelIrErrorV12::Decode(
                KernelIrDecodeError::Truncated,
            ))?;
    if magic != KERNEL_IR_MAGIC_V1 {
        return Err(VerifiedCanonicalKernelIrErrorV12::Decode(
            KernelIrDecodeError::InvalidMagic,
        ));
    }
    let version_bytes =
        bytes
            .get(VERSION_OFFSET..VERSION_END)
            .ok_or(VerifiedCanonicalKernelIrErrorV12::Decode(
                KernelIrDecodeError::Truncated,
            ))?;
    let version = u16::from_le_bytes([version_bytes[0], version_bytes[1]]);
    if version != KERNEL_IR_VERSION_V12 {
        return Err(VerifiedCanonicalKernelIrErrorV12::NotExactV12 { version });
    }
    decode_module_v12(bytes).map_err(VerifiedCanonicalKernelIrErrorV12::Decode)
}

fn canonical_identity(bytes: &[u8]) -> VerifiedCanonicalKernelIrIdentityV12 {
    let canonical_length =
        u64::try_from(bytes.len()).expect("hard-bounded canonical Kernel IR length fits u64");
    let domain_length = u32::try_from(VERIFIED_CANONICAL_KERNEL_IR_V12_IDENTITY_DOMAIN_V1.len())
        .expect("frozen canonical Kernel IR identity domain length fits u32");
    let mut digest = Sha256::new();
    digest.update(domain_length.to_le_bytes());
    digest.update(VERIFIED_CANONICAL_KERNEL_IR_V12_IDENTITY_DOMAIN_V1);
    digest.update(VERIFIED_CANONICAL_KERNEL_IR_V12_IDENTITY_POLICY_V1.to_le_bytes());
    digest.update(canonical_length.to_le_bytes());
    digest.update(bytes);
    VerifiedCanonicalKernelIrIdentityV12 {
        digest: digest.finalize().into(),
        canonical_length,
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VerifiedCanonicalKernelIrErrorV12 {
    Encode(KernelIrEncodeError),
    Decode(KernelIrDecodeError),
    Verification(VerificationErrors),
    NotExactV12 { version: u16 },
    RoundTripMismatch,
    IdentityMismatch,
}

impl fmt::Display for VerifiedCanonicalKernelIrErrorV12 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Encode(error) => {
                write!(formatter, "cannot encode canonical Kernel IR V12: {error}")
            }
            Self::Decode(error) => {
                write!(formatter, "cannot decode canonical Kernel IR V12: {error}")
            }
            Self::Verification(error) => error.fmt(formatter),
            Self::NotExactV12 { version } => {
                write!(
                    formatter,
                    "expected exact Kernel IR V12 bytes, found V{version}"
                )
            }
            Self::RoundTripMismatch => {
                formatter.write_str("Kernel IR V12 round trip changed bytes or semantics")
            }
            Self::IdentityMismatch => {
                formatter.write_str("canonical Kernel IR V12 identity mismatch")
            }
        }
    }
}

impl Error for VerifiedCanonicalKernelIrErrorV12 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Encode(error) => Some(error),
            Self::Decode(error) => Some(error),
            Self::Verification(error) => Some(error),
            Self::NotExactV12 { .. } | Self::RoundTripMismatch | Self::IdentityMismatch => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_owner_rejects_a_v11_envelope() {
        let bytes = crate::encode_module_v11(&Module::new("canonical-v12-hostile")).unwrap();
        assert!(matches!(
            VerifiedCanonicalKernelIrV12::from_canonical_bytes(bytes),
            Err(VerifiedCanonicalKernelIrErrorV12::NotExactV12 { version: 11 })
        ));
    }

    #[test]
    fn exact_owner_returns_the_same_verified_v12_module() {
        let module = Module::new("canonical-v12-module-custody");
        let bytes = crate::encode_module_v12(&module).unwrap();
        let (owner, decoded) =
            VerifiedCanonicalKernelIrV12::from_canonical_bytes_with_module(bytes.clone()).unwrap();
        assert_eq!(owner.canonical_bytes(), bytes);
        assert_eq!(decoded, module);
    }
}
