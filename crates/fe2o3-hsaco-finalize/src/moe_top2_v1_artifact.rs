//! Opaque finalization receipt for exact T8/E4/K2/C4 MoE routing.

use std::{error::Error, fmt};

use fe2o3_kernel_descriptor::{CanonicalCodeObjectDigest, CodeObjectVersion, DeviceTargetV1};
use sha2::{Digest, Sha256};

use crate::{
    ContentIdentityV1, FinalizedWorkerV2HsacoIdentityV1, InspectedMoeTop2V1WorkerV2HsacoV1,
    PreparedFinalizedWorkerV2HsacoV1, ValidatedMoeTop2V1WorkerExchangeV1,
    WorkerV2HsacoFinalizationError, finalize_inspected_worker_v2_hsaco_v1,
};

const FINALIZED_IDENTITY_DOMAIN: &[u8] =
    b"FE2O3/MOE-TOP2-T8-E4-K2-C4/OPAQUE-FINALIZED-ADMISSION/V1\0";

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct FinalizedMoeTop2V1HsacoIdentityV1([u8; 32]);

impl FinalizedMoeTop2V1HsacoIdentityV1 {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Linear inert receipt for one exact MoE code object.
///
/// This value intentionally exposes identities only. It has no byte,
/// publication, load, launch, runtime, hardware, or proof conversion.
#[derive(Debug)]
pub struct PreparedFinalizedMoeTop2V1HsacoV1 {
    identity: FinalizedMoeTop2V1HsacoIdentityV1,
    exchange: ValidatedMoeTop2V1WorkerExchangeV1,
    finalized: PreparedFinalizedWorkerV2HsacoV1,
}

impl PreparedFinalizedMoeTop2V1HsacoV1 {
    pub const fn identity(&self) -> FinalizedMoeTop2V1HsacoIdentityV1 {
        self.identity
    }

    pub const fn exchange(&self) -> ValidatedMoeTop2V1WorkerExchangeV1 {
        self.exchange
    }

    pub const fn structural_finalization_identity(&self) -> FinalizedWorkerV2HsacoIdentityV1 {
        self.finalized.identity()
    }

    pub const fn raw_output_identity(&self) -> ContentIdentityV1 {
        self.finalized.raw_output_identity()
    }

    pub const fn finalized_output_identity(&self) -> ContentIdentityV1 {
        self.finalized.finalized_output_identity()
    }

    pub const fn target(&self) -> DeviceTargetV1 {
        self.finalized.target()
    }

    pub const fn code_object_version(&self) -> CodeObjectVersion {
        self.finalized.code_object_version()
    }

    pub const fn canonical_digest(&self) -> CanonicalCodeObjectDigest {
        self.finalized.canonical_digest()
    }

    pub const fn exact_source_kir_compiler_profile_was_checked(&self) -> bool {
        true
    }

    pub const fn direct_upstream_llvm_lld_exchange_was_checked(&self) -> bool {
        true
    }

    pub const fn authenticates_compiler_origin(&self) -> bool {
        false
    }

    pub const fn proves_source_refinement(&self) -> bool {
        false
    }

    pub const fn proves_compiler_refinement(&self) -> bool {
        false
    }

    pub const fn proves_machine_refinement(&self) -> bool {
        false
    }

    pub const fn grants_publication_authority(&self) -> bool {
        false
    }

    pub const fn grants_load_authority(&self) -> bool {
        false
    }

    pub const fn grants_launch_authority(&self) -> bool {
        false
    }

    /// Borrows the retained artifact only at the reviewed exact MoE runtime boundary.
    ///
    /// # Safety
    ///
    /// `consume` must pass the bytes directly to the exact T8/E4/K2/C4
    /// reviewed runtime adapter. It must not copy, persist, publish, return,
    /// reinterpret, or expose the bytes or derive generic load authority from
    /// them. The receipt must remain retained through load validation.
    #[doc(hidden)]
    #[allow(unsafe_code)]
    pub unsafe fn with_exact_finalized_bytes_for_reviewed_moe_top2_runtime_v1<T>(
        &self,
        consume: impl FnOnce(&[u8], ContentIdentityV1) -> T,
    ) -> T {
        consume(
            self.finalized.exact_finalized_bytes(),
            self.finalized.finalized_output_identity(),
        )
    }
}

#[derive(Debug)]
#[non_exhaustive]
pub enum MoeTop2V1FinalizationErrorV1 {
    Structural(WorkerV2HsacoFinalizationError),
    ProfileMismatch(&'static str),
}

impl fmt::Display for MoeTop2V1FinalizationErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Structural(error) => write!(formatter, "MoE finalization failed: {error}"),
            Self::ProfileMismatch(field) => {
                write!(formatter, "finalized exact MoE profile mismatch: {field}")
            }
        }
    }
}

impl Error for MoeTop2V1FinalizationErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Structural(error) => Some(error),
            Self::ProfileMismatch(_) => None,
        }
    }
}

pub fn finalize_moe_top2_v1_worker_v2_hsaco_v1(
    inspected: InspectedMoeTop2V1WorkerV2HsacoV1,
) -> Result<PreparedFinalizedMoeTop2V1HsacoV1, MoeTop2V1FinalizationErrorV1> {
    let (exchange, raw) = inspected.into_parts();
    let finalized = finalize_inspected_worker_v2_hsaco_v1(raw)
        .map_err(MoeTop2V1FinalizationErrorV1::Structural)?;
    if finalized.target().to_string() != "gfx942:xnack-"
        || finalized.code_object_version() != CodeObjectVersion::V6
        || !finalized.canonical_descriptor_finalization_ran()
        || finalized.finalized_output_identity().sha256() == &[0; 32]
        || finalized.canonical_digest().as_bytes() == &[0; 32]
    {
        return Err(MoeTop2V1FinalizationErrorV1::ProfileMismatch(
            "target/COV6/canonical descriptor lineage",
        ));
    }
    let identity = calculate_identity(exchange, &finalized);
    Ok(PreparedFinalizedMoeTop2V1HsacoV1 {
        identity,
        exchange,
        finalized,
    })
}

fn calculate_identity(
    exchange: ValidatedMoeTop2V1WorkerExchangeV1,
    finalized: &PreparedFinalizedWorkerV2HsacoV1,
) -> FinalizedMoeTop2V1HsacoIdentityV1 {
    let mut digest = Sha256::new();
    digest.update(FINALIZED_IDENTITY_DOMAIN);
    digest.update(exchange.identity().as_bytes());
    digest.update(exchange.compiler_module_identity().sha256());
    digest.update(exchange.compiler_module_identity().byte_len().to_le_bytes());
    digest.update(exchange.linked_output_identity().sha256());
    digest.update(exchange.linked_output_identity().byte_len().to_le_bytes());
    digest.update(finalized.identity().as_bytes());
    digest.update(finalized.raw_output_identity().sha256());
    digest.update(finalized.raw_output_identity().byte_len().to_le_bytes());
    digest.update(finalized.finalized_output_identity().sha256());
    digest.update(
        finalized
            .finalized_output_identity()
            .byte_len()
            .to_le_bytes(),
    );
    digest.update(finalized.canonical_digest().as_bytes());
    FinalizedMoeTop2V1HsacoIdentityV1(digest.finalize().into())
}
