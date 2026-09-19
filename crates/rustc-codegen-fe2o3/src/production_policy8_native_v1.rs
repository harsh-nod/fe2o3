//! Signed original N/(E), one retained P7 history and already emitted final K.
//! These owning endpoints add no publisher, serialized chain or launch authority.
use super::super::native_checked_output_handoff_v1::{
    NativeOutputHandoffErrorV1, SourceProofV1, StageInputsV1, check_stage,
};
use super::*;
use crate::production_native_source_lineage_v1::{
    NativeSourceLineageErrorV1, PreparedErasedNativeSourceLineageV1, PreparedNativeSourceLineageV1,
    try_prepare_erased_native_source_lineage_v1, try_prepare_native_source_lineage_v1,
};
use crate::production_ranked_projection_v1::AuthenticatedRankedVerificationRosterV1 as Ranked;

#[path = "production_policy8_native_receipts_v1.rs"]
pub(crate) mod final_receipts;
#[path = "production_policy8_native_input_association_v1.rs"]
pub(crate) mod input_association;
#[cfg(test)]
#[path = "production_policy8_native_v1_tests.rs"]
pub(crate) mod tests;

fn native_error(value: NativeOutputHandoffErrorV1) -> ProductionPipelineError {
    error(CheckedOutputPolicy8StageErrorV1::Native(Box::new(value)))
}
fn source_error(value: NativeSourceLineageErrorV1) -> ProductionPipelineError {
    error(CheckedOutputPolicy8StageErrorV1::NativeSource(Box::new(
        value,
    )))
}

#[allow(
    clippy::large_enum_variant,
    reason = "move the one actual signed source owner in place"
)]
enum SourceLineage8 {
    Direct(PreparedNativeSourceLineageV1),
    Erased(PreparedErasedNativeSourceLineageV1),
}
impl SourceLineage8 {
    fn ranked(&self) -> &Ranked {
        match self {
            Self::Direct(v) => v.ranked(),
            Self::Erased(v) => v.ranked(),
        }
    }
    fn proof(&self) -> SourceProofV1<'_> {
        match self {
            Self::Direct(v) => SourceProofV1::Direct(v.proof()),
            Self::Erased(v) => SourceProofV1::Erased(v.proof()),
        }
    }
    fn original_envelope(&self) -> &[u8] {
        match self {
            Self::Direct(v) => v.native_module(),
            Self::Erased(v) => v.original_native_module(),
        }
    }
    fn stored_header(&self) -> usize {
        match self {
            Self::Direct(_) => size_of::<PreparedNativeSourceLineageV1>(),
            Self::Erased(_) => size_of::<PreparedErasedNativeSourceLineageV1>(),
        }
    }
}

/// Borrow only the retained source. In particular this function has no artifact
/// producer, native-I/J preparation or second semantic import in either branch.
fn prepare_source(
    admitted: &Admitted8,
    ranked: Ranked,
    budget: &mut Budget<'_>,
) -> Result8<(SourceLineage8, usize)> {
    match admitted {
        Admitted8::Direct(owner) => {
            let (lineage, receipt) = try_prepare_native_source_lineage_v1(
                owner.prefix().prefix().source_semantic_kir(),
                ranked,
                budget,
            )
            .map_err(source_error)?;
            Ok((SourceLineage8::Direct(lineage), receipt.retained_storage()))
        }
        Admitted8::Erased(owner) => {
            let (lineage, receipt) = try_prepare_erased_native_source_lineage_v1(
                owner.prefix().prefix().erased_source(),
                ranked,
                budget,
            )
            .map_err(source_error)?;
            Ok((SourceLineage8::Erased(lineage), receipt.retained_storage()))
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Policy8NativeStorageV1(usize);
impl Policy8NativeStorageV1 {
    pub(crate) const fn retained_storage(self) -> usize {
        self.0
    }
}

// The old unsigned header and the source constructor's added header jointly
// credit the ranked field once. Retain any excess; never refund inherited floors.
fn native_header_delta(source_header: usize) -> Result8<usize> {
    let credited = size_of::<CheckedOutputTargetProductionCompilationPolicy8V1>()
        .checked_add(source_header)
        .and_then(|n| n.checked_sub(size_of::<Ranked>()))
        .ok_or_else(|| resource(Resource::Arithmetic))?;
    Ok(size_of::<PreparedNativeCheckedOutputWorkerHandoffPolicy8V1>().saturating_sub(credited))
}

fn finish_native_receipt(
    incoming: usize,
    source_header: usize,
    source_delta: usize,
    budget: &mut Budget<'_>,
) -> Result8<(usize, Policy8NativeStorageV1)> {
    let header = native_header_delta(source_header)?;
    let delta = source_delta
        .checked_add(header)
        .ok_or_else(|| resource(Resource::Arithmetic))?;
    let floor = incoming
        .checked_add(delta)
        .ok_or_else(|| resource(Resource::Arithmetic))?;
    budget.reserve_storage(header).map_err(resource)?;
    Ok((floor, Policy8NativeStorageV1(delta)))
}

/// The actual K artifacts, single prefix, signed N source, and collector stay
/// move-only. A valid signature is necessary but is not protected publication.
pub(crate) struct PreparedNativeCheckedOutputWorkerHandoffPolicy8V1 {
    artifacts: PreparedPolicy8ArtifactsV1,
    source_lineage: SourceLineage8,
    bindings: AuthenticatedProductionBindings,
    retained_floor: usize,
}
impl CheckedOutputTargetProductionCompilationPolicy8V1 {
    /// The existing stage floor must be reserved. Success returns only added
    /// source payload/header storage, unreserved. All exits restore the incoming
    /// ledger floor, retaining cumulative work. No native-I/J artifact is produced.
    pub(crate) fn prepare_native_worker_handoff_v1(
        self,
        budget: &mut Budget<'_>,
    ) -> Result8<(
        PreparedNativeCheckedOutputWorkerHandoffPolicy8V1,
        Policy8NativeStorageV1,
    )> {
        scoped(self.retained_floor, budget, move |budget| {
            self.verify_equivalence(budget)?;
            let Self {
                artifacts,
                ranked_verification,
                bindings,
                retained_floor,
            } = self;
            let (source_lineage, source_delta) =
                prepare_source(&artifacts.admitted, ranked_verification, budget)?;
            budget.reserve_storage(source_delta).map_err(resource)?;
            let (retained_floor, receipt) = finish_native_receipt(
                retained_floor,
                source_lineage.stored_header(),
                source_delta,
                budget,
            )?;
            let native = PreparedNativeCheckedOutputWorkerHandoffPolicy8V1 {
                artifacts,
                source_lineage,
                bindings,
                retained_floor,
            };
            native.verify_equivalence(budget)?;
            Ok((native, receipt))
        })
    }
}
impl PreparedNativeCheckedOutputWorkerHandoffPolicy8V1 {
    pub(crate) fn output(&self) -> &Graph {
        self.artifacts.output()
    }
    pub(crate) fn original(&self) -> &Graph {
        self.artifacts.original()
    }
    pub(crate) const fn policy_version(&self) -> u16 {
        8
    }
    pub(crate) fn prefix_execution(&self) -> &Policy7ExecutionWitnessV1 {
        self.artifacts.prefix_execution()
    }
    pub(crate) fn handoff(&self) -> &fe2o3_compiler_ffi::CompilerModuleHandoffV2 {
        self.artifacts.parts.prepared.native_output_parts_v1().0
    }
    pub(crate) fn original_kernel_ir_preimage(&self) -> &[u8] {
        self.source_lineage.original_envelope()
    }
    pub(crate) const fn retained_storage_floor_v1(&self) -> usize {
        self.retained_floor
    }
    pub(crate) const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
    pub(in crate::production_pipeline) fn ranked(&self) -> &Ranked {
        self.source_lineage.ranked()
    }
    pub(in crate::production_pipeline) fn inputs(&self) -> StageInputsV1<'_> {
        StageInputsV1 {
            output: self.artifacts.native_worker_output_v1(),
            proof: self.source_lineage.proof(),
            bindings: &self.bindings,
            retained_floor: self.retained_floor,
        }
    }
    /// Original-N evidence only. Final output/report checking never uses I/J.
    pub(in crate::production_pipeline) fn original_source_view(&self) -> OutputOwnerV1<'_> {
        match &self.artifacts.admitted {
            Admitted8::Direct(v) => OutputOwnerV1::Direct6(v.prefix().prefix()),
            Admitted8::Erased(v) => OutputOwnerV1::Erased6(v.prefix().prefix()),
        }
    }
    pub(crate) fn verify_equivalence(&self, budget: &mut Budget<'_>) -> Result8<()> {
        scoped(self.retained_floor, budget, |budget| {
            budget.charge_work(2).map_err(resource)?;
            if self.ranked().root_count() != self.output().module().kernels.len() {
                return Err(execution_error("complete retained ranked roster"));
            }
            // Replay binds K while the unchanged P7 witness remains about I/J.
            self.artifacts.check_history(budget)?;
            self.artifacts
                .check_producer(self.bindings.rustc_target.profile(), budget)?;
            check_stage(self.inputs(), budget).map_err(native_error)
        })
    }
}
