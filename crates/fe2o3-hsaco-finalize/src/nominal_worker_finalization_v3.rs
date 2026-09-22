//! Descriptor-schema V3 continuation of the existing strict Worker V3 transaction.
//! Worker protocol version and descriptor schema version are independent.

use std::{error::Error, fmt};

use crate::{
    ContentIdentityV1, FinalizedNominalHsacoV3, InertProtectedFirstBuildWorkerV3EvidenceV1,
    InspectedProtectedWorkerV3HsacoV1, NominalFinalizationErrorV3, WorkerV3HsacoInspectionError,
    finalize_unfinalized_nominal_hsaco_v3, inspect_unfinalized_nominal_hsaco_v3,
    worker_v3_hsaco_admission::{
        inspect_protected_worker_with_launch, strict_kernel_launch_contract,
    },
};

/// Retains the original strict transaction and exact final artifact. Structural
/// agreement is not compiler/proof authentication or publication/load/launch authority.
#[derive(Debug)]
pub struct PreparedFinalizedNominalWorkerHsacoV3 {
    raw: InspectedProtectedWorkerV3HsacoV1,
    finalized: FinalizedNominalHsacoV3,
    output: ContentIdentityV1,
    descriptor: ContentIdentityV1,
}
impl PreparedFinalizedNominalWorkerHsacoV3 {
    pub fn raw(&self) -> &InspectedProtectedWorkerV3HsacoV1 {
        &self.raw
    }
    pub fn finalized(&self) -> &FinalizedNominalHsacoV3 {
        &self.finalized
    }
    pub fn output_identity(&self) -> ContentIdentityV1 {
        self.output
    }
    pub fn descriptor_identity(&self) -> ContentIdentityV1 {
        self.descriptor
    }
    pub const fn authenticates_compiler_origin(&self) -> bool {
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
}

#[derive(Debug)]
pub enum NominalWorkerFinalizationErrorV3<E> {
    Inspection(WorkerV3HsacoInspectionError),
    Finalization(NominalFinalizationErrorV3<E>),
    ExportManifestMismatch,
}
impl<E: fmt::Display> fmt::Display for NominalWorkerFinalizationErrorV3<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Inspection(e) => write!(f, "nominal worker inspection failed: {e}"),
            Self::Finalization(e) => write!(f, "nominal worker finalization failed: {e}"),
            Self::ExportManifestMismatch => {
                f.write_str("nominal worker export manifest differs from retained receipt")
            }
        }
    }
}
impl<E: Error + 'static> Error for NominalWorkerFinalizationErrorV3<E> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Inspection(e) => Some(e),
            Self::Finalization(e) => Some(e),
            _ => None,
        }
    }
}
impl<E> From<NominalFinalizationErrorV3<E>> for NominalWorkerFinalizationErrorV3<E> {
    fn from(e: NominalFinalizationErrorV3<E>) -> Self {
        Self::Finalization(e)
    }
}
impl<E> From<WorkerV3HsacoInspectionError> for NominalWorkerFinalizationErrorV3<E> {
    fn from(e: WorkerV3HsacoInspectionError) -> Self {
        Self::Inspection(e)
    }
}

/// Consume strict first-build evidence, require exact V3 ABI receipt bytes and
/// common physical launch policy, then use the shared lineage and symbol closure
/// checks. There is no V1 fallback, caller-supplied descriptor, or authority upgrade.
/// `prepaid_scratch` covers only the descriptor traversal described by
/// `NOMINAL_DESCRIPTOR_SCRATCH_STORAGE_V3`; retained evidence stays caller-paid.
pub fn finalize_protected_worker_nominal_hsaco_v3<E>(
    source: InertProtectedFirstBuildWorkerV3EvidenceV1,
    prepaid_scratch: usize,
    charge: &mut impl FnMut(usize) -> Result<(), E>,
) -> Result<PreparedFinalizedNominalWorkerHsacoV3, NominalWorkerFinalizationErrorV3<E>> {
    let launch = {
        let inspected =
            inspect_unfinalized_nominal_hsaco_v3(source.output_bytes(), prepaid_scratch, charge)?;
        let table = inspected.descriptor_table();
        let mut launch = None;
        for index in 0..table.kernel_count() {
            let kernel = table
                .kernel(index, charge)
                .map_err(NominalFinalizationErrorV3::Wire)?;
            let actual = strict_kernel_launch_contract(kernel.launch())?;
            if launch.is_some_and(|expected| expected != actual) {
                return Err(
                    WorkerV3HsacoInspectionError::StrictV3DescriptorLaunchContract(
                        "heterogeneous per-kernel launch policy",
                    )
                    .into(),
                );
            }
            launch = Some(actual);
        }
        launch
            .ok_or(WorkerV3HsacoInspectionError::StrictV3DescriptorLaunchContract("kernel set"))?
    };
    let raw = inspect_protected_worker_with_launch(source, launch)?;
    let handoff = raw.outer_handoff();
    let receipts = handoff.capsule().receipts();
    check_export_manifest(
        receipts.export_manifest().canonical_preimage(),
        handoff.module_handoff().symbol_manifest().canonical_bytes(),
        charge,
    )?;
    let finalized = finalize_unfinalized_nominal_hsaco_v3(
        raw.source_evidence().output_bytes(),
        receipts.abi().canonical_preimage(),
        prepaid_scratch,
        charge,
    )?;
    charge(finalized.as_bytes().len() + finalized.descriptor_bytes().len())
        .map_err(NominalFinalizationErrorV3::Work)?;
    let output = ContentIdentityV1::calculate(finalized.as_bytes());
    let descriptor = ContentIdentityV1::calculate(finalized.descriptor_bytes());
    Ok(PreparedFinalizedNominalWorkerHsacoV3 {
        raw,
        finalized,
        output,
        descriptor,
    })
}

fn check_export_manifest<E>(
    receipt: &[u8],
    manifest: &[u8],
    charge: &mut impl FnMut(usize) -> Result<(), E>,
) -> Result<(), NominalWorkerFinalizationErrorV3<E>> {
    charge(receipt.len().max(manifest.len()).saturating_add(1))
        .map_err(NominalFinalizationErrorV3::Work)?;
    if receipt != manifest {
        return Err(NominalWorkerFinalizationErrorV3::ExportManifestMismatch);
    }
    Ok(())
}

#[cfg(test)]
#[path = "nominal_worker_finalization_v3_tests.rs"]
mod tests;
