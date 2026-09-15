//! One move-only canonical production owner, with an explicit declared version.
//! Verification establishes KIR consistency, not source or hardware authority.
use std::{error::Error, fmt};
use sha2::{Digest, Sha256};
use crate::{Module, KernelIrDecodeError, KernelIrEncodeError, VerificationErrors,
    KERNEL_IR_MAGIC_V1, MAX_MODULE_BYTES_V1, decode_module_v13, decode_module_v14,
    encode_module_v13, encode_module_v14, verify_module};

pub const VERIFIED_CANONICAL_KERNEL_IR_V14_IDENTITY_DOMAIN_V1: &[u8] = b"FE2O3/VERIFIED-CANONICAL-KERNEL-IR/V14\0";

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CanonicalKernelIrVersionV1 { V13, V14 }
impl CanonicalKernelIrVersionV1 {
    pub const fn wire_version(self) -> u16 { match self { Self::V13 => 13, Self::V14 => 14 } }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct VerifiedCanonicalKernelIrIdentityV1 {
    version: CanonicalKernelIrVersionV1,
    digest: [u8; 32],
    canonical_length: u64,
}
impl VerifiedCanonicalKernelIrIdentityV1 {
    pub const fn version(&self) -> CanonicalKernelIrVersionV1 { self.version }
    pub const fn digest(&self) -> &[u8; 32] { &self.digest }
    pub const fn canonical_length(&self) -> u64 { self.canonical_length }
}

#[derive(Debug, Eq, PartialEq)]
pub struct VerifiedCanonicalKernelIrV1 {
    canonical_bytes: Vec<u8>,
    identity: VerifiedCanonicalKernelIrIdentityV1,
}
impl VerifiedCanonicalKernelIrV1 {
    pub fn from_module(module: Module, version: CanonicalKernelIrVersionV1) -> Result<Self, VerifiedCanonicalKernelIrErrorV1> {
        let bytes = encode(&module, version).map_err(VerifiedCanonicalKernelIrErrorV1::Encode)?;
        let decoded = decode_exact(&bytes, version)?;
        verify_module(&decoded).map_err(VerifiedCanonicalKernelIrErrorV1::Verification)?;
        if decoded != module { return Err(VerifiedCanonicalKernelIrErrorV1::RoundTripMismatch); }
        Ok(Self::validated(bytes, version))
    }
    pub fn from_canonical_bytes(bytes: Vec<u8>, version: CanonicalKernelIrVersionV1) -> Result<Self, VerifiedCanonicalKernelIrErrorV1> {
        Self::from_canonical_bytes_with_module(bytes, version).map(|(owner, _)| owner)
    }
    pub fn from_canonical_bytes_with_module(bytes: Vec<u8>, version: CanonicalKernelIrVersionV1)
        -> Result<(Self, Module), VerifiedCanonicalKernelIrErrorV1> {
        let module = decode_exact(&bytes, version)?;
        verify_module(&module).map_err(VerifiedCanonicalKernelIrErrorV1::Verification)?;
        Ok((Self::validated(bytes, version), module))
    }
    pub const fn version(&self) -> CanonicalKernelIrVersionV1 { self.identity.version }
    pub fn canonical_bytes(&self) -> &[u8] { &self.canonical_bytes }
    pub const fn identity(&self) -> &VerifiedCanonicalKernelIrIdentityV1 { &self.identity }
    pub fn into_canonical_bytes(self) -> Vec<u8> { self.canonical_bytes }
    pub fn revalidate(&self) -> Result<(), VerifiedCanonicalKernelIrErrorV1> {
        let decoded = decode_exact(&self.canonical_bytes, self.version())?;
        verify_module(&decoded).map_err(VerifiedCanonicalKernelIrErrorV1::Verification)?;
        if identity(&self.canonical_bytes, self.version()) != self.identity {
            return Err(VerifiedCanonicalKernelIrErrorV1::IdentityMismatch);
        }
        Ok(())
    }
    fn validated(bytes: Vec<u8>, version: CanonicalKernelIrVersionV1) -> Self {
        let identity = identity(&bytes, version);
        Self { canonical_bytes: bytes, identity }
    }
}

fn encode(module: &Module, version: CanonicalKernelIrVersionV1) -> Result<Vec<u8>, KernelIrEncodeError> {
    match version { CanonicalKernelIrVersionV1::V13 => encode_module_v13(module), CanonicalKernelIrVersionV1::V14 => encode_module_v14(module) }
}
fn decode_exact(bytes: &[u8], expected: CanonicalKernelIrVersionV1) -> Result<Module, VerifiedCanonicalKernelIrErrorV1> {
    use VerifiedCanonicalKernelIrErrorV1 as E;
    if bytes.len() > MAX_MODULE_BYTES_V1 { return Err(E::Decode(KernelIrDecodeError::TooLarge { max: MAX_MODULE_BYTES_V1 })); }
    let magic = bytes.get(..8).ok_or(E::Decode(KernelIrDecodeError::Truncated))?;
    if magic != KERNEL_IR_MAGIC_V1 { return Err(E::Decode(KernelIrDecodeError::InvalidMagic)); }
    let tag = bytes.get(8..10).ok_or(E::Decode(KernelIrDecodeError::Truncated))?;
    let actual = u16::from_le_bytes([tag[0], tag[1]]);
    if actual != expected.wire_version() { return Err(E::NotDeclaredVersion { expected, actual }); }
    match expected {
        CanonicalKernelIrVersionV1::V13 => decode_module_v13(bytes),
        CanonicalKernelIrVersionV1::V14 => decode_module_v14(bytes),
    }.map_err(E::Decode)
}
fn identity(bytes: &[u8], version: CanonicalKernelIrVersionV1) -> VerifiedCanonicalKernelIrIdentityV1 {
    let domain = match version {
        CanonicalKernelIrVersionV1::V13 => crate::VERIFIED_CANONICAL_KERNEL_IR_V13_IDENTITY_DOMAIN_V1,
        CanonicalKernelIrVersionV1::V14 => VERIFIED_CANONICAL_KERNEL_IR_V14_IDENTITY_DOMAIN_V1,
    };
    let length = u64::try_from(bytes.len()).expect("bounded canonical KIR length fits u64");
    let mut digest = Sha256::new();
    digest.update(u32::try_from(domain.len()).expect("fixed identity domain fits u32").to_le_bytes());
    digest.update(domain);
    digest.update(crate::VERIFIED_CANONICAL_KERNEL_IR_V13_IDENTITY_POLICY_V1.to_le_bytes());
    digest.update(length.to_le_bytes());
    digest.update(bytes);
    VerifiedCanonicalKernelIrIdentityV1 { version, digest: digest.finalize().into(), canonical_length: length }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VerifiedCanonicalKernelIrErrorV1 {
    Encode(KernelIrEncodeError), Decode(KernelIrDecodeError), Verification(VerificationErrors),
    NotDeclaredVersion { expected: CanonicalKernelIrVersionV1, actual: u16 },
    RoundTripMismatch, IdentityMismatch,
}
impl fmt::Display for VerifiedCanonicalKernelIrErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Encode(e) => write!(f, "cannot encode canonical KIR: {e}"),
            Self::Decode(e) => write!(f, "cannot decode canonical KIR: {e}"),
            Self::Verification(e) => e.fmt(f),
            Self::NotDeclaredVersion { expected, actual } => write!(f, "expected KIR V{}, found V{actual}", expected.wire_version()),
            Self::RoundTripMismatch => f.write_str("canonical KIR round trip changed semantics"),
            Self::IdentityMismatch => f.write_str("canonical KIR identity mismatch"),
        }
    }
}
impl Error for VerifiedCanonicalKernelIrErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self { Self::Encode(e) => Some(e), Self::Decode(e) => Some(e), Self::Verification(e) => Some(e), _ => None }
    }
}
