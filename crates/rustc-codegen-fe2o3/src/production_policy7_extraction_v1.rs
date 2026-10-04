//! A closed live-ledger extraction phase, not a protected compiler publisher.
use super::*;
use fe2o3_compiler_ffi::{
    CompilerDescriptorSourceV1 as Descriptor, CompilerModuleHandoffV2 as Handoff,
};
use fe2o3_kernel_ir::{CanonicalKernelIrWorkLedgerIdentityV1 as Ledger, Kernel, WorkgroupSize};
use fe2o3_lower_mir_kernel::ProductionHelperSourcePolicyV1 as SourcePolicy;

/// Inert observations of the exact stage consumed by this invocation.
pub(crate) struct Policy7ExtractionObservationV1 {
    pub(crate) original: [u8; 32],
    pub(crate) erased: Option<[u8; 32]>,
    pub(crate) input: [u8; 32],
    pub(crate) output: [u8; 32],
    pub(crate) policy: u16,
    pub(crate) retained_floor: usize,
}

// This token never escapes the closure that holds the originating Work borrow.
// It is not stored in the long-lived convenience stage or used as authority.
struct LiveExtractionPhaseV1 {
    ledger: Ledger,
    slot: usize,
    work: usize,
    storage: usize,
}
impl LiveExtractionPhaseV1 {
    fn capture(budget: &Budget<'_>) -> Self {
        Self {
            ledger: budget.work_ledger_identity_v1(),
            slot: budget as *const Budget<'_> as usize,
            work: budget.work(),
            storage: budget.storage(),
        }
    }

    fn check(&self, budget: &Budget<'_>) -> Result7<()> {
        if self.ledger != budget.work_ledger_identity_v1()
            || self.slot != budget as *const Budget<'_> as usize
            || budget.work() < self.work
            || budget.storage() < self.storage
        {
            return Err(resource(Resource::Accounting));
        }
        Ok(())
    }
}

impl Admitted7 {
    fn semantic_digest(&self) -> [u8; 32] {
        match self {
            Self::Direct(v) => *v
                .prefix()
                .source_semantic_kir()
                .semantic()
                .semantic()
                .semantic_sha256()
                .as_bytes(),
            Self::Erased(v) => *v
                .prefix()
                .original_source()
                .semantic_ssa()
                .source_semantic()
                .semantic_sha256()
                .as_bytes(),
        }
    }
}

impl PreparedPolicy7ArtifactsV1 {
    fn into_extraction_parts(self, budget: &mut Budget<'_>) -> Result7<(Handoff, Descriptor)> {
        check_extraction_metadata(
            &self.parts.workgroups,
            &self.output().module().kernels,
            self.parts.catalog.semantic_source(),
            &self.admitted.semantic_digest(),
            budget,
        )?;
        self.parts
            .prepared
            .into_validated_parts()
            .map_err(ProductionPipelineError::WorkerHandoff)
    }

    /// Exercises only the real unsigned artifact component, not collector
    /// custody, a public runner or a fabricated signed-native stage.
    #[cfg(test)]
    pub(crate) fn test_into_extraction_component_v1(
        self,
        profile: Profile,
        typed: &[crate::compiler_descriptor::TypedDescriptorRootV1],
        budget: &mut Budget<'_>,
    ) -> Result7<(Handoff, Descriptor)> {
        scoped(self.retained_floor, budget, move |budget| {
            self.verify_equivalence(profile, typed, budget)?;
            self.into_extraction_parts(budget)
        })
    }
}

fn metadata_mismatch() -> ProductionPipelineError {
    ProductionPipelineError::RankedVerification(
        crate::production_ranked_projection_v1::ProductionRankedVerificationErrorV1::RosterMetadata(
            "checked-output extraction source, ranked or workgroup roster",
        ),
    )
}

// The admitted output already bounds this roster. This additional exact scan
// still pays its own work on the same ledger before every comparison.
fn check_extraction_metadata(
    workgroups: &[(String, WorkgroupSize)],
    kernels: &[Kernel],
    catalog_source: &[u8; 32],
    actual_source: &[u8; 32],
    budget: &mut Budget<'_>,
) -> Result7<()> {
    budget.charge_work(2).map_err(resource)?;
    if workgroups.len() != kernels.len() {
        return Err(metadata_mismatch());
    }
    for ((name, size), kernel) in workgroups.iter().zip(kernels) {
        budget.charge_work(4).map_err(resource)?;
        let kernel_name = kernel.id.as_str();
        let comparison = name
            .len()
            .checked_add(kernel_name.len())
            .and_then(|bytes| bytes.checked_add(32))
            .ok_or_else(|| resource(Resource::Arithmetic))?;
        budget.charge_work(comparison).map_err(resource)?;
        if name != kernel_name || Some(*size) != kernel.workgroup_size {
            return Err(metadata_mismatch());
        }
    }
    budget.charge_work(64).map_err(resource)?;
    if catalog_source != actual_source {
        return Err(metadata_mismatch());
    }
    Ok(())
}

impl CheckedOutputTargetProductionCompilationPolicy7V1 {
    pub(crate) fn input(&self) -> &Graph {
        self.artifacts.admitted.input()
    }

    pub(crate) fn erased_digest(&self) -> Option<&[u8; 32]> {
        match &self.artifacts.admitted {
            Admitted7::Direct(_) => None,
            Admitted7::Erased(value) => {
                Some(value.prefix().erased().canonical().identity().digest())
            }
        }
    }

    pub(crate) fn source_policy_v1(&self) -> SourcePolicy {
        match &self.artifacts.admitted {
            Admitted7::Direct(_) => SourcePolicy::RawEmpty,
            Admitted7::Erased(_) => SourcePolicy::UnitLocal,
        }
    }

    fn observation(&self) -> Policy7ExtractionObservationV1 {
        Policy7ExtractionObservationV1 {
            original: *self.original().canonical().identity().digest(),
            erased: self.erased_digest().copied(),
            input: *self.input().canonical().identity().digest(),
            output: *self.output().canonical().identity().digest(),
            policy: self.execution().policy_version(),
            retained_floor: self.retained_floor,
        }
    }

    // Only the closed phase below can supply this live continuation checkpoint.
    // Inert outputs stay inside that phase until both have been dropped.
    fn into_worker_handoff_extraction_v1(
        self,
        phase: &LiveExtractionPhaseV1,
        budget: &mut Budget<'_>,
    ) -> Result7<(Handoff, Descriptor)> {
        phase.check(budget)?;
        if !self
            .bindings
            .transaction
            .compiler_custody
            .is_extraction_only()
        {
            return Err(error(
                CheckedOutputPolicy7StageErrorV1::NativePublicationUnavailable,
            ));
        }
        self.verify_equivalence(budget)?;
        phase.check(budget)?;
        self.artifacts.into_extraction_parts(budget)
    }
}

impl RankedVerifiedProductionCompilation {
    /// Prepares and consumes one actual J on the caller's still-live ledger.
    /// The callback borrows the existing inert output; no signed/native owner or
    /// extra preparation is created. Its result is not a publication authority.
    pub(crate) fn with_fixed_checked_output_policy7_extraction_v1(
        self,
        budget: &mut Budget<'_>,
        observe: impl FnOnce(
            Policy7ExtractionObservationV1,
            &Handoff,
            &Descriptor,
        ) -> Result<(), String>,
    ) -> Result7<Result<(), String>> {
        let floor = budget.storage();
        scoped(floor, budget, move |budget| {
            let entry = LiveExtractionPhaseV1::capture(budget);
            let stage = self.lower_fixed_checked_output_policy7_with_budget_v1(budget)?;
            entry.check(budget)?;
            let prepared = LiveExtractionPhaseV1::capture(budget);
            #[cfg(test)]
            if let Err(error) = super::source_observation::observe_actual(&stage, budget) {
                return Ok(Err(error));
            }
            let observation = stage.observation();
            let (handoff, descriptor) =
                stage.into_worker_handoff_extraction_v1(&prepared, budget)?;
            let result = observe(observation, &handoff, &descriptor);
            // Keep the complete source/history/artifact floor reserved while
            // either transferred output is live, even after its source drops.
            prepared.check(budget)?;
            drop((handoff, descriptor));
            Ok(result)
        })
    }
}

#[cfg(test)]
#[path = "production_policy7_extraction_v1_tests.rs"]
mod tests;
