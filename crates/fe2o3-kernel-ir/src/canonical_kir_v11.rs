use std::{error::Error, fmt};

use sha2::{Digest, Sha256};

use crate::{
    KERNEL_IR_MAGIC_V1, KERNEL_IR_VERSION_V11, KernelIrDecodeError, KernelIrEncodeError,
    MAX_MODULE_BYTES_V1, Module, VerificationErrors, decode_module_v11, encode_module_v11,
    verify_module,
};

/// Exact domain bytes for verified canonical Kernel IR V11 policy identities.
pub const VERIFIED_CANONICAL_KERNEL_IR_V11_IDENTITY_DOMAIN_V1: &[u8] =
    b"FE2O3/VERIFIED-CANONICAL-KERNEL-IR/V11\0";
pub const VERIFIED_CANONICAL_KERNEL_IR_V11_IDENTITY_POLICY_V1: u16 = 1;

const VERSION_OFFSET: usize = 8;
const VERSION_END: usize = VERSION_OFFSET + 2;

/// Typed identity minted only for exact canonical Kernel IR V11 bytes accepted
/// by the semantic verifier.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct VerifiedCanonicalKernelIrIdentityV11 {
    digest: [u8; 32],
    canonical_length: u64,
}

impl VerifiedCanonicalKernelIrIdentityV11 {
    pub const fn digest(&self) -> &[u8; 32] {
        &self.digest
    }

    pub const fn canonical_length(&self) -> u64 {
        self.canonical_length
    }
}

/// Move-only owner of one exact V11 encoding whose decoded module passed
/// semantic verification.
#[derive(Debug, Eq, PartialEq)]
pub struct VerifiedCanonicalKernelIrV11 {
    canonical_bytes: Vec<u8>,
    identity: VerifiedCanonicalKernelIrIdentityV11,
}

impl VerifiedCanonicalKernelIrV11 {
    pub fn from_module(module: Module) -> Result<Self, VerifiedCanonicalKernelIrErrorV11> {
        let canonical_bytes =
            encode_module_v11(&module).map_err(VerifiedCanonicalKernelIrErrorV11::Encode)?;
        let decoded = decode_exact_v11(&canonical_bytes)?;
        verify_module(&decoded).map_err(VerifiedCanonicalKernelIrErrorV11::Verification)?;
        if decoded != module {
            return Err(VerifiedCanonicalKernelIrErrorV11::RoundTripMismatch);
        }
        Ok(Self::from_validated_bytes(canonical_bytes))
    }

    pub fn from_canonical_bytes(
        canonical_bytes: Vec<u8>,
    ) -> Result<Self, VerifiedCanonicalKernelIrErrorV11> {
        Self::from_canonical_bytes_with_module(canonical_bytes).map(|(owner, _)| owner)
    }

    pub fn from_canonical_bytes_with_module(
        canonical_bytes: Vec<u8>,
    ) -> Result<(Self, Module), VerifiedCanonicalKernelIrErrorV11> {
        let decoded = decode_exact_v11(&canonical_bytes)?;
        verify_module(&decoded).map_err(VerifiedCanonicalKernelIrErrorV11::Verification)?;
        Ok((Self::from_validated_bytes(canonical_bytes), decoded))
    }

    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }

    pub const fn identity(&self) -> &VerifiedCanonicalKernelIrIdentityV11 {
        &self.identity
    }

    pub fn revalidate(&self) -> Result<(), VerifiedCanonicalKernelIrErrorV11> {
        let decoded = decode_exact_v11(&self.canonical_bytes)?;
        verify_module(&decoded).map_err(VerifiedCanonicalKernelIrErrorV11::Verification)?;
        if canonical_identity(&self.canonical_bytes) != self.identity {
            return Err(VerifiedCanonicalKernelIrErrorV11::IdentityMismatch);
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

fn decode_exact_v11(bytes: &[u8]) -> Result<Module, VerifiedCanonicalKernelIrErrorV11> {
    if bytes.len() > MAX_MODULE_BYTES_V1 {
        return Err(VerifiedCanonicalKernelIrErrorV11::Decode(
            KernelIrDecodeError::TooLarge {
                max: MAX_MODULE_BYTES_V1,
            },
        ));
    }
    let magic =
        bytes
            .get(..KERNEL_IR_MAGIC_V1.len())
            .ok_or(VerifiedCanonicalKernelIrErrorV11::Decode(
                KernelIrDecodeError::Truncated,
            ))?;
    if magic != KERNEL_IR_MAGIC_V1 {
        return Err(VerifiedCanonicalKernelIrErrorV11::Decode(
            KernelIrDecodeError::InvalidMagic,
        ));
    }
    let version_bytes =
        bytes
            .get(VERSION_OFFSET..VERSION_END)
            .ok_or(VerifiedCanonicalKernelIrErrorV11::Decode(
                KernelIrDecodeError::Truncated,
            ))?;
    let version = u16::from_le_bytes([version_bytes[0], version_bytes[1]]);
    if version != KERNEL_IR_VERSION_V11 {
        return Err(VerifiedCanonicalKernelIrErrorV11::NotExactV11 { version });
    }
    decode_module_v11(bytes).map_err(VerifiedCanonicalKernelIrErrorV11::Decode)
}

fn canonical_identity(bytes: &[u8]) -> VerifiedCanonicalKernelIrIdentityV11 {
    let canonical_length =
        u64::try_from(bytes.len()).expect("hard-bounded canonical Kernel IR length fits u64");
    let domain_length = u32::try_from(VERIFIED_CANONICAL_KERNEL_IR_V11_IDENTITY_DOMAIN_V1.len())
        .expect("frozen canonical Kernel IR identity domain length fits u32");
    let mut digest = Sha256::new();
    digest.update(domain_length.to_le_bytes());
    digest.update(VERIFIED_CANONICAL_KERNEL_IR_V11_IDENTITY_DOMAIN_V1);
    digest.update(VERIFIED_CANONICAL_KERNEL_IR_V11_IDENTITY_POLICY_V1.to_le_bytes());
    digest.update(canonical_length.to_le_bytes());
    digest.update(bytes);
    VerifiedCanonicalKernelIrIdentityV11 {
        digest: digest.finalize().into(),
        canonical_length,
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VerifiedCanonicalKernelIrErrorV11 {
    Encode(KernelIrEncodeError),
    Decode(KernelIrDecodeError),
    Verification(VerificationErrors),
    NotExactV11 { version: u16 },
    RoundTripMismatch,
    IdentityMismatch,
}

impl fmt::Display for VerifiedCanonicalKernelIrErrorV11 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Encode(error) => {
                write!(formatter, "cannot encode canonical Kernel IR V11: {error}")
            }
            Self::Decode(error) => {
                write!(formatter, "cannot decode canonical Kernel IR V11: {error}")
            }
            Self::Verification(error) => error.fmt(formatter),
            Self::NotExactV11 { version } => {
                write!(
                    formatter,
                    "expected exact Kernel IR V11 bytes, found V{version}"
                )
            }
            Self::RoundTripMismatch => {
                formatter.write_str("Kernel IR V11 round trip changed bytes or semantics")
            }
            Self::IdentityMismatch => {
                formatter.write_str("canonical Kernel IR V11 identity mismatch")
            }
        }
    }
}

impl Error for VerifiedCanonicalKernelIrErrorV11 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Encode(error) => Some(error),
            Self::Decode(error) => Some(error),
            Self::Verification(error) => Some(error),
            Self::NotExactV11 { .. } | Self::RoundTripMismatch | Self::IdentityMismatch => None,
        }
    }
}
