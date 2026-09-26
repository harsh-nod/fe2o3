//! Actual L-to-R-to-F source ownership before the first LLVM emission.
use super::*;
use crate::compiler_descriptor::checked_output_policy3_v1::refined_forwarding_v1::{
    FinalOwnerV1, RefinedForwardingDescriptorErrorV1, validate_final_descriptor_evidence_v1,
};
use fe2o3_kernel_analysis::CanonicalKirCrossBlockForwardingLimitsV1 as ForwardingLimits;
use fe2o3_lower_mir_kernel::{
    ProductionCrossBlockForwardingOriginV1 as FinalOrigin,
    ProductionOwnedRefinedCrossBlockForwardingContinuationV1 as DirectComposed,
    ProductionOwnedUnitLocalRefinedCrossBlockForwardingContinuationV1 as ErasedComposed,
    ProductionRefinedCrossBlockForwardingErrorV1 as CompositionAdmissionError,
};

#[path = "production_refined_forwarding_history_v1.rs"]
mod history;
use history::{PreparedRefinedForwardingHistoryClaimsV1, RefinedForwardingHistoryErrorV1};

#[path = "production_refined_forwarding_worker_v1.rs"]
mod worker;

#[path = "production_pipeline_loop_unroll_native_v1.rs"]
mod loop_unroll_native_v1;

#[derive(Debug)]
pub(crate) enum RefinedForwardingNativeStageErrorV1 {
    Resource(Resource),
    Admission(Box<CompositionAdmissionError>),
    Descriptor(Box<RefinedForwardingDescriptorErrorV1>),
    History(Box<RefinedForwardingHistoryErrorV1>),
    Mismatch(&'static str),
    Worker(Box<worker::RefinedForwardingWorkerErrorV1>),
    BoundedUnroll(loop_unroll_native_v1::LoopUnrollNativeStageErrorV1),
    ConditionalPacket(Box<crate::production_native_source_lineage_v1::ConditionalPacketErrorV2>),
}
impl fmt::Display for RefinedForwardingNativeStageErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for RefinedForwardingNativeStageErrorV1 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Admission(error) => Some(error.as_ref()),
            Self::Descriptor(error) => Some(error.as_ref()),
            Self::History(error) => Some(error.as_ref()),
            Self::Worker(error) => Some(error.as_ref()),
            Self::BoundedUnroll(error) => Some(error),
            Self::ConditionalPacket(error) => Some(error.as_ref()),
            _ => None,
        }
    }
}
fn error(value: RefinedForwardingNativeStageErrorV1) -> ProductionPipelineError {
    ProductionPipelineError::InductionRefinementNativeStage(
        InductionRefinementNativeStageErrorV1::ForwardingComposition(value),
    )
}
fn resource(value: Resource) -> ProductionPipelineError {
    error(RefinedForwardingNativeStageErrorV1::Resource(value))
}
impl ProductionPipelineError {
    pub(in crate::production_pipeline) fn conditional_packet_v2(
        value: crate::production_native_source_lineage_v1::ConditionalPacketErrorV2,
    ) -> Self {
        error(RefinedForwardingNativeStageErrorV1::ConditionalPacket(
            Box::new(value),
        ))
    }
}
fn mismatch(detail: &'static str) -> ProductionPipelineError {
    error(RefinedForwardingNativeStageErrorV1::Mismatch(detail))
}
fn admission(value: CompositionAdmissionError) -> ProductionPipelineError {
    error(RefinedForwardingNativeStageErrorV1::Admission(Box::new(
        value,
    )))
}

#[allow(
    clippy::large_enum_variant,
    reason = "one consumed source-bearing owner"
)]
enum Composed {
    Direct(DirectComposed),
    Erased(ErasedComposed),
}
impl Composed {
    fn output(&self) -> &Graph {
        match self {
            Self::Direct(value) => value.output(),
            Self::Erased(value) => value.output(),
        }
    }
    fn refinement_output(&self) -> &Graph {
        match self {
            Self::Direct(value) => value.prefix().output(),
            Self::Erased(value) => value.prefix().output(),
        }
    }
    fn licm_input(&self) -> &Graph {
        match self {
            Self::Direct(value) => value.prefix().prefix().output(),
            Self::Erased(value) => value.prefix().prefix().output(),
        }
    }
    fn historical_p8(&self) -> &Graph {
        match self {
            Self::Direct(value) => value.prefix().prefix().prefix().prefix().prefix().output(),
            Self::Erased(value) => value.prefix().prefix().prefix().prefix().prefix().output(),
        }
    }
    fn history(&self) -> Admitted7Ref<'_> {
        match self {
            Self::Direct(value) => {
                Admitted7Ref::Direct(value.prefix().prefix().prefix().prefix().prefix().prefix())
            }
            Self::Erased(value) => {
                Admitted7Ref::Erased(value.prefix().prefix().prefix().prefix().prefix().prefix())
            }
        }
    }
    fn original(&self) -> Result<&Graph> {
        match self {
            Self::Direct(value) => value
                .prefix()
                .prefix()
                .prefix()
                .prefix()
                .prefix()
                .prefix()
                .prefix()
                .source_semantic_kir()
                .pre_ranked_executable()
                .ok_or_else(|| mismatch("original direct refined-forwarding source N")),
            Self::Erased(value) => Ok(value
                .prefix()
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
    fn refinement_origins(&self) -> &[SourceOrigin] {
        match self {
            Self::Direct(value) => value.refinement_origins(),
            Self::Erased(value) => value.refinement_origins(),
        }
    }
    fn origins(&self) -> &[FinalOrigin] {
        match self {
            Self::Direct(value) => value.origins(),
            Self::Erased(value) => value.origins(),
        }
    }
    fn refinement_limits(&self) -> Limits {
        match self {
            Self::Direct(value) => value.refinement_limits(),
            Self::Erased(value) => value.refinement_limits(),
        }
    }
    fn limits(&self) -> ForwardingLimits {
        match self {
            Self::Direct(value) => value.limits(),
            Self::Erased(value) => value.limits(),
        }
    }
    fn header(&self) -> usize {
        match self {
            Self::Direct(_) => size_of::<DirectComposed>(),
            Self::Erased(_) => size_of::<ErasedComposed>(),
        }
    }
    fn replay(&self, budget: &mut Budget<'_>) -> Result<()> {
        match self {
            Self::Direct(value) => value.verify_equivalence(budget),
            Self::Erased(value) => value.verify_equivalence(budget),
        }
        .map_err(admission)
    }
    fn continue_once(
        refined: Refined,
        limits: ForwardingLimits,
        budget: &mut Budget<'_>,
    ) -> Result<(Self, usize)> {
        let (owner, receipt) = match refined {
            Refined::Direct(value) => {
                let (owner, receipt) = value
                    .continue_cross_block_forwarding_v1(limits, budget)
                    .map_err(admission)?;
                (Self::Direct(owner), receipt)
            }
            Refined::Erased(value) => {
                let (owner, receipt) = value
                    .continue_cross_block_forwarding_v1(limits, budget)
                    .map_err(admission)?;
                (Self::Erased(owner), receipt)
            }
        };
        budget
            .reserve_storage(receipt.retained_storage())
            .map_err(resource)?;
        Ok((owner, receipt.retained_storage()))
    }
}

/// Unreserved added ownership, not the inherited source or caller siblings.
#[derive(Clone, Copy)]
pub(crate) struct RefinedForwardingNativeStorageV1(usize);
impl RefinedForwardingNativeStorageV1 {
    pub(crate) const fn retained_storage(self) -> usize {
        self.0
    }
}
pub(crate) struct PreparedRefinedForwardingNativeOutputV1 {
    owner: Composed,
    prefix_execution: Policy7ExecutionWitnessV1,
    llvm: String,
    profile: Profile,
    retained_floor: usize,
}
impl PreparedRefinedForwardingNativeOutputV1 {
    pub(crate) fn output(&self) -> &Graph {
        self.owner.output()
    }
    pub(crate) fn refinement_output(&self) -> &Graph {
        self.owner.refinement_output()
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
    pub(crate) fn refinement_origins(&self) -> &[SourceOrigin] {
        self.owner.refinement_origins()
    }
    pub(crate) fn origins(&self) -> &[FinalOrigin] {
        self.owner.origins()
    }
    pub(crate) fn refinement_limits(&self) -> Limits {
        self.owner.refinement_limits()
    }
    pub(crate) fn limits(&self) -> ForwardingLimits {
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
        mismatch("exact final refined-forwarding native LLVM")
    })
}
// Transfer only the existing source owners and individual receipts. The fixed F
// caller keeps its original summation, header reservation and emission order.
fn prepare_refined_forwarding_source_prefix_v1(
    prefix: Prefix6,
    refinement_limits: Limits,
    forwarding_limits: ForwardingLimits,
    budget: &mut Budget<'_>,
) -> Result<(
    Composed,
    Policy7ExecutionWitnessV1,
    usize,
    usize,
    usize,
    usize,
    usize,
    usize,
)> {
    let (licm, prefix_execution, history_added, promoted_added, preheaders_added, licm_added) =
        prepare_licm_source_prefix_v1(prefix, budget)?;
    let (refined, refined_added) = Refined::continue_once(licm, refinement_limits, budget)?;
    let (owner, forwarded_added) = Composed::continue_once(refined, forwarding_limits, budget)?;
    Ok((
        owner,
        prefix_execution,
        history_added,
        promoted_added,
        preheaders_added,
        licm_added,
        refined_added,
        forwarded_added,
    ))
}
fn prepare(
    prefix: Prefix6,
    profile: Profile,
    refinement_limits: Limits,
    forwarding_limits: ForwardingLimits,
    budget: &mut Budget<'_>,
) -> Result<(
    PreparedRefinedForwardingNativeOutputV1,
    RefinedForwardingNativeStorageV1,
)> {
    let floor = budget.storage();
    scoped(prefix.minimum()?, budget, move |budget| {
        let (
            owner,
            prefix_execution,
            history_added,
            promoted_added,
            preheaders_added,
            licm_added,
            refined_added,
            forwarded_added,
        ) = prepare_refined_forwarding_source_prefix_v1(
            prefix,
            refinement_limits,
            forwarding_limits,
            budget,
        )?;
        let (llvm, native_storage) = lower_native(owner.output(), profile, budget)?;
        budget.reserve_storage(native_storage).map_err(resource)?;
        let header = size_of::<PreparedRefinedForwardingNativeOutputV1>()
            .checked_sub(owner.header())
            .and_then(|bytes| bytes.checked_sub(size_of::<Policy7ExecutionWitnessV1>()))
            .and_then(|bytes| bytes.checked_sub(size_of::<String>()))
            .ok_or_else(|| resource(Resource::Arithmetic))?;
        budget.reserve_storage(header).map_err(resource)?;
        let retained = history_added
            .checked_add(prefix_execution.retained_storage())
            .and_then(|bytes| bytes.checked_add(promoted_added))
            .and_then(|bytes| bytes.checked_add(preheaders_added))
            .and_then(|bytes| bytes.checked_add(licm_added))
            .and_then(|bytes| bytes.checked_add(refined_added))
            .and_then(|bytes| bytes.checked_add(forwarded_added))
            .and_then(|bytes| bytes.checked_add(native_storage))
            .and_then(|bytes| bytes.checked_add(header))
            .ok_or_else(|| resource(Resource::Arithmetic))?;
        let value = PreparedRefinedForwardingNativeOutputV1 {
            owner,
            prefix_execution,
            llvm,
            profile,
            retained_floor: floor
                .checked_add(retained)
                .ok_or_else(|| resource(Resource::Arithmetic))?,
        };
        value.verify_equivalence(budget)?;
        Ok((value, RefinedForwardingNativeStorageV1(retained)))
    })
}

/// One actual authenticated source/ranked entry and its final composed output.
pub(crate) struct RefinedForwardingNativeProductionCompilationV1 {
    native: PreparedRefinedForwardingNativeOutputV1,
    history: PreparedRefinedForwardingHistoryClaimsV1,
    ranked_verification:
        crate::production_ranked_projection_v1::AuthenticatedRankedVerificationRosterV1,
    bindings: AuthenticatedProductionBindings,
    retained_floor: usize,
}
impl RankedVerifiedProductionCompilation {
    /// Refines and forwards the same evolving graph before first LLVM emission.
    /// Both exact limits are retained. Reserve the returned added ownership.
    pub(crate) fn lower_refined_cross_block_forwarding_native_with_budget_v1(
        self,
        refinement_limits: Limits,
        forwarding_limits: ForwardingLimits,
        budget: &mut Budget<'_>,
    ) -> Result<(
        RefinedForwardingNativeProductionCompilationV1,
        RefinedForwardingNativeStorageV1,
    )> {
        if self.has_direct_conditional_roots_v2() {
            return Err(self.conditional_finalizer_refusal_v2(
                fe2o3_kernel_opt::CanonicalRefinedForwardingHistoryLimitsV1 {
                    refinement: refinement_limits,
                    forwarding: forwarding_limits,
                },
                budget,
            ));
        }
        let floor = budget.storage();
        scoped(floor, budget, move |budget| {
            let (prefix, ranked_verification, bindings) = self.prepare_native_prefix_v1(budget)?;
            let (native, receipt) = prepare(
                prefix,
                bindings.rustc_target.profile(),
                refinement_limits,
                forwarding_limits,
                budget,
            )?;
            budget
                .reserve_storage(receipt.retained_storage())
                .map_err(resource)?;
            let history = PreparedRefinedForwardingHistoryClaimsV1::prepare(&native, budget)?;
            budget
                .reserve_storage(history.retained_storage())
                .map_err(resource)?;
            let wrapper = size_of::<RefinedForwardingNativeProductionCompilationV1>()
                .checked_sub(size_of::<PreparedRefinedForwardingNativeOutputV1>())
                .and_then(|bytes| {
                    bytes.checked_sub(size_of::<PreparedRefinedForwardingHistoryClaimsV1>())
                })
                .ok_or_else(|| resource(Resource::Arithmetic))?;
            budget.reserve_storage(wrapper).map_err(resource)?;
            let value = RefinedForwardingNativeProductionCompilationV1 {
                native,
                history,
                ranked_verification,
                bindings,
                retained_floor: budget.storage(),
            };
            value.verify_equivalence(budget)?;
            let retained = budget
                .storage()
                .checked_sub(floor)
                .ok_or_else(|| resource(Resource::Accounting))?;
            Ok((value, RefinedForwardingNativeStorageV1(retained)))
        })
    }
}
impl RefinedForwardingNativeProductionCompilationV1 {
    pub(crate) fn output(&self) -> &Graph {
        self.native.output()
    }
    pub(crate) fn refinement_output(&self) -> &Graph {
        self.native.refinement_output()
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
    pub(crate) fn refinement_origins(&self) -> &[SourceOrigin] {
        self.native.refinement_origins()
    }
    pub(crate) fn origins(&self) -> &[FinalOrigin] {
        self.native.origins()
    }
    pub(crate) fn refinement_limits(&self) -> Limits {
        self.native.refinement_limits()
    }
    pub(crate) fn limits(&self) -> ForwardingLimits {
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
                    "complete refined-forwarding target/ranked custody",
                ));
            }
            let checked = self.history.check(&self.native, budget)?;
            let history_storage = checked.retained_storage();
            budget.reserve_storage(history_storage).map_err(resource)?;
            drop(checked);
            budget.release_storage(history_storage).map_err(resource)?;
            let owner = match &self.native.owner {
                Composed::Direct(owner) => FinalOwnerV1::Direct(owner),
                Composed::Erased(owner) => FinalOwnerV1::Erased(owner),
            };
            validate_final_descriptor_evidence_v1(
                owner,
                &self.bindings.typed_descriptor_roots,
                self.native.profile,
                budget,
            )
            .map_err(|value| {
                error(RefinedForwardingNativeStageErrorV1::Descriptor(Box::new(
                    value,
                )))
            })?;
            self.native.verify_equivalence(budget)
        })
    }
}

#[cfg(test)]
#[path = "production_pipeline_refined_forwarding_native_v1_tests.rs"]
mod tests;

#[cfg(test)]
mod source_lane_tests {
    use super::*;
    use crate::production_pipeline::checked_output_policy7_v1::CheckedOutputPolicy7StageErrorV1 as P7Error;
    use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
    use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};

    pub(crate) struct RefinedForwardingSourceObservationV1<'a> {
        pub(crate) original: &'a Graph,
        pub(crate) historical: &'a Graph,
        pub(crate) promoted: &'a Graph,
        pub(crate) preheaders: &'a Graph,
        pub(crate) licm: &'a Graph,
        pub(crate) refined: &'a Graph,
        pub(crate) final_graph: &'a Graph,
        pub(crate) licm_origins: &'a [fe2o3_kernel_analysis::CanonicalKirLicmOriginV1],
        pub(crate) refinement_origins: &'a [SourceOrigin],
        pub(crate) forwarding_origins: &'a [FinalOrigin],
        pub(crate) licm_kernels: &'a [fe2o3_kernel_ir::FormalMemoryObligations],
        pub(crate) final_kernels: &'a [fe2o3_kernel_ir::FormalMemoryObligations],
        pub(crate) launch: &'a fe2o3_lower_mir_kernel::ProductionSourceLaunchRosterV1,
        pub(crate) semantic: &'a fe2o3_mir_model::semantic_mir_v1::AdmittedInertSemanticMirV1,
        pub(crate) profile: Profile,
        pub(crate) unit_local: bool,
        pub(crate) deleted_helpers: (usize, usize),
        pub(crate) source_identity: [u8; 32],
        pub(crate) preflight_identity: [u8; 32],
        pub(crate) ranked_identity: [u8; 32],
        pub(crate) execution: &'a [u8],
    }
    impl RefinedForwardingNativeProductionCompilationV1 {
        pub(crate) fn with_source_test_observation_v1<R>(
            &self,
            next: impl for<'a> FnOnce(RefinedForwardingSourceObservationV1<'a>) -> R,
        ) -> R {
            macro_rules! observe {
                ($value:expr, $launch:expr, $semantic:expr, $erased:expr, $deleted:expr) => {{
                    let value = $value;
                    let licm = value.prefix().prefix();
                    next(RefinedForwardingSourceObservationV1 {
                        original: self.original().unwrap(),
                        historical: self.historical_p8_output(),
                        promoted: licm.prefix().prefix().output(),
                        preheaders: licm.prefix().output(),
                        licm: self.licm_input(),
                        refined: self.refinement_output(),
                        final_graph: self.output(),
                        licm_origins: licm.operation_origins(),
                        refinement_origins: self.refinement_origins(),
                        forwarding_origins: self.origins(),
                        licm_kernels: licm.kernels(),
                        final_kernels: value.kernels(),
                        launch: $launch,
                        semantic: $semantic,
                        profile: self.bindings.rustc_target.profile(),
                        unit_local: $erased,
                        deleted_helpers: $deleted,
                        source_identity: self.bindings.rustc_identity_inventory.sha256(),
                        preflight_identity: self
                            .bindings
                            .rustc_preflight_plan
                            .rustc_identity_inventory_sha256(),
                        ranked_identity: *self
                            .ranked_verification
                            .canonical_roster_identity()
                            .as_bytes(),
                        execution: self.native.prefix_execution.canonical_bytes(),
                    })
                }};
            }
            match &self.native.owner {
                Composed::Direct(value) => {
                    let licm = value.prefix().prefix();
                    let source = licm
                        .prefix()
                        .prefix()
                        .prefix()
                        .prefix()
                        .prefix()
                        .source_semantic_kir();
                    observe!(
                        value,
                        source.source_launch_roster().unwrap(),
                        source.semantic().semantic(),
                        false,
                        (0, 0)
                    )
                }
                Composed::Erased(value) => {
                    let licm = value.prefix().prefix();
                    let source = licm.prefix().prefix().prefix().prefix().prefix();
                    observe!(
                        value,
                        source.original_source().source_launch(),
                        source.original_source().semantic_ssa().source_semantic(),
                        true,
                        (
                            source.erased_source().deleted_call_count(),
                            source.erased_source().deleted_function_count()
                        )
                    )
                }
            }
        }

        pub(crate) fn source_test_restoring_faults_v1(&mut self, budget: &mut Budget<'_>) {
            fn restored<T>(result: std::thread::Result<T>) -> T {
                match result {
                    Ok(value) => value,
                    Err(payload) => resume_unwind(payload),
                }
            }
            let floor = budget.storage();
            let ledger = budget.work_ledger_identity_v1();
            let byte = self.native.llvm.as_bytes()[0];
            assert!(byte.is_ascii());
            self.native
                .llvm
                .replace_range(..1, if byte == b'X' { "Y" } else { "X" });
            let result = catch_unwind(AssertUnwindSafe(|| self.verify_equivalence(budget)));
            self.native
                .llvm
                .replace_range(..1, std::str::from_utf8(&[byte]).unwrap());
            assert!(matches!(
                restored(result),
                Err(ProductionPipelineError::InductionRefinementNativeStage(
                    InductionRefinementNativeStageErrorV1::ForwardingComposition(
                        RefinedForwardingNativeStageErrorV1::Mismatch(
                            "exact final refined-forwarding native LLVM"
                        )
                    )
                ))
            ));
            self.verify_equivalence(budget).unwrap();

            let profile = self.native.profile;
            self.native.profile = match profile {
                Profile::Gfx942 => Profile::Gfx950,
                Profile::Gfx950 => Profile::Gfx942,
            };
            let result = catch_unwind(AssertUnwindSafe(|| self.verify_equivalence(budget)));
            self.native.profile = profile;
            assert!(matches!(
                restored(result),
                Err(ProductionPipelineError::InductionRefinementNativeStage(
                    InductionRefinementNativeStageErrorV1::ForwardingComposition(
                        RefinedForwardingNativeStageErrorV1::Mismatch(
                            "complete refined-forwarding target/ranked custody"
                        )
                    )
                ))
            ));
            self.verify_equivalence(budget).unwrap();

            budget.release_storage(1).unwrap();
            let result = catch_unwind(AssertUnwindSafe(|| self.verify_equivalence(budget)));
            budget.reserve_storage(1).unwrap();
            assert!(matches!(
                restored(result),
                Err(ProductionPipelineError::CheckedOutputPolicy7Stage(
                    P7Error::Resource(Resource::Accounting)
                ))
            ));
            self.verify_equivalence(budget).unwrap();
            assert_eq!(budget.storage(), floor);
            assert!(budget.work_ledger_identity_v1() == ledger);
        }

        pub(crate) fn source_test_exact_replay_work_v1(
            &self,
            work_cap: usize,
            storage_cap: usize,
        ) -> [usize; 3] {
            let measure = |work_limit, storage_limit| {
                let mut work = Work::new(work_limit);
                work.charge_work(17).unwrap();
                let (result, accepted, peak, failed_storage) = {
                    let mut budget = Budget::new(&mut work, storage_limit);
                    budget.reserve_storage(self.retained_floor).unwrap();
                    let ledger = budget.work_ledger_identity_v1();
                    let result = self.verify_equivalence(&mut budget);
                    assert_eq!(budget.storage(), self.retained_floor);
                    assert!(budget.work_ledger_identity_v1() == ledger);
                    (
                        result,
                        budget.work(),
                        budget.peak_storage(),
                        budget.failed_storage(),
                    )
                };
                (result, accepted, peak, work.failed_work(), failed_storage)
            };
            let full = measure(work_cap, storage_cap);
            full.0.unwrap();
            assert_eq!((full.3, full.4), (None, None));
            let exact = measure(full.1, full.2);
            exact.0.unwrap();
            assert_eq!(
                (exact.1, exact.2, exact.3, exact.4),
                (full.1, full.2, None, None)
            );
            let short = measure(full.1 - 1, full.2);
            let Err(ProductionPipelineError::InductionRefinementNativeStage(
                InductionRefinementNativeStageErrorV1::ForwardingComposition(
                    RefinedForwardingNativeStageErrorV1::Resource(Resource::Work(error)),
                ),
            )) = short.0
            else {
                panic!("exact final combined native replay Work phase")
            };
            assert_eq!((error.actual(), error.limit()), (full.1, full.1 - 1));
            let final_charge = self
                .llvm_ir()
                .len()
                .checked_mul(2)
                .unwrap()
                .checked_add(1)
                .unwrap();
            assert_eq!(
                (short.1, short.2, short.3, short.4),
                (full.1 - final_charge, full.2, Some(full.1), None)
            );
            [full.1, full.2, short.1]
        }
    }
}
