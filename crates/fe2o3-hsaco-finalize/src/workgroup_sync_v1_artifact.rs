//! Opaque exact-profile finalization for workgroup synchronization V1.

use std::{error::Error, fmt};

use fe2o3_kernel_descriptor::{CanonicalCodeObjectDigest, CodeObjectVersion, DeviceTargetV1};
use sha2::{Digest, Sha256};

use crate::{
    ContentIdentityV1, FinalizedWorkerV2HsacoIdentityV1, InspectedWorkgroupSyncWorkerV2HsacoV1,
    PreparedFinalizedWorkerV2HsacoV1, ValidatedWorkgroupSyncWorkerExchangeV1,
    WorkerV2HsacoFinalizationError, WorkgroupSyncProfileKindV1,
    finalize_inspected_worker_v2_hsaco_v1,
};

const FINALIZED_IDENTITY_DOMAIN: &[u8] = b"FE2O3/WORKGROUP-SYNC-V1/OPAQUE-FINALIZED-ADMISSION/V1\0";

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct FinalizedWorkgroupSyncHsacoIdentityV1([u8; 32]);

impl FinalizedWorkgroupSyncHsacoIdentityV1 {
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Linear, opaque admission for one exact workgroup-sync code object.
///
/// Bytes remain private and there is no publication, load, or launch
/// conversion. The value retains the exact compiler/worker exchange and the
/// canonical descriptor-finalization lineage for a future typed runtime join.
#[derive(Debug)]
pub struct PreparedFinalizedWorkgroupSyncHsacoV1 {
    identity: FinalizedWorkgroupSyncHsacoIdentityV1,
    profile: WorkgroupSyncProfileKindV1,
    exchange: ValidatedWorkgroupSyncWorkerExchangeV1,
    finalized: PreparedFinalizedWorkerV2HsacoV1,
}

impl PreparedFinalizedWorkgroupSyncHsacoV1 {
    pub const fn identity(&self) -> FinalizedWorkgroupSyncHsacoIdentityV1 {
        self.identity
    }

    pub const fn profile(&self) -> WorkgroupSyncProfileKindV1 {
        self.profile
    }

    pub const fn exchange(&self) -> ValidatedWorkgroupSyncWorkerExchangeV1 {
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

    pub const fn exact_source_kir_profile_was_checked(&self) -> bool {
        true
    }

    pub const fn direct_upstream_llvm_lld_exchange_was_checked(&self) -> bool {
        true
    }

    pub const fn authenticates_compiler_origin(&self) -> bool {
        false
    }

    pub const fn proves_compiler_refinement(&self) -> bool {
        false
    }

    pub const fn proves_source_refinement(&self) -> bool {
        false
    }

    pub const fn proves_machine_refinement(&self) -> bool {
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
pub enum WorkgroupSyncFinalizationErrorV1 {
    Structural(WorkerV2HsacoFinalizationError),
    ProfileMismatch(&'static str),
}

impl fmt::Display for WorkgroupSyncFinalizationErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Structural(error) => {
                write!(formatter, "workgroup-sync finalization failed: {error}")
            }
            Self::ProfileMismatch(field) => {
                write!(
                    formatter,
                    "finalized workgroup-sync profile mismatch: {field}"
                )
            }
        }
    }
}

impl Error for WorkgroupSyncFinalizationErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Structural(error) => Some(error),
            Self::ProfileMismatch(_) => None,
        }
    }
}

pub fn finalize_workgroup_sync_v1_worker_v2_hsaco_v1(
    inspected: InspectedWorkgroupSyncWorkerV2HsacoV1,
) -> Result<PreparedFinalizedWorkgroupSyncHsacoV1, WorkgroupSyncFinalizationErrorV1> {
    let profile = inspected.profile();
    let (exchange, raw) = inspected.into_parts();
    let finalized = finalize_inspected_worker_v2_hsaco_v1(raw)
        .map_err(WorkgroupSyncFinalizationErrorV1::Structural)?;
    if finalized.target().to_string() != "gfx942:xnack-"
        || finalized.code_object_version() != CodeObjectVersion::V6
        || !finalized.canonical_descriptor_finalization_ran()
        || finalized.finalized_output_identity().sha256() == &[0; 32]
        || finalized.canonical_digest().as_bytes() == &[0; 32]
    {
        return Err(WorkgroupSyncFinalizationErrorV1::ProfileMismatch(
            "target/COV6/canonical descriptor lineage",
        ));
    }
    let identity = calculate_identity(profile, exchange, &finalized);
    Ok(PreparedFinalizedWorkgroupSyncHsacoV1 {
        identity,
        profile,
        exchange,
        finalized,
    })
}

fn calculate_identity(
    profile: WorkgroupSyncProfileKindV1,
    exchange: ValidatedWorkgroupSyncWorkerExchangeV1,
    finalized: &PreparedFinalizedWorkerV2HsacoV1,
) -> FinalizedWorkgroupSyncHsacoIdentityV1 {
    let compiler = exchange.compiler_pins();
    let mut digest = Sha256::new();
    digest.update(FINALIZED_IDENTITY_DOMAIN);
    digest.update([match profile {
        WorkgroupSyncProfileKindV1::LdsReduction => 1,
        WorkgroupSyncProfileKindV1::ScopedAtomic => 2,
    }]);
    digest.update(exchange.identity().as_bytes());
    digest.update(exchange.compiler_module_identity().sha256());
    digest.update(exchange.compiler_module_identity().byte_len().to_le_bytes());
    digest.update(compiler.source_authority());
    digest.update(compiler.kernel_ir_identity());
    digest.update(compiler.descriptor_profile_identity());
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
    FinalizedWorkgroupSyncHsacoIdentityV1(digest.finalize().into())
}
