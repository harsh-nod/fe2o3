//! Source-owned fixed Policy6 prefix plus one named local-order tail.
//! Only actual L is emitted; this is not a protected publication endpoint.
#![allow(
    dead_code,
    reason = "staged actual-L endpoint; public recipe/default and protected publication remain closed"
)]
use super::checked_output_policy6_v1::AdmittedPolicy6StageV1;
use super::{
    AuthenticatedProductionBindings, ProductionPipelineError, RankedVerifiedProductionCompilation,
};
use fe2o3_amd_target::ProductionAmdTargetProfileV1 as Profile;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource, InertCanonicalKirTransitionReceiptV1,
    VerifiedCanonicalKernelIrModuleV12 as Graph,
};
use fe2o3_kernel_opt::{U32LocalOrderPreferenceV1, U32LocalOrderRegionV1};
use fe2o3_lower_mir_kernel::{
    ProductionHelperSourcePolicyV1 as HelperPolicy,
    ProductionOwnedSourceLocalOrderContinuationV1 as Admitted,
    SourceU32LocalOrderRequestV1 as Request,
};
use std::{fmt, mem::size_of};

#[path = "production_pipeline_source_local_order_scope_v1.rs"]
mod scope;
use scope::{retained, scoped};
#[path = "production_pipeline_source_local_order_artifacts_v1.rs"]
mod artifacts;
pub(crate) use artifacts::{
    PreparedSourceLocalOrderArtifactsV1, SourceLocalOrderArtifactsStorageV1,
    prepare_source_local_order_artifacts_v1,
};

#[derive(Debug)]
pub(crate) enum SourceLocalOrderStageErrorV1 {
    Resource(Resource),
    Admission(Box<fe2o3_lower_mir_kernel::ProductionSourceLocalOrderErrorV1>),
    Native(Box<super::native_checked_output_handoff_v1::NativeOutputHandoffErrorV1>),
    Context(Box<crate::production_semantic_body_v1::ProductionSemanticBodyErrorV1>),
    Execution(&'static str),
    NativePublicationUnavailable,
    Panicked,
}
impl fmt::Display for SourceLocalOrderStageErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for SourceLocalOrderStageErrorV1 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Resource(error) => Some(error),
            Self::Admission(error) => Some(error.as_ref()),
            Self::Native(error) => Some(error.as_ref()),
            Self::Context(error) => Some(error.as_ref()),
            _ => None,
        }
    }
}
type ResultL<T> = Result<T, ProductionPipelineError>;
fn error(value: SourceLocalOrderStageErrorV1) -> ProductionPipelineError {
    ProductionPipelineError::SourceLocalOrderStage(value)
}
fn resource(value: Resource) -> ProductionPipelineError {
    error(SourceLocalOrderStageErrorV1::Resource(value))
}
fn execution(detail: &'static str) -> ProductionPipelineError {
    error(SourceLocalOrderStageErrorV1::Execution(detail))
}
pub(super) fn admission(
    value: fe2o3_lower_mir_kernel::ProductionSourceLocalOrderErrorV1,
) -> ProductionPipelineError {
    error(SourceLocalOrderStageErrorV1::Admission(Box::new(value)))
}
fn native(
    value: super::native_checked_output_handoff_v1::NativeOutputHandoffErrorV1,
) -> ProductionPipelineError {
    error(SourceLocalOrderStageErrorV1::Native(Box::new(value)))
}

fn check_profile(profile: Profile, helper: HelperPolicy, budget: &mut Budget<'_>) -> ResultL<()> {
    budget.charge_work(3).map_err(resource)?;
    if profile != Profile::Gfx942 || helper != HelperPolicy::RawEmpty {
        return Err(execution(
            "source-local-order-policy6-v1 requires gfx942:xnack-/wave64 and RawEmpty",
        ));
    }
    Ok(())
}

/// Original authenticated compiler roster/bindings retained beside actual N/I/L.
/// There is no inert-packet constructor, protected OutputOwnerV1 conversion,
/// mutable legacy escape, or unscheduled artifact fallback.
pub(crate) struct SourceLocalOrderTargetProductionCompilationV1 {
    artifacts: PreparedSourceLocalOrderArtifactsV1,
    ranked_verification:
        crate::production_ranked_projection_v1::AuthenticatedRankedVerificationRosterV1,
    bindings: AuthenticatedProductionBindings,
    retained_floor: usize,
}

impl RankedVerifiedProductionCompilation {
    /// Gate actual authenticated target and source policy before the unchanged
    /// prefix runs. Success keeps its original source/B/history reservations;
    /// failures and caught unwinds restore the entry floor on the same ledger.
    /// The caller joins live HIR to returned admitted.source_semantic_kir before
    /// passing a current N-coordinate request to the consuming continuation.
    pub(super) fn prepare_source_local_order_prefix_v1(
        self,
        budget: &mut Budget<'_>,
    ) -> ResultL<AdmittedPolicy6StageV1> {
        retained(0, budget, move |budget| {
            check_profile(
                self.bindings.rustc_target.profile(),
                self.ranked.materialized().helper_source_policy_v1(),
                budget,
            )?;
            self.prepare_admitted_policy6_v1(budget)
        })
    }
}

/// Consumes the same admitted source stage after the caller's current-source
/// join. Existing prefix reservations remain live; successful added receipts
/// and the authenticated-stage header stay reserved. Refusal never emits I.
pub(super) fn continue_admitted_source_local_order_v1(
    stage: AdmittedPolicy6StageV1,
    request: Request,
    budget: &mut Budget<'_>,
) -> ResultL<SourceLocalOrderTargetProductionCompilationV1> {
    let minimum = stage
        .admitted
        .retained_input_storage_floor_v1()
        .map_err(super::checked_output_policy6_v1::admission)?;
    retained(minimum, budget, move |budget| {
        // Direct Policy6 admission already requires the RawEmpty source policy.
        // The immutable actual target is checked again at this consuming seam.
        check_profile(
            stage.bindings.rustc_target.profile(),
            HelperPolicy::RawEmpty,
            budget,
        )?;
        let (artifacts, storage): (
            PreparedSourceLocalOrderArtifactsV1,
            SourceLocalOrderArtifactsStorageV1,
        ) = prepare_source_local_order_artifacts_v1(
            stage.admitted,
            request,
            stage.bindings.rustc_target.profile(),
            &stage.bindings.typed_descriptor_roots,
            stage.bindings.transaction.compiler_ffi_envelope.clone(),
            budget,
        )?;
        budget
            .reserve_storage(storage.retained_storage())
            .map_err(resource)?;
        let wrapper = size_of::<SourceLocalOrderTargetProductionCompilationV1>()
            .checked_sub(size_of::<PreparedSourceLocalOrderArtifactsV1>())
            .ok_or_else(|| resource(Resource::Arithmetic))?;
        budget.reserve_storage(wrapper).map_err(resource)?;
        let result = SourceLocalOrderTargetProductionCompilationV1 {
            artifacts,
            ranked_verification: stage.ranked_verification,
            bindings: stage.bindings,
            retained_floor: budget.storage(),
        };
        result.verify_equivalence(budget)?;
        Ok(result)
    })
}

impl SourceLocalOrderTargetProductionCompilationV1 {
    pub(crate) fn admitted(&self) -> &Admitted {
        self.artifacts.admitted()
    }
    pub(crate) fn output(&self) -> &Graph {
        self.artifacts.output()
    }
    pub(crate) fn input(&self) -> &Graph {
        self.admitted().prefix().output()
    }
    pub(crate) fn original(&self) -> ResultL<&Graph> {
        self.artifacts.original()
    }
    pub(crate) fn llvm_ir(&self) -> ResultL<&str> {
        self.artifacts.llvm_ir()
    }
    pub(crate) fn descriptor_source(&self) -> &fe2o3_compiler_ffi::CompilerDescriptorSourceV1 {
        self.artifacts.descriptor_source()
    }
    pub(crate) fn region(&self) -> U32LocalOrderRegionV1 {
        self.admitted().continuation().region()
    }
    pub(crate) fn preference(&self) -> U32LocalOrderPreferenceV1 {
        self.admitted().continuation().preference()
    }
    pub(crate) fn transition_receipt(&self) -> &InertCanonicalKirTransitionReceiptV1 {
        self.admitted().continuation().transition_receipt()
    }
    pub(crate) const fn retained_storage_floor_v1(&self) -> usize {
        self.retained_floor
    }
    pub(crate) const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
    pub(crate) fn require_protected_publication_v1(&self) -> ResultL<()> {
        Err(error(
            SourceLocalOrderStageErrorV1::NativePublicationUnavailable,
        ))
    }

    pub(crate) fn verify_equivalence(&self, budget: &mut Budget<'_>) -> ResultL<()> {
        scoped(self.retained_floor, budget, |budget| {
            check_profile(
                self.bindings.rustc_target.profile(),
                HelperPolicy::RawEmpty,
                budget,
            )?;
            budget.charge_work(68).map_err(resource)?;
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
                return Err(execution(
                    "complete retained source-local-order ranked roster",
                ));
            }
            self.bindings
                .context_entries
                .validate_source(
                    self.admitted()
                        .prefix()
                        .source_semantic_kir()
                        .semantic()
                        .semantic(),
                )
                .map_err(|value| error(SourceLocalOrderStageErrorV1::Context(Box::new(value))))?;
            self.artifacts.verify_equivalence(
                self.bindings.rustc_target.profile(),
                &self.bindings.typed_descriptor_roots,
                budget,
            )
        })
    }
}

#[cfg(test)]
#[path = "production_pipeline_source_local_order_scope_v1_tests.rs"]
mod tests;
