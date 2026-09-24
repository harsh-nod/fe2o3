//! Closed descriptor-schema dispatch; Worker protocol and descriptor versions differ.
use crate::{
    ContentIdentityV1, FinalizedProtectedWorkerV3HsacoIdentityV1,
    InspectedProtectedWorkerV3HsacoV1, NOMINAL_DESCRIPTOR_SCRATCH_STORAGE_V3,
    PreparedFinalizedNominalWorkerHsacoV3, PreparedFinalizedProtectedWorkerV3HsacoV1,
    WorkerV3HsacoPublicationErrorV1 as E,
};
use fe2o3_kernel_descriptor::{CanonicalCodeObjectDigest, DEVICE_DESCRIPTOR_MAGIC};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DescriptorSchema {
    V1,
    NominalV3,
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
        }
    }
}

#[derive(Debug)]
pub(crate) enum FinalizedOwner {
    V1(PreparedFinalizedProtectedWorkerV3HsacoV1),
    NominalV3(PreparedFinalizedNominalWorkerHsacoV3),
}
impl FinalizedOwner {
    pub(crate) fn view(&self) -> FinalizedRef<'_> {
        match self {
            Self::V1(v) => FinalizedRef::V1(v),
            Self::NominalV3(v) => FinalizedRef::NominalV3(v),
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
    pub(crate) fn into_replay_parts(
        self,
    ) -> crate::worker_v3_hsaco_finalization::OwnedPreparedFinalizedProtectedWorkerV3ReplayPartsV1
    {
        match self {
            Self::V1(v) => v.into_compact_replay_parts(),
            Self::NominalV3(v) => v.into_compact_replay_parts(),
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) enum FinalizedRef<'a> {
    V1(&'a PreparedFinalizedProtectedWorkerV3HsacoV1),
    NominalV3(&'a PreparedFinalizedNominalWorkerHsacoV3),
}
impl<'a> FinalizedRef<'a> {
    pub(crate) fn schema(self) -> DescriptorSchema {
        match self {
            Self::V1(_) => DescriptorSchema::V1,
            Self::NominalV3(_) => DescriptorSchema::NominalV3,
        }
    }
    pub(crate) fn raw(self) -> &'a InspectedProtectedWorkerV3HsacoV1 {
        match self {
            Self::V1(v) => v.raw(),
            Self::NominalV3(v) => v.raw(),
        }
    }
    pub(crate) fn identity(self) -> FinalizedProtectedWorkerV3HsacoIdentityV1 {
        match self {
            Self::V1(v) => v.identity(),
            Self::NominalV3(v) => v.identity(),
        }
    }
    pub(crate) fn output_identity(self) -> ContentIdentityV1 {
        match self {
            Self::V1(v) => v.finalized_output_identity(),
            Self::NominalV3(v) => v.output_identity(),
        }
    }
    pub(crate) fn descriptor_identity(self) -> ContentIdentityV1 {
        match self {
            Self::V1(v) => v.canonical_descriptor_evidence_identity(),
            Self::NominalV3(v) => v.descriptor_identity(),
        }
    }
    pub(crate) fn canonical_digest(self) -> CanonicalCodeObjectDigest {
        match self {
            Self::V1(v) => v.canonical_digest(),
            Self::NominalV3(v) => v.finalized().digest(),
        }
    }
    pub(crate) fn bytes(self) -> &'a [u8] {
        match self {
            Self::V1(v) => v.exact_finalized_bytes(),
            Self::NominalV3(v) => v.finalized().as_bytes(),
        }
    }
}
