//! Source-owned checked induction refinement, emitted from its actual final graph.
//! Historical P7 execution remains distinct. No numbered or publication authority.
use super::*;
#[path = "production_pipeline_refined_forwarding_native_v1.rs"]
mod refined_forwarding_native_v1;
use fe2o3_kernel_analysis::CanonicalKirLoopLimitsV1 as Limits;
use fe2o3_lower_mir_kernel::{
    ProductionInductionRefinementErrorV1 as RefinementAdmissionError,
    ProductionInductionRefinementOriginV1 as SourceOrigin,
    ProductionOwnedInductionRefinementContinuationV1 as DirectRefined,
    ProductionOwnedUnitLocalInductionRefinementContinuationV1 as ErasedRefined,
};

#[derive(Debug)]
pub(crate) enum InductionRefinementNativeStageErrorV1 {
    Resource(Resource),
    Admission(Box<RefinementAdmissionError>),
    ForwardingComposition(refined_forwarding_native_v1::RefinedForwardingNativeStageErrorV1),
    Mismatch(&'static str),
}
impl fmt::Display for InductionRefinementNativeStageErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for InductionRefinementNativeStageErrorV1 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Admission(error) => Some(error.as_ref()),
            Self::ForwardingComposition(error) => Some(error),
            _ => None,
        }
    }
}
fn error(value: InductionRefinementNativeStageErrorV1) -> ProductionPipelineError {
    ProductionPipelineError::InductionRefinementNativeStage(value)
}
fn resource(value: Resource) -> ProductionPipelineError {
    error(InductionRefinementNativeStageErrorV1::Resource(value))
}
fn mismatch(detail: &'static str) -> ProductionPipelineError {
    error(InductionRefinementNativeStageErrorV1::Mismatch(detail))
}
fn admission(value: RefinementAdmissionError) -> ProductionPipelineError {
    error(InductionRefinementNativeStageErrorV1::Admission(Box::new(
        value,
    )))
}

#[allow(
    clippy::large_enum_variant,
    reason = "one consumed source-bearing owner"
)]
enum Refined {
    Direct(DirectRefined),
    Erased(ErasedRefined),
}
impl Refined {
    fn output(&self) -> &Graph {
        match self {
            Self::Direct(v) => v.output(),
            Self::Erased(v) => v.output(),
        }
    }
    fn licm_input(&self) -> &Graph {
        match self {
            Self::Direct(v) => v.prefix().output(),
            Self::Erased(v) => v.prefix().output(),
        }
    }
    fn historical_p8(&self) -> &Graph {
        match self {
            Self::Direct(v) => v.prefix().prefix().prefix().prefix().output(),
            Self::Erased(v) => v.prefix().prefix().prefix().prefix().output(),
        }
    }
    fn history(&self) -> Admitted7Ref<'_> {
        match self {
            Self::Direct(v) => Admitted7Ref::Direct(v.prefix().prefix().prefix().prefix().prefix()),
            Self::Erased(v) => Admitted7Ref::Erased(v.prefix().prefix().prefix().prefix().prefix()),
        }
    }
    fn original(&self) -> Result<&Graph> {
        match self {
            Self::Direct(v) => v
                .prefix()
                .prefix()
                .prefix()
                .prefix()
                .prefix()
                .prefix()
                .source_semantic_kir()
                .pre_ranked_executable()
                .ok_or_else(|| mismatch("original direct induction-refinement source N")),
            Self::Erased(v) => Ok(v
                .prefix()
                .prefix()
                .prefix()
                .prefix()
                .prefix()
                .prefix()
                .original_source()
                .executable()),
        }
    }
    fn origins(&self) -> &[SourceOrigin] {
        match self {
            Self::Direct(v) => v.origins(),
            Self::Erased(v) => v.origins(),
        }
    }
    fn limits(&self) -> Limits {
        match self {
            Self::Direct(v) => v.limits(),
            Self::Erased(v) => v.limits(),
        }
    }
    fn header(&self) -> usize {
        match self {
            Self::Direct(_) => size_of::<DirectRefined>(),
            Self::Erased(_) => size_of::<ErasedRefined>(),
        }
    }
    fn replay(&self, budget: &mut Budget<'_>) -> Result<()> {
        match self {
            Self::Direct(v) => v.verify_equivalence(budget),
            Self::Erased(v) => v.verify_equivalence(budget),
        }
        .map_err(admission)
    }
    fn continue_once(licm: Licm, limits: Limits, budget: &mut Budget<'_>) -> Result<(Self, usize)> {
        let (owner, storage) = match licm {
            Licm::Direct(v) => {
                let (owner, storage) = v
                    .continue_induction_refinement_v1(limits, budget)
                    .map_err(admission)?;
                (Self::Direct(owner), storage)
            }
            Licm::Erased(v) => {
                let (owner, storage) = v
                    .continue_induction_refinement_v1(limits, budget)
                    .map_err(admission)?;
                (Self::Erased(owner), storage)
            }
        };
        budget
            .reserve_storage(storage.retained_storage())
            .map_err(resource)?;
        Ok((owner, storage.retained_storage()))
    }
}

/// Unreserved addition excluding the caller's retained source and siblings.
#[derive(Clone, Copy)]
pub(crate) struct InductionRefinementNativeStorageV1(usize);
impl InductionRefinementNativeStorageV1 {
    pub(crate) const fn retained_storage(self) -> usize {
        self.0
    }
}

pub(crate) struct PreparedInductionRefinementNativeOutputV1 {
    owner: Refined,
    prefix_execution: Policy7ExecutionWitnessV1,
    llvm: String,
    profile: Profile,
    retained_floor: usize,
}
impl PreparedInductionRefinementNativeOutputV1 {
    pub(crate) fn output(&self) -> &Graph {
        self.owner.output()
    }
    pub(crate) fn licm_input(&self) -> &Graph {
        self.owner.licm_input()
    }
    pub(crate) fn historical_p8_output(&self) -> &Graph {
        self.owner.historical_p8()
    }
    pub(crate) fn original(&self) -> Result<&Graph> {
        self.owner.original()
    }
    pub(crate) fn origins(&self) -> &[SourceOrigin] {
        self.owner.origins()
    }
    pub(crate) fn limits(&self) -> Limits {
        self.owner.limits()
    }
    pub(crate) fn llvm_ir(&self) -> &str {
        &self.llvm
    }
    pub(crate) const fn retained_storage_floor_v1(&self) -> usize {
        self.retained_floor
    }
    pub(crate) const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
    pub(crate) fn verify_equivalence(&self, budget: &mut Budget<'_>) -> Result<()> {
        scoped(self.retained_floor, budget, |budget| {
            self.owner.replay(budget)?;
            let scratch = fe2o3_kernel_opt::POLICY7_EXECUTION_HEADER_BYTES_V1;
            budget.reserve_storage(scratch).map_err(resource)?;
            self.prefix_execution
                .check_history_v1(self.owner.history(), budget)?;
            budget.release_storage(scratch).map_err(resource)?;
            check_native_text(self.output(), self.profile, &self.llvm, budget)
        })
    }
}
fn check_native_text(
    output: &Graph,
    profile: Profile,
    actual: &str,
    budget: &mut Budget<'_>,
) -> Result<()> {
    check_native_text_with_errors_v1(output, profile, actual, budget, resource, || {
        mismatch("exact induction-refinement native LLVM")
    })
}
fn prepare(
    prefix: Prefix6,
    profile: Profile,
    limits: Limits,
    budget: &mut Budget<'_>,
) -> Result<(
    PreparedInductionRefinementNativeOutputV1,
    InductionRefinementNativeStorageV1,
)> {
    let floor = budget.storage();
    scoped(prefix.minimum()?, budget, move |budget| {
        let (licm, prefix_execution, history_added, promoted_added, preheaders_added, licm_added) =
            prepare_licm_source_prefix_v1(prefix, budget)?;
        let (owner, refined_added) = Refined::continue_once(licm, limits, budget)?;
        let (llvm, native_storage) = lower_native(owner.output(), profile, budget)?;
        budget.reserve_storage(native_storage).map_err(resource)?;
        // Subtract only the active, already-paid embedded owner and headers.
        // Enum padding and every additional prepared field remain prepaid here.
        let header = size_of::<PreparedInductionRefinementNativeOutputV1>()
            .checked_sub(owner.header())
            .and_then(|n| n.checked_sub(size_of::<Policy7ExecutionWitnessV1>()))
            .and_then(|n| n.checked_sub(size_of::<String>()))
            .ok_or_else(|| resource(Resource::Arithmetic))?;
        budget.reserve_storage(header).map_err(resource)?;
        let retained = history_added
            .checked_add(prefix_execution.retained_storage())
            .and_then(|n| n.checked_add(promoted_added))
            .and_then(|n| n.checked_add(preheaders_added))
            .and_then(|n| n.checked_add(licm_added))
            .and_then(|n| n.checked_add(refined_added))
            .and_then(|n| n.checked_add(native_storage))
            .and_then(|n| n.checked_add(header))
            .ok_or_else(|| resource(Resource::Arithmetic))?;
        let value = PreparedInductionRefinementNativeOutputV1 {
            owner,
            prefix_execution,
            llvm,
            profile,
            retained_floor: floor
                .checked_add(retained)
                .ok_or_else(|| resource(Resource::Arithmetic))?,
        };
        value.verify_equivalence(budget)?;
        Ok((value, InductionRefinementNativeStorageV1(retained)))
    })
}

/// Actual authenticated source/ranked custody and final refined text, without wire authority.
pub(crate) struct InductionRefinementNativeProductionCompilationV1 {
    native: PreparedInductionRefinementNativeOutputV1,
    ranked_verification:
        crate::production_ranked_projection_v1::AuthenticatedRankedVerificationRosterV1,
    bindings: AuthenticatedProductionBindings,
    retained_floor: usize,
}
impl RankedVerifiedProductionCompilation {
    /// Consumes the real source entry; exact limits and source cap refusals are
    /// retained. Reserve the returned addition before any further controlled use.
    pub(crate) fn lower_induction_refinement_native_with_budget_v1(
        self,
        limits: Limits,
        budget: &mut Budget<'_>,
    ) -> Result<(
        InductionRefinementNativeProductionCompilationV1,
        InductionRefinementNativeStorageV1,
    )> {
        let floor = budget.storage();
        scoped(floor, budget, move |budget| {
            let (prefix, ranked_verification, bindings) = self.prepare_native_prefix_v1(budget)?;
            let (native, storage) =
                prepare(prefix, bindings.rustc_target.profile(), limits, budget)?;
            budget
                .reserve_storage(storage.retained_storage())
                .map_err(resource)?;
            let wrapper = size_of::<InductionRefinementNativeProductionCompilationV1>()
                .checked_sub(size_of::<PreparedInductionRefinementNativeOutputV1>())
                .ok_or_else(|| resource(Resource::Arithmetic))?;
            budget.reserve_storage(wrapper).map_err(resource)?;
            let value = InductionRefinementNativeProductionCompilationV1 {
                native,
                ranked_verification,
                bindings,
                retained_floor: budget.storage(),
            };
            value.verify_equivalence(budget)?;
            let retained = budget
                .storage()
                .checked_sub(floor)
                .ok_or_else(|| resource(Resource::Accounting))?;
            Ok((value, InductionRefinementNativeStorageV1(retained)))
        })
    }
}
impl InductionRefinementNativeProductionCompilationV1 {
    pub(crate) fn output(&self) -> &Graph {
        self.native.output()
    }
    pub(crate) fn licm_input(&self) -> &Graph {
        self.native.licm_input()
    }
    pub(crate) fn historical_p8_output(&self) -> &Graph {
        self.native.historical_p8_output()
    }
    pub(crate) fn original(&self) -> Result<&Graph> {
        self.native.original()
    }
    pub(crate) fn origins(&self) -> &[SourceOrigin] {
        self.native.origins()
    }
    pub(crate) fn limits(&self) -> Limits {
        self.native.limits()
    }
    pub(crate) fn llvm_ir(&self) -> &str {
        self.native.llvm_ir()
    }
    pub(crate) const fn retained_storage_floor_v1(&self) -> usize {
        self.retained_floor
    }
    pub(crate) const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
    pub(crate) fn verify_equivalence(&self, budget: &mut Budget<'_>) -> Result<()> {
        scoped(self.retained_floor, budget, |budget| {
            if self
                .bindings
                .rustc_preflight_plan
                .rustc_identity_inventory_sha256()
                != self.bindings.rustc_identity_inventory.sha256()
            {
                return Err(ProductionPipelineError::RustcLineageMismatch);
            }
            if self.native.profile != self.bindings.rustc_target.profile()
                || self.ranked_verification.root_count() != self.output().module().kernels.len()
                || !self
                    .ranked_verification
                    .every_functional_verification_is_coherent()
            {
                return Err(mismatch(
                    "complete induction-refinement target/ranked custody",
                ));
            }
            self.native.verify_equivalence(budget)
        })
    }
}

#[cfg(test)]
#[path = "production_pipeline_induction_refinement_native_v1_tests.rs"]
mod tests;
