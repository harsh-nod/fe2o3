//! Opaque exact-profile finalization for masked Wave64 collectives V1.

use std::{error::Error, fmt};

use fe2o3_kernel_descriptor::{CanonicalCodeObjectDigest, CodeObjectVersion, DeviceTargetV1};
use sha2::{Digest, Sha256};

use crate::{
    ContentIdentityV1, FinalizedWorkerV2HsacoIdentityV1,
    InspectedWave64CollectivesV1WorkerV2HsacoV1, PreparedFinalizedWorkerV2HsacoV1,
    ValidatedWave64CollectivesV1WorkerExchangeV1, WorkerV2HsacoFinalizationError,
    finalize_inspected_worker_v2_hsaco_v1,
};

const FINALIZED_IDENTITY_DOMAIN: &[u8] =
    b"FE2O3/WAVE64-COLLECTIVES-V1/OPAQUE-FINALIZED-ADMISSION/V1\0";

/// Stable identity of one exact Wave64 Worker V2 finalization transition.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct FinalizedWave64CollectivesV1HsacoIdentityV1([u8; 32]);

impl FinalizedWave64CollectivesV1HsacoIdentityV1 {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Linear, opaque admission for one finalized exact Wave64 code object.
///
/// This value deliberately exposes neither artifact bytes nor a generic
/// publication, load, or launch conversion. It retains the measured Worker V2
/// exchange and canonical finalizer lineage for a later private runtime join.
#[derive(Debug)]
pub struct PreparedFinalizedWave64CollectivesV1HsacoV1 {
    identity: FinalizedWave64CollectivesV1HsacoIdentityV1,
    exchange: ValidatedWave64CollectivesV1WorkerExchangeV1,
    finalized: PreparedFinalizedWorkerV2HsacoV1,
}

impl PreparedFinalizedWave64CollectivesV1HsacoV1 {
    pub const fn identity(&self) -> FinalizedWave64CollectivesV1HsacoIdentityV1 {
        self.identity
    }

    pub const fn exchange(&self) -> ValidatedWave64CollectivesV1WorkerExchangeV1 {
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

    /// Borrows the exact finalized bytes for the reviewed Wave64 lifecycle.
    ///
    /// This is a doc-hidden runtime SPI, not artifact extraction authority.
    /// Safe code cannot call it, and the returned borrow remains tied to this
    /// linear admission.
    ///
    /// # Safety
    ///
    /// The caller must be the exact typed Wave64 publication/load lifecycle.
    /// It must use this borrow only for one identity-checked HSA load, must not
    /// retain or copy the bytes outside that lifecycle, and must not expose the
    /// bytes or derive generic publication, load, or launch authority from
    /// them.
    #[doc(hidden)]
    #[allow(unsafe_code)]
    pub unsafe fn exact_finalized_bytes_for_reviewed_wave64_runtime_v1(&self) -> &[u8] {
        self.finalized.exact_finalized_bytes()
    }

    pub const fn exact_profile_descriptor_source_was_checked(&self) -> bool {
        true
    }

    pub const fn exact_five_argument_abi_was_checked(&self) -> bool {
        true
    }

    pub const fn direct_upstream_llvm_worker_exchange_was_checked(&self) -> bool {
        true
    }

    pub const fn authenticates_compiler_origin(&self) -> bool {
        false
    }

    pub const fn proves_compiler_refinement(&self) -> bool {
        false
    }

    pub const fn proves_functional_collectives(&self) -> bool {
        false
    }

    pub const fn proves_no_comgr_linkage(&self) -> bool {
        false
    }

    pub const fn no_comgr_requires_measured_worker_build_manifest(&self) -> bool {
        true
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
}

#[derive(Debug)]
#[non_exhaustive]
pub enum Wave64CollectivesV1FinalizationErrorV1 {
    Structural(WorkerV2HsacoFinalizationError),
    ProfileMismatch(&'static str),
}

impl fmt::Display for Wave64CollectivesV1FinalizationErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Structural(error) => write!(formatter, "Wave64 finalization failed: {error}"),
            Self::ProfileMismatch(field) => {
                write!(formatter, "finalized Wave64 profile mismatch: {field}")
            }
        }
    }
}

impl Error for Wave64CollectivesV1FinalizationErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Structural(error) => Some(error),
            Self::ProfileMismatch(_) => None,
        }
    }
}

/// Consumes exact raw inspection and performs canonical descriptor finalization.
pub fn finalize_wave64_collectives_v1_worker_v2_hsaco_v1(
    inspected: InspectedWave64CollectivesV1WorkerV2HsacoV1,
) -> Result<PreparedFinalizedWave64CollectivesV1HsacoV1, Wave64CollectivesV1FinalizationErrorV1> {
    let (exchange, raw) = inspected.into_parts();
    let finalized = finalize_inspected_worker_v2_hsaco_v1(raw)
        .map_err(Wave64CollectivesV1FinalizationErrorV1::Structural)?;
    if finalized.target().to_string() != "gfx942:xnack-"
        || finalized.code_object_version() != CodeObjectVersion::V6
        || !finalized.canonical_descriptor_finalization_ran()
        || finalized.finalized_output_identity().sha256() == &[0; 32]
        || finalized.canonical_digest().as_bytes() == &[0; 32]
    {
        return Err(Wave64CollectivesV1FinalizationErrorV1::ProfileMismatch(
            "target/COV6/canonical descriptor lineage",
        ));
    }
    let identity = calculate_identity(exchange, &finalized);
    Ok(PreparedFinalizedWave64CollectivesV1HsacoV1 {
        identity,
        exchange,
        finalized,
    })
}

fn calculate_identity(
    exchange: ValidatedWave64CollectivesV1WorkerExchangeV1,
    finalized: &PreparedFinalizedWorkerV2HsacoV1,
) -> FinalizedWave64CollectivesV1HsacoIdentityV1 {
    let mut digest = Sha256::new();
    digest.update(FINALIZED_IDENTITY_DOMAIN);
    digest.update(exchange.identity().as_bytes());
    digest.update(exchange.compiler_module_identity().sha256());
    digest.update(exchange.compiler_module_identity().byte_len().to_le_bytes());
    digest.update(exchange.compiler_pins().source_authority());
    digest.update(exchange.compiler_pins().portable_mir_sha256());
    digest.update(exchange.compiler_pins().canonical_kernel_ir_identity());
    digest.update(exchange.compiler_pins().descriptor_profile_identity());
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
    FinalizedWave64CollectivesV1HsacoIdentityV1(digest.finalize().into())
}
