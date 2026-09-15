use std::{error::Error, fmt};

use crate::{
    KernelIrDecodeError, KernelIrEncodeError, Module, VerificationErrors,
    CanonicalKernelIrVersionV1, VerifiedCanonicalKernelIrV1, VerifiedCanonicalKernelIrErrorV1,
};

pub const VERIFIED_CANONICAL_KERNEL_IR_V13_IDENTITY_DOMAIN_V1: &[u8] =
    b"FE2O3/VERIFIED-CANONICAL-KERNEL-IR/V13\0";
pub const VERIFIED_CANONICAL_KERNEL_IR_V13_IDENTITY_POLICY_V1: u16 = 1;

/// Stable identity of exact verified canonical KIR V13 bytes.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct VerifiedCanonicalKernelIrIdentityV13 {
    digest: [u8; 32],
    canonical_length: u64,
}

impl VerifiedCanonicalKernelIrIdentityV13 {
    pub const fn digest(&self) -> &[u8; 32] {
        &self.digest
    }

    pub const fn canonical_length(&self) -> u64 {
        self.canonical_length
    }
}

/// Move-only owner of one exact, semantically verified KIR V13 graph.
#[derive(Debug, Eq, PartialEq)]
pub struct VerifiedCanonicalKernelIrV13 {
    owner: VerifiedCanonicalKernelIrV1,
    identity: VerifiedCanonicalKernelIrIdentityV13,
}

impl VerifiedCanonicalKernelIrV13 {
    /// Borrows the same strictly V13 owner without copying bytes or reissuing custody.
    pub const fn as_common(&self) -> &VerifiedCanonicalKernelIrV1 {
        &self.owner
    }

    /// Moves the same declared-version owner; V14 cannot enter this facade.
    pub fn into_common(self) -> VerifiedCanonicalKernelIrV1 {
        self.owner
    }

    pub fn from_module(module: Module) -> Result<Self, VerifiedCanonicalKernelIrErrorV13> {
        let owner = VerifiedCanonicalKernelIrV1::from_module(module, CanonicalKernelIrVersionV1::V13).map_err(legacy_error)?;
        Ok(Self::from_owner(owner))
    }

    pub fn from_canonical_bytes(
        canonical_bytes: Vec<u8>,
    ) -> Result<Self, VerifiedCanonicalKernelIrErrorV13> {
        Self::from_canonical_bytes_with_module(canonical_bytes).map(|(owner, _)| owner)
    }

    pub fn from_canonical_bytes_with_module(
        canonical_bytes: Vec<u8>,
    ) -> Result<(Self, Module), VerifiedCanonicalKernelIrErrorV13> {
        let (owner, module) = VerifiedCanonicalKernelIrV1::from_canonical_bytes_with_module(canonical_bytes, CanonicalKernelIrVersionV1::V13).map_err(legacy_error)?;
        Ok((Self::from_owner(owner), module))
    }

    pub fn canonical_bytes(&self) -> &[u8] {
        self.owner.canonical_bytes()
    }

    pub const fn identity(&self) -> &VerifiedCanonicalKernelIrIdentityV13 {
        &self.identity
    }

    pub fn revalidate(&self) -> Result<(), VerifiedCanonicalKernelIrErrorV13> {
        self.owner.revalidate().map_err(legacy_error)?;
        if self.owner.identity().digest() != self.identity.digest()
            || self.owner.identity().canonical_length() != self.identity.canonical_length() {
            return Err(VerifiedCanonicalKernelIrErrorV13::IdentityMismatch);
        }
        Ok(())
    }

    pub fn into_canonical_bytes(self) -> Vec<u8> {
        self.owner.into_canonical_bytes()
    }

    fn from_owner(owner: VerifiedCanonicalKernelIrV1) -> Self {
        let identity = VerifiedCanonicalKernelIrIdentityV13 { digest: *owner.identity().digest(), canonical_length: owner.identity().canonical_length() };
        Self {
            owner,
            identity,
        }
    }
}

fn legacy_error(error: VerifiedCanonicalKernelIrErrorV1) -> VerifiedCanonicalKernelIrErrorV13 {
    use VerifiedCanonicalKernelIrErrorV1 as E;
    match error {
        E::Encode(e) => VerifiedCanonicalKernelIrErrorV13::Encode(e),
        E::Decode(e) => VerifiedCanonicalKernelIrErrorV13::Decode(e),
        E::Verification(e) => VerifiedCanonicalKernelIrErrorV13::Verification(e),
        E::NotDeclaredVersion { actual, .. } => VerifiedCanonicalKernelIrErrorV13::NotExactV13 { version: actual },
        E::RoundTripMismatch => VerifiedCanonicalKernelIrErrorV13::RoundTripMismatch,
        E::IdentityMismatch => VerifiedCanonicalKernelIrErrorV13::IdentityMismatch,
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VerifiedCanonicalKernelIrErrorV13 {
    Encode(KernelIrEncodeError),
    Decode(KernelIrDecodeError),
    Verification(VerificationErrors),
    NotExactV13 { version: u16 },
    RoundTripMismatch,
    IdentityMismatch,
}

impl fmt::Display for VerifiedCanonicalKernelIrErrorV13 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Encode(error) => write!(formatter, "cannot encode canonical KIR V13: {error}"),
            Self::Decode(error) => write!(formatter, "cannot decode canonical KIR V13: {error}"),
            Self::Verification(error) => error.fmt(formatter),
            Self::NotExactV13 { version } => {
                write!(formatter, "expected exact KIR V13 bytes, found V{version}")
            }
            Self::RoundTripMismatch => formatter.write_str("KIR V13 round trip changed semantics"),
            Self::IdentityMismatch => formatter.write_str("canonical KIR V13 identity mismatch"),
        }
    }
}

impl Error for VerifiedCanonicalKernelIrErrorV13 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Encode(error) => Some(error),
            Self::Decode(error) => Some(error),
            Self::Verification(error) => Some(error),
            Self::NotExactV13 { .. } | Self::RoundTripMismatch | Self::IdentityMismatch => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_owner_rejects_v12() {
        let bytes = crate::encode_module_v12(&Module::new("canonical-v13-hostile")).unwrap();
        assert!(matches!(
            VerifiedCanonicalKernelIrV13::from_canonical_bytes(bytes),
            Err(VerifiedCanonicalKernelIrErrorV13::NotExactV13 { version: 12 })
        ));
    }
}
