//! Closed descriptor-schema dispatch; Worker protocol and descriptor versions differ.
use crate::{
    ContentIdentityV1, FinalizedProtectedWorkerV3HsacoIdentityV1,
    InspectedProtectedWorkerV3HsacoV1, NOMINAL_DESCRIPTOR_SCRATCH_STORAGE_V3,
    NOMINAL_DESCRIPTOR_SCRATCH_STORAGE_V4, PreparedFinalizedNominalWorkerHsacoV3,
    PreparedFinalizedNominalWorkerHsacoV4, PreparedFinalizedProtectedWorkerV3HsacoV1,
    WorkerV3HsacoPublicationErrorV1 as E,
};
use fe2o3_kernel_descriptor::{CanonicalCodeObjectDigest, DEVICE_DESCRIPTOR_MAGIC};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DescriptorSchema {
    V1,
    NominalV3,
    NominalV4,
}
impl DescriptorSchema {
    pub(crate) fn from_abi(bytes: &[u8]) -> Result<Self, E> {
        // This selects the strict codec only. The finalizer still validates the
        // complete receipt against the actual descriptor and artifact.
        if bytes.get(..8) != Some(DEVICE_DESCRIPTOR_MAGIC.as_slice()) {
            return Err(E::DescriptorSchemaMismatch);
        }
        match bytes.get(8..10) {
            Some([1, 0]) => Ok(Self::V1),
            Some([3, 0]) => Ok(Self::NominalV3),
            Some([4, 0]) => Ok(Self::NominalV4),
            _ => Err(E::DescriptorSchemaMismatch),
        }
    }
    pub(crate) fn derive_raw(self, bytes: &[u8]) -> Result<Vec<u8>, E> {
        match self {
            Self::V1 => Ok(crate::derive_unfinalized_hsaco_from_finalized_v1(bytes)?),
            Self::NominalV3 => crate::derive_unfinalized_nominal_hsaco_v3(
                bytes,
                NOMINAL_DESCRIPTOR_SCRATCH_STORAGE_V3,
                &mut |_| Ok::<_, std::convert::Infallible>(()),
            )
            .map_err(E::NominalArtifact),
            Self::NominalV4 => crate::derive_unfinalized_nominal_hsaco_v4(
                bytes,
                NOMINAL_DESCRIPTOR_SCRATCH_STORAGE_V4,
                &mut |_| Ok::<_, std::convert::Infallible>(()),
            )
            .map_err(E::NominalArtifactV4),
        }
    }

    // Native carrier custody is a separate prerequisite. Extending strict
    // Worker V3 replay must not widen that adapter's supported schemas.
    pub(crate) fn from_native_abi(bytes: &[u8]) -> Result<Self, E> {
        match Self::from_abi(bytes)? {
            Self::V1 => Ok(Self::V1),
            Self::NominalV3 => Ok(Self::NominalV3),
            Self::NominalV4 => Err(E::DescriptorSchemaMismatch),
        }
    }
}

#[derive(Debug)]
pub(crate) enum FinalizedOwner {
    V1(PreparedFinalizedProtectedWorkerV3HsacoV1),
    NominalV3(PreparedFinalizedNominalWorkerHsacoV3),
    NominalV4(PreparedFinalizedNominalWorkerHsacoV4),
}
impl FinalizedOwner {
    pub(crate) fn view(&self) -> FinalizedRef<'_> {
        match self {
            Self::V1(v) => FinalizedRef::V1(v),
            Self::NominalV3(v) => FinalizedRef::NominalV3(v),
            Self::NominalV4(v) => FinalizedRef::NominalV4(v),
        }
    }
    pub(crate) fn into_v1(self) -> Result<PreparedFinalizedProtectedWorkerV3HsacoV1, E> {
        match self {
            Self::V1(v) => Ok(v),
            _ => Err(E::DescriptorSchemaMismatch),
        }
    }
    pub(crate) fn into_nominal(self) -> Result<PreparedFinalizedNominalWorkerHsacoV3, E> {
        match self {
            Self::NominalV3(v) => Ok(v),
            _ => Err(E::DescriptorSchemaMismatch),
        }
    }
    pub(crate) fn into_nominal_v4(self) -> Result<PreparedFinalizedNominalWorkerHsacoV4, E> {
        match self {
            Self::NominalV4(v) => Ok(v),
            _ => Err(E::DescriptorSchemaMismatch),
        }
    }
    pub(crate) fn into_replay_parts(
        self,
    ) -> crate::worker_v3_hsaco_finalization::OwnedPreparedFinalizedProtectedWorkerV3ReplayPartsV1
    {
        match self {
            Self::V1(v) => v.into_compact_replay_parts(),
            Self::NominalV3(v) => v.into_compact_replay_parts(),
            Self::NominalV4(v) => v.into_compact_replay_parts(),
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) enum FinalizedRef<'a> {
    V1(&'a PreparedFinalizedProtectedWorkerV3HsacoV1),
    NominalV3(&'a PreparedFinalizedNominalWorkerHsacoV3),
    NominalV4(&'a PreparedFinalizedNominalWorkerHsacoV4),
}
impl<'a> FinalizedRef<'a> {
    pub(crate) fn schema(self) -> DescriptorSchema {
        match self {
            Self::V1(_) => DescriptorSchema::V1,
            Self::NominalV3(_) => DescriptorSchema::NominalV3,
            Self::NominalV4(_) => DescriptorSchema::NominalV4,
        }
    }
    pub(crate) fn raw(self) -> &'a InspectedProtectedWorkerV3HsacoV1 {
        match self {
            Self::V1(v) => v.raw(),
            Self::NominalV3(v) => v.raw(),
            Self::NominalV4(v) => v.raw(),
        }
    }
    pub(crate) fn identity(self) -> FinalizedProtectedWorkerV3HsacoIdentityV1 {
        match self {
            Self::V1(v) => v.identity(),
            Self::NominalV3(v) => v.identity(),
            Self::NominalV4(v) => v.identity(),
        }
    }
    pub(crate) fn output_identity(self) -> ContentIdentityV1 {
        match self {
            Self::V1(v) => v.finalized_output_identity(),
            Self::NominalV3(v) => v.output_identity(),
            Self::NominalV4(v) => v.output_identity(),
        }
    }
    pub(crate) fn descriptor_identity(self) -> ContentIdentityV1 {
        match self {
            Self::V1(v) => v.canonical_descriptor_evidence_identity(),
            Self::NominalV3(v) => v.descriptor_identity(),
            Self::NominalV4(v) => v.descriptor_identity(),
        }
    }
    pub(crate) fn canonical_digest(self) -> CanonicalCodeObjectDigest {
        match self {
            Self::V1(v) => v.canonical_digest(),
            Self::NominalV3(v) => v.finalized().digest(),
            Self::NominalV4(v) => v.finalized().digest(),
        }
    }
    pub(crate) fn bytes(self) -> &'a [u8] {
        match self {
            Self::V1(v) => v.exact_finalized_bytes(),
            Self::NominalV3(v) => v.finalized().as_bytes(),
            Self::NominalV4(v) => v.finalized().as_bytes(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn abi_dispatch_is_closed_and_requires_the_exact_magic_and_version() {
        let mut bytes = DEVICE_DESCRIPTOR_MAGIC.to_vec();
        bytes.extend_from_slice(&[0, 0]);
        for version in [0u16, 1, 2, 3, 4, 5, 0x0104, u16::MAX] {
            bytes[8..10].copy_from_slice(&version.to_le_bytes());
            match version {
                1 => assert_eq!(
                    DescriptorSchema::from_abi(&bytes).unwrap(),
                    DescriptorSchema::V1
                ),
                3 => assert_eq!(
                    DescriptorSchema::from_abi(&bytes).unwrap(),
                    DescriptorSchema::NominalV3
                ),
                4 => assert_eq!(
                    DescriptorSchema::from_abi(&bytes).unwrap(),
                    DescriptorSchema::NominalV4
                ),
                _ => assert!(matches!(
                    DescriptorSchema::from_abi(&bytes),
                    Err(E::DescriptorSchemaMismatch)
                )),
            }
        }
        bytes[8..10].copy_from_slice(&4u16.to_le_bytes());
        assert!(matches!(
            DescriptorSchema::from_native_abi(&bytes),
            Err(E::DescriptorSchemaMismatch)
        ));
        for version in [1u16, 3] {
            let mut native = bytes.clone();
            native[8..10].copy_from_slice(&version.to_le_bytes());
            assert_eq!(
                DescriptorSchema::from_native_abi(&native).unwrap(),
                DescriptorSchema::from_abi(&native).unwrap()
            );
        }
        for end in 0..bytes.len() {
            assert!(matches!(
                DescriptorSchema::from_abi(&bytes[..end]),
                Err(E::DescriptorSchemaMismatch)
            ));
        }
        bytes[0] ^= 1;
        assert!(matches!(
            DescriptorSchema::from_abi(&bytes),
            Err(E::DescriptorSchemaMismatch)
        ));
    }
}
