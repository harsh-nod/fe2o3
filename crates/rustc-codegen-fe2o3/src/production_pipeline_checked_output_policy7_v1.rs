//! Fixed Policy7: retained Policy6 plus one source-checked redundant Store pass.
//! Signed source custody is a distinct native stage. No serialized final-proof,
//! protected or default entry is added.
#![allow(
    dead_code,
    reason = "extraction-only Policy7 owner; protected/default dispatch remains closed"
)]
use super::checked_output_artifacts_v1::{
    CheckedArtifactsOwnerRefV1, PreparedCheckedArtifactPartsV1, prepare_checked_artifact_parts_v1,
};
use super::native_checked_output_handoff_v1::{OutputInputsV1, OutputOwnerV1};
use super::*;
use fe2o3_amd_target::ProductionAmdTargetProfileV1 as Profile;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkBudgetV1 as Work, VerifiedCanonicalKernelIrModuleV12 as Graph,
};
use fe2o3_lower_mir_kernel::{
    ProductionOwnedRedundantStoreContinuationV1 as Direct,
    ProductionOwnedUnitLocalRedundantStoreContinuationV1 as Erased,
};
use std::{
    mem::size_of,
    panic::{AssertUnwindSafe, catch_unwind},
};

#[path = "production_policy7_execution_v1.rs"]
mod execution;
pub(crate) use execution::Policy7ExecutionWitnessV1;

#[path = "production_policy7_extraction_v1.rs"]
mod extraction;

#[cfg(test)]
#[path = "production_policy7_source_observation_v1_tests.rs"]
pub(crate) mod source_observation;

#[path = "production_policy7_native_v1.rs"]
pub(crate) mod native;

#[derive(Debug)]
pub(crate) enum CheckedOutputPolicy7StageErrorV1 {
    Resource(Resource),
    Admission(Box<fe2o3_lower_mir_kernel::ProductionRedundantStoreAdmissionErrorV1>),
    Native(Box<super::native_checked_output_handoff_v1::NativeOutputHandoffErrorV1>),
    NativeSource(Box<crate::production_native_source_lineage_v1::NativeSourceLineageErrorV1>),
    InputAssociation(Box<super::native_checked_output_handoff_v1::policy6::final_receipts::input_association::NativeInputAssociationErrorPolicy6V1>),
    Execution(&'static str),
    NativePublicationUnavailable,
    Panicked,
}
impl fmt::Display for CheckedOutputPolicy7StageErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for CheckedOutputPolicy7StageErrorV1 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Admission(error) => Some(error.as_ref()),
            Self::Native(error) => Some(error.as_ref()),
            Self::NativeSource(error) => Some(error.as_ref()),
            Self::InputAssociation(error) => Some(error.as_ref()),
            _ => None,
        }
    }
}
type Result7<T> = Result<T, ProductionPipelineError>;
fn error(error: CheckedOutputPolicy7StageErrorV1) -> ProductionPipelineError {
    ProductionPipelineError::CheckedOutputPolicy7Stage(error)
}
fn resource(value: Resource) -> ProductionPipelineError {
    error(CheckedOutputPolicy7StageErrorV1::Resource(value))
}
fn execution_error(detail: &'static str) -> ProductionPipelineError {
    error(CheckedOutputPolicy7StageErrorV1::Execution(detail))
}
pub(super) fn admission(
    value: fe2o3_lower_mir_kernel::ProductionRedundantStoreAdmissionErrorV1,
) -> ProductionPipelineError {
    error(CheckedOutputPolicy7StageErrorV1::Admission(Box::new(value)))
}

#[allow(
    clippy::large_enum_variant,
    reason = "move-only source/history custody; no second Prefix6"
)]
enum Admitted7 {
    Direct(Direct),
    Erased(Erased),
}
impl Admitted7 {
    fn output(&self) -> &Graph {
        match self {
            Self::Direct(v) => v.output(),
            Self::Erased(v) => v.output(),
        }
    }
    fn input(&self) -> &Graph {
        match self {
            Self::Direct(v) => v.prefix().output(),
            Self::Erased(v) => v.prefix().output(),
        }
    }
    fn prefix_record(&self) -> &[u8; 256] {
        match self {
            Self::Direct(v) => v.prefix().checked_output().execution().canonical_bytes(),
            Self::Erased(v) => v.prefix().checked_output().execution().canonical_bytes(),
        }
    }
    fn continuation(&self) -> &fe2o3_kernel_opt::OwnedRedundantStoreContinuationV1 {
        match self {
            Self::Direct(v) => v.continuation(),
            Self::Erased(v) => v.continuation(),
        }
    }
    fn original(&self) -> &Graph {
        match self {
            Self::Direct(v) => v
                .prefix()
                .source_semantic_kir()
                .pre_ranked_executable()
                .expect("admitted Direct source N"),
            Self::Erased(v) => v.prefix().original_source().executable(),
        }
    }
    fn artifact_view(&self) -> CheckedArtifactsOwnerRefV1<'_> {
        match self {
            Self::Direct(v) => CheckedArtifactsOwnerRefV1::Direct7(v),
            Self::Erased(v) => CheckedArtifactsOwnerRefV1::Erased7(v),
        }
    }
    fn native_view(&self) -> OutputOwnerV1<'_> {
        match self {
            Self::Direct(v) => OutputOwnerV1::Direct7(v),
            Self::Erased(v) => OutputOwnerV1::Erased7(v),
        }
    }
    fn replay(&self, budget: &mut Budget<'_>) -> Result7<()> {
        match self {
            Self::Direct(v) => v.verify_equivalence(budget),
            Self::Erased(v) => v.verify_equivalence(budget),
        }
        .map_err(admission)
    }
}

#[derive(Clone, Copy)]
pub(crate) struct Policy7ArtifactsStorageV1(usize);
impl Policy7ArtifactsStorageV1 {
    pub(crate) const fn retained_storage(self) -> usize {
        self.0
    }
}

/// Owns the actual J and its artifacts; Prefix6 remains inside its lowerer owner.
pub(crate) struct PreparedPolicy7ArtifactsV1 {
    admitted: Admitted7,
    execution: Policy7ExecutionWitnessV1,
    parts: PreparedCheckedArtifactPartsV1,
    retained_floor: usize,
}
impl PreparedPolicy7ArtifactsV1 {
    pub(crate) fn output(&self) -> &Graph {
        self.admitted.output()
    }
    pub(crate) fn original(&self) -> &Graph {
        self.admitted.original()
    }
    pub(crate) fn execution(&self) -> &Policy7ExecutionWitnessV1 {
        &self.execution
    }
    #[cfg(test)]
    pub(crate) fn descriptor_source(&self) -> &fe2o3_compiler_ffi::CompilerDescriptorSourceV1 {
        self.parts.prepared.descriptor_source()
    }
    #[cfg(test)]
    pub(crate) fn llvm_ir(&self) -> &str {
        self.parts.prepared.llvm_ir()
    }
    pub(crate) const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
    #[cfg(test)]
    pub(crate) fn test_input_v1(&self) -> &Graph {
        self.admitted.input()
    }
    #[cfg(test)]
    pub(crate) fn test_deletion_count_v1(&self) -> usize {
        self.admitted.continuation().rows().len()
    }
    #[cfg(test)]
    pub(crate) fn test_execution_record_v1(&self, budget: &mut Budget<'_>) {
        execution::exercise_exact_record(&self.admitted, budget);
    }
    pub(crate) const fn retained_storage_floor_v1(&self) -> usize {
        self.retained_floor
    }
    pub(crate) fn native_worker_output_v1(&self) -> OutputInputsV1<'_> {
        OutputInputsV1 {
            owner: self.admitted.native_view(),
            catalog: &self.parts.catalog,
            prepared: &self.parts.prepared,
            workgroups: &self.parts.workgroups,
        }
    }
    pub(crate) fn verify_equivalence(
        &self,
        profile: Profile,
        typed: &[crate::compiler_descriptor::TypedDescriptorRootV1],
        budget: &mut Budget<'_>,
    ) -> Result7<()> {
        scoped(self.retained_floor, budget, |budget| {
            self.admitted.replay(budget)?;
            self.execution.check(&self.admitted, budget)?;
            super::native_checked_output_handoff_v1::check_output_inputs_v1(
                self.native_worker_output_v1(),
                profile,
                typed,
                budget,
            )
            .map_err(|e| error(CheckedOutputPolicy7StageErrorV1::Native(Box::new(e))))
        })
    }
}

#[allow(
    clippy::large_enum_variant,
    reason = "one actual move-only lowerer prefix"
)]
enum Prefix6 {
    Direct(fe2o3_lower_mir_kernel::ProductionCheckedOutputOwnerPolicy6V1),
    Erased(fe2o3_lower_mir_kernel::ProductionUnitLocalErasedCheckedOutputOwnerPolicy6V1),
}
impl Prefix6 {
    fn minimum(&self) -> Result7<usize> {
        match self {
            Self::Direct(v) => v.retained_input_storage_floor_v1(),
            Self::Erased(v) => v.retained_input_storage_floor_v1(),
        }
        .map_err(super::checked_output_policy6_v1::admission)
    }
    fn continue_once(self, budget: &mut Budget<'_>) -> Result7<(Admitted7, usize)> {
        let (owner, storage) = match self {
            Self::Direct(v) => {
                let (v, s) = v
                    .continue_redundant_private_stores_v1(budget)
                    .map_err(admission)?;
                (Admitted7::Direct(v), s)
            }
            Self::Erased(v) => {
                let (v, s) = v
                    .continue_redundant_private_stores_v1(budget)
                    .map_err(admission)?;
                (Admitted7::Erased(v), s)
            }
        };
        Ok((owner, storage.retained_storage()))
    }
}

fn prepare(
    prefix: Prefix6,
    profile: Profile,
    typed: &[crate::compiler_descriptor::TypedDescriptorRootV1],
    envelope: Option<fe2o3_compiler_ffi::CompilerFfiEnvelopeV1>,
    budget: &mut Budget<'_>,
) -> Result7<(PreparedPolicy7ArtifactsV1, Policy7ArtifactsStorageV1)> {
    let floor = budget.storage();
    scoped(prefix.minimum()?, budget, move |budget| {
        let (admitted, added) = prefix.continue_once(budget)?;
        budget.reserve_storage(added).map_err(resource)?;
        let execution = Policy7ExecutionWitnessV1::prepare(&admitted, budget)?;
        budget
            .reserve_storage(execution.retained_storage())
            .map_err(resource)?;
        let (parts, artifacts) = prepare_checked_artifact_parts_v1(
            admitted.artifact_view(),
            profile,
            typed,
            envelope,
            size_of::<PreparedPolicy7ArtifactsV1>(),
            budget,
        )?;
        budget.reserve_storage(artifacts).map_err(resource)?;
        let retained = added
            .checked_add(execution.retained_storage())
            .and_then(|n| n.checked_add(artifacts))
            .ok_or_else(|| resource(Resource::Arithmetic))?;
        let value = PreparedPolicy7ArtifactsV1 {
            admitted,
            execution,
            parts,
            retained_floor: floor
                .checked_add(retained)
                .ok_or_else(|| resource(Resource::Arithmetic))?,
        };
        value.verify_equivalence(profile, typed, budget)?;
        Ok((value, Policy7ArtifactsStorageV1(retained)))
    })
}

pub(crate) fn prepare_direct_policy7_artifacts_v1(
    prefix: fe2o3_lower_mir_kernel::ProductionCheckedOutputOwnerPolicy6V1,
    profile: Profile,
    typed: &[crate::compiler_descriptor::TypedDescriptorRootV1],
    envelope: Option<fe2o3_compiler_ffi::CompilerFfiEnvelopeV1>,
    budget: &mut Budget<'_>,
) -> Result7<(PreparedPolicy7ArtifactsV1, Policy7ArtifactsStorageV1)> {
    prepare(Prefix6::Direct(prefix), profile, typed, envelope, budget)
}
pub(crate) fn prepare_erased_policy7_artifacts_v1(
    prefix: fe2o3_lower_mir_kernel::ProductionUnitLocalErasedCheckedOutputOwnerPolicy6V1,
    profile: Profile,
    typed: &[crate::compiler_descriptor::TypedDescriptorRootV1],
    envelope: Option<fe2o3_compiler_ffi::CompilerFfiEnvelopeV1>,
    budget: &mut Budget<'_>,
) -> Result7<(PreparedPolicy7ArtifactsV1, Policy7ArtifactsStorageV1)> {
    prepare(Prefix6::Erased(prefix), profile, typed, envelope, budget)
}

/// Complete collector, source/ranked, historical and actual-J custody. No publisher.
pub(crate) struct CheckedOutputTargetProductionCompilationPolicy7V1 {
    artifacts: PreparedPolicy7ArtifactsV1,
    ranked_verification:
        crate::production_ranked_projection_v1::AuthenticatedRankedVerificationRosterV1,
    bindings: AuthenticatedProductionBindings,
    retained_floor: usize,
}
impl RankedVerifiedProductionCompilation {
    pub(crate) fn lower_fixed_checked_output_policy7_v1(
        self,
    ) -> Result7<CheckedOutputTargetProductionCompilationPolicy7V1> {
        let mut work = Work::new(
            usize::try_from(crate::production_canonical_phase_policy_v1::WORK_LIMIT)
                .map_err(|_| resource(Resource::Arithmetic))?,
        );
        let mut budget = Budget::new(
            &mut work,
            crate::production_canonical_phase_policy_v1::STORAGE_LIMIT,
        );
        self.lower_fixed_checked_output_policy7_with_budget_v1(&mut budget)
    }

    fn lower_fixed_checked_output_policy7_with_budget_v1(
        self,
        budget: &mut Budget<'_>,
    ) -> Result7<CheckedOutputTargetProductionCompilationPolicy7V1> {
        use fe2o3_lower_mir_kernel::ProductionHelperSourcePolicyV1;
        let policy = self.ranked.materialized().helper_source_policy_v1();
        let (artifacts, storage, ranked_verification, bindings) = match policy {
            ProductionHelperSourcePolicyV1::RawEmpty => {
                let stage = self.prepare_admitted_policy6_v1(budget)?;
                let (artifacts, storage) = prepare_direct_policy7_artifacts_v1(
                    stage.admitted,
                    stage.bindings.rustc_target.profile(),
                    &stage.bindings.typed_descriptor_roots,
                    stage.bindings.transaction.compiler_ffi_envelope.clone(),
                    budget,
                )?;
                (
                    artifacts,
                    storage,
                    stage.ranked_verification,
                    stage.bindings,
                )
            }
            ProductionHelperSourcePolicyV1::UnitLocal => {
                let stage = self.prepare_admitted_erased_policy6_v1(budget)?;
                let (artifacts, storage) = prepare_erased_policy7_artifacts_v1(
                    stage.admitted,
                    stage.bindings.rustc_target.profile(),
                    &stage.bindings.typed_descriptor_roots,
                    stage.bindings.transaction.compiler_ffi_envelope.clone(),
                    budget,
                )?;
                (
                    artifacts,
                    storage,
                    stage.ranked_verification,
                    stage.bindings,
                )
            }
        };
        budget
            .reserve_storage(storage.retained_storage())
            .map_err(resource)?;
        let wrapper = size_of::<CheckedOutputTargetProductionCompilationPolicy7V1>()
            .checked_sub(size_of::<PreparedPolicy7ArtifactsV1>())
            .ok_or_else(|| resource(Resource::Arithmetic))?;
        budget.reserve_storage(wrapper).map_err(resource)?;
        let stage = CheckedOutputTargetProductionCompilationPolicy7V1 {
            artifacts,
            ranked_verification,
            bindings,
            retained_floor: budget.storage(),
        };
        stage.verify_equivalence(budget)?;
        Ok(stage)
    }
}
impl CheckedOutputTargetProductionCompilationPolicy7V1 {
    pub(crate) fn output(&self) -> &Graph {
        self.artifacts.output()
    }
    pub(crate) fn original(&self) -> &Graph {
        self.artifacts.original()
    }
    pub(crate) fn execution(&self) -> &Policy7ExecutionWitnessV1 {
        self.artifacts.execution()
    }
    pub(crate) const fn retained_storage_floor_v1(&self) -> usize {
        self.retained_floor
    }
    pub(crate) const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
    /// Replays on a correctly prepaid next-phase ledger; no artifact escape.
    pub(crate) fn verify_equivalence(&self, budget: &mut Budget<'_>) -> Result7<()> {
        scoped(self.retained_floor, budget, |budget| {
            if self
                .bindings
                .rustc_preflight_plan
                .rustc_identity_inventory_sha256()
                != self.bindings.rustc_identity_inventory.sha256()
            {
                return Err(ProductionPipelineError::RustcLineageMismatch);
            }
            if self.ranked_verification.root_count() != self.output().module().kernels.len()
                || !self
                    .ranked_verification
                    .every_functional_verification_is_coherent()
            {
                return Err(execution_error("complete retained ranked roster"));
            }
            self.artifacts.verify_equivalence(
                self.bindings.rustc_target.profile(),
                &self.bindings.typed_descriptor_roots,
                budget,
            )
        })
    }
}

fn scoped<'w, T>(
    required: usize,
    budget: &mut Budget<'w>,
    run: impl FnOnce(&mut Budget<'w>) -> Result7<T>,
) -> Result7<T> {
    if budget.storage() < required {
        return Err(resource(Resource::Accounting));
    }
    let floor = budget.storage();
    let slot = budget as *const Budget<'_> as usize;
    let ledger = budget.work_ledger_identity_v1();
    let mut payload = None;
    let result = match catch_unwind(AssertUnwindSafe(|| run(budget))) {
        Ok(result) => result,
        Err(panic) => {
            payload = Some(panic);
            Err(error(CheckedOutputPolicy7StageErrorV1::Panicked))
        }
    };
    if slot != budget as *const Budget<'_> as usize
        || ledger != budget.work_ledger_identity_v1()
        || budget.storage() < floor
    {
        if let Err(panic) = catch_unwind(AssertUnwindSafe(|| drop(result))) {
            payload = Some(panic);
        }
        drop(payload);
        return Err(resource(Resource::Accounting));
    }
    if let Err(error) = budget.release_storage(budget.storage() - floor) {
        if let Err(panic) = catch_unwind(AssertUnwindSafe(|| drop(result))) {
            payload = Some(panic);
        }
        drop(payload);
        return Err(resource(error));
    }
    drop(payload);
    result
}

#[cfg(test)]
#[path = "production_policy7_execution_v1_tests.rs"]
mod tests;
