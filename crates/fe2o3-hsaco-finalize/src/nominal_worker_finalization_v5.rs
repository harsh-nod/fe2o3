//! Structural descriptor V5 continuation of the existing strict Worker V3 transaction.
//! Mandatory conditional contracts remain bytes, not authenticated proof receipts.
//! This prerequisite does not close #272 or grant publication/load/launch authority.

use std::{error::Error, fmt};

use crate::{
    ContentIdentityV1, FinalizedNominalHsacoV5, InertProtectedFirstBuildWorkerV3EvidenceV1,
    InspectedProtectedWorkerV3HsacoV1, NominalFinalizationErrorV5, WorkerV3HsacoInspectionError,
    finalize_unfinalized_nominal_hsaco_v5, inspect_unfinalized_nominal_hsaco_v5,
    nominal_worker_common::{common_launch, export_manifest_matches},
    worker_v3_hsaco_admission::{
        inspect_protected_worker_with_launch, strict_kernel_launch_contract,
    },
};

/// Move-only V5 artifact and original strict transaction, with no authority upgrade.
/// There is no conversion to a V1/V3 owner or authenticated conditional proof.
///
/// ```compile_fail
/// use fe2o3_hsaco_finalize::PreparedFinalizedNominalWorkerHsacoV5;
/// fn duplicate(value: PreparedFinalizedNominalWorkerHsacoV5) { let _ = value.clone(); }
/// ```
/// ```compile_fail
/// use fe2o3_hsaco_finalize::{PreparedFinalizedNominalWorkerHsacoV4, PreparedFinalizedNominalWorkerHsacoV5};
/// fn downgrade(value: PreparedFinalizedNominalWorkerHsacoV5) -> PreparedFinalizedNominalWorkerHsacoV4 { value.into() }
/// ```
#[derive(Debug)]
pub struct PreparedFinalizedNominalWorkerHsacoV5 {
    raw: InspectedProtectedWorkerV3HsacoV1,
    finalized: FinalizedNominalHsacoV5,
    output: ContentIdentityV1,
    descriptor: ContentIdentityV1,
    identity: crate::FinalizedProtectedWorkerV3HsacoIdentityV1,
}
impl PreparedFinalizedNominalWorkerHsacoV5 {
    pub const fn identity(&self) -> crate::FinalizedProtectedWorkerV3HsacoIdentityV1 {
        self.identity
    }
    pub fn raw(&self) -> &InspectedProtectedWorkerV3HsacoV1 {
        &self.raw
    }
    pub fn finalized(&self) -> &FinalizedNominalHsacoV5 {
        &self.finalized
    }
    pub fn output_identity(&self) -> ContentIdentityV1 {
        self.output
    }
    pub fn descriptor_identity(&self) -> ContentIdentityV1 {
        self.descriptor
    }
    pub const fn is_structural_only(&self) -> bool {
        true
    }
    pub const fn authenticates_compiler_origin(&self) -> bool {
        false
    }
    pub const fn grants_compiler_authority(&self) -> bool {
        false
    }
    pub const fn grants_proof_authority(&self) -> bool {
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
    pub(crate) fn into_compact_replay_parts(
        self,
    ) -> crate::worker_v3_hsaco_finalization::OwnedPreparedFinalizedProtectedWorkerV3ReplayPartsV1
    {
        crate::worker_v3_hsaco_finalization::OwnedPreparedFinalizedProtectedWorkerV3ReplayPartsV1 {
            identity: self.identity,
            source: self.raw.into_source_evidence(),
            finalized_bytes: self.finalized.into_bytes(),
        }
    }
}

#[derive(Debug)]
pub enum NominalWorkerFinalizationErrorV5<E> {
    Inspection(WorkerV3HsacoInspectionError),
    Finalization(NominalFinalizationErrorV5<E>),
    ExportManifestMismatch,
}
impl<E: fmt::Display> fmt::Display for NominalWorkerFinalizationErrorV5<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Inspection(e) => write!(f, "conditional nominal worker inspection failed: {e}"),
            Self::Finalization(e) => {
                write!(f, "conditional nominal worker finalization failed: {e}")
            }
            Self::ExportManifestMismatch => {
                f.write_str("nominal worker export manifest differs from retained receipt")
            }
        }
    }
}
impl<E: Error + 'static> Error for NominalWorkerFinalizationErrorV5<E> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Inspection(e) => Some(e),
            Self::Finalization(e) => Some(e),
            _ => None,
        }
    }
}
impl<E> From<NominalFinalizationErrorV5<E>> for NominalWorkerFinalizationErrorV5<E> {
    fn from(e: NominalFinalizationErrorV5<E>) -> Self {
        Self::Finalization(e)
    }
}
impl<E> From<WorkerV3HsacoInspectionError> for NominalWorkerFinalizationErrorV5<E> {
    fn from(e: WorkerV3HsacoInspectionError) -> Self {
        Self::Inspection(e)
    }
}

/// Retain every mandatory contract and require the entire embedded zero-digest
/// descriptor to equal the original strict ABI receipt. Shared strict lineage,
/// symbol closure and export checks still run; there is no V1/V3 fallback.
/// `prepaid_scratch` covers `NOMINAL_DESCRIPTOR_SCRATCH_STORAGE_V5` only. Retained
/// evidence, ELF/AMDHSA allocations, strict inspection and identity serialization
/// remain in their existing bounded domains outside the descriptor work callback.
pub fn finalize_protected_worker_nominal_hsaco_v5<E>(
    source: InertProtectedFirstBuildWorkerV3EvidenceV1,
    prepaid_scratch: usize,
    charge: &mut impl FnMut(usize) -> Result<(), E>,
) -> Result<PreparedFinalizedNominalWorkerHsacoV5, NominalWorkerFinalizationErrorV5<E>> {
    let launch = {
        let inspected =
            inspect_unfinalized_nominal_hsaco_v5(source.output_bytes(), prepaid_scratch, charge)?;
        let table = inspected.descriptor_table();
        common_launch(table.kernel_count(), |index| {
            let kernel = table
                .kernel(index, charge)
                .map_err(NominalFinalizationErrorV5::Wire)?;
            Ok::<_, NominalWorkerFinalizationErrorV5<E>>(strict_kernel_launch_contract(
                kernel.launch(),
            )?)
        })?
    };
    let raw = inspect_protected_worker_with_launch(source, launch)?;
    let handoff = raw.outer_handoff();
    let receipts = handoff.capsule().receipts();
    if !export_manifest_matches(
        receipts.export_manifest().canonical_preimage(),
        handoff.module_handoff().symbol_manifest().canonical_bytes(),
        charge,
    )
    .map_err(NominalFinalizationErrorV5::Work)?
    {
        return Err(NominalWorkerFinalizationErrorV5::ExportManifestMismatch);
    }
    let finalized = finalize_unfinalized_nominal_hsaco_v5(
        raw.source_evidence().output_bytes(),
        receipts.abi().canonical_preimage(),
        prepaid_scratch,
        charge,
    )?;
    charge(finalized.as_bytes().len() + finalized.descriptor_bytes().len())
        .map_err(NominalFinalizationErrorV5::Work)?;
    let output = ContentIdentityV1::calculate(finalized.as_bytes());
    let descriptor = ContentIdentityV1::calculate(finalized.descriptor_bytes());
    let identity =
        crate::worker_v3_hsaco_finalization::calculate_nominal_worker_finalized_identity_v5(
            &raw, &finalized, output, descriptor,
        );
    Ok(PreparedFinalizedNominalWorkerHsacoV5 {
        raw,
        finalized,
        output,
        descriptor,
        identity,
    })
}
