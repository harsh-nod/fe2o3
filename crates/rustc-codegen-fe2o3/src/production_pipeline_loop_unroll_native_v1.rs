//! Actual source F-to-U before the first LLVM emission. No wire/default authority.
use super::*;
#[path = "production_pipeline_expanded_native_v3.rs"]
pub(crate) mod expanded_v3;
#[path = "production_pipeline_nominal_loop_unroll_native_v3.rs"]
pub(crate) mod nominal_v3;
use super::history::ActualFinalFSourceRefV1 as FinalSource;
use crate::compiler_descriptor::checked_output_policy3_v1::refined_forwarding_v1::loop_unroll_v1::{
    LoopUnrollDescriptorErrorV1, UnrolledOwnerV1, validate_unrolled_descriptor_evidence_v1,
};
use fe2o3_kernel_analysis::CanonicalKirLoopUnrollLimitsV1 as UnrollLimits;
use fe2o3_lower_mir_kernel::{
    ProductionLoopUnrollErrorV1 as UnrollAdmission,
    ProductionLoopUnrollOriginV1 as UnrollOrigin,
    ProductionOwnedLoopUnrollContinuationV1 as DirectU,
    ProductionOwnedUnitLocalLoopUnrollContinuationV1 as ErasedU,
};

#[derive(Debug)]
pub(crate) enum LoopUnrollNativeStageErrorV1 {
    Resource(Resource),
    Admission(Box<UnrollAdmission>),
    Descriptor(Box<LoopUnrollDescriptorErrorV1>),
    Mismatch(&'static str),
}
impl fmt::Display for LoopUnrollNativeStageErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for LoopUnrollNativeStageErrorV1 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Admission(e) => Some(e.as_ref()),
            Self::Descriptor(e) => Some(e.as_ref()),
            Self::Resource(e) => Some(e),
            Self::Mismatch(_) => None,
        }
    }
}
fn error(e: LoopUnrollNativeStageErrorV1) -> ProductionPipelineError {
    super::error(RefinedForwardingNativeStageErrorV1::BoundedUnroll(e))
}
fn resource(e: Resource) -> ProductionPipelineError {
    error(LoopUnrollNativeStageErrorV1::Resource(e))
}
fn mismatch(s: &'static str) -> ProductionPipelineError {
    error(LoopUnrollNativeStageErrorV1::Mismatch(s))
}
fn admission(e: UnrollAdmission) -> ProductionPipelineError {
    error(LoopUnrollNativeStageErrorV1::Admission(Box::new(e)))
}

#[allow(
    clippy::large_enum_variant,
    reason = "one consumed genuine source owner, not duplicated or boxed"
)]
enum Unrolled {
    Direct(DirectU),
    Erased(ErasedU),
}
impl Unrolled {
    fn output(&self) -> &Graph {
        match self {
            Self::Direct(v) => v.output(),
            Self::Erased(v) => v.output(),
        }
    }
    fn source(&self) -> FinalSource<'_> {
        match self {
            Self::Direct(v) => FinalSource::Direct(v.prefix()),
            Self::Erased(v) => FinalSource::Erased(v.prefix()),
        }
    }
    fn descriptor(&self) -> UnrolledOwnerV1<'_> {
        match self {
            Self::Direct(v) => UnrolledOwnerV1::Direct(v),
            Self::Erased(v) => UnrolledOwnerV1::Erased(v),
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
                .prefix()
                .prefix()
                .source_semantic_kir()
                .pre_ranked_executable()
                .ok_or_else(|| mismatch("original U direct source N")),
            Self::Erased(v) => Ok(v
                .prefix()
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
    fn origins(&self) -> &[UnrollOrigin] {
        match self {
            Self::Direct(v) => v.origins(),
            Self::Erased(v) => v.origins(),
        }
    }
    fn limits(&self) -> UnrollLimits {
        match self {
            Self::Direct(v) => v.limits(),
            Self::Erased(v) => v.limits(),
        }
    }
    fn replay(&self, budget: &mut Budget<'_>) -> Result<()> {
        match self {
            Self::Direct(v) => v.verify_equivalence(budget),
            Self::Erased(v) => v.verify_equivalence(budget),
        }
        .map_err(admission)
    }
    fn active_header(erased: bool) -> usize {
        if erased {
            size_of::<ErasedU>()
        } else {
            size_of::<DirectU>()
        }
    }
    fn continue_once(
        owner: Composed,
        limits: UnrollLimits,
        budget: &mut Budget<'_>,
    ) -> Result<(Self, usize)> {
        let (owner, receipt) = match owner {
            Composed::Direct(v) => {
                let (v, r) = v
                    .continue_bounded_loop_unroll_v1(limits, budget)
                    .map_err(admission)?;
                (Self::Direct(v), r)
            }
            Composed::Erased(v) => {
                let (v, r) = v
                    .continue_bounded_loop_unroll_v1(limits, budget)
                    .map_err(admission)?;
                (Self::Erased(v), r)
            }
        };
        budget
            .reserve_storage(receipt.retained_storage())
            .map_err(resource)?;
        Ok((owner, receipt.retained_storage()))
    }
}

/// Unreserved addition; inherited source, bindings and siblings remain paid.
#[derive(Clone, Copy)]
pub(crate) struct LoopUnrollNativeStorageV1(usize);
impl LoopUnrollNativeStorageV1 {
    pub(crate) const fn retained_storage(self) -> usize {
        self.0
    }
}

pub(crate) struct PreparedLoopUnrollNativeOutputV1 {
    owner: Unrolled,
    prefix_execution: Policy7ExecutionWitnessV1,
    llvm: String,
    profile: Profile,
    retained_floor: usize,
}

/// Move-only actual source/P7/F/U custody, before any native emission.
/// The returned addition is unreserved; the original prefix receipt transfers.
struct PreparedLoopUnrollSourceSeedV1 {
    owner: Unrolled,
    prefix_execution: Policy7ExecutionWitnessV1,
    profile: Profile,
    retained_floor: usize,
}
fn source_seed_header(erased: bool) -> Result<usize> {
    size_of::<PreparedLoopUnrollSourceSeedV1>()
        .checked_sub(Unrolled::active_header(erased))
        .and_then(|n| n.checked_sub(size_of::<Policy7ExecutionWitnessV1>()))
        .ok_or_else(|| resource(Resource::Arithmetic))
}
fn prepared_header(erased: bool) -> Result<usize> {
    size_of::<PreparedLoopUnrollNativeOutputV1>()
        .checked_sub(Unrolled::active_header(erased))
        .and_then(|n| n.checked_sub(size_of::<Policy7ExecutionWitnessV1>()))
        .and_then(|n| n.checked_sub(size_of::<String>()))
        .ok_or_else(|| resource(Resource::Arithmetic))
}
fn composed_header(erased: bool) -> Result<usize> {
    size_of::<Composed>()
        .checked_sub(if erased {
            size_of::<ErasedComposed>()
        } else {
            size_of::<DirectComposed>()
        })
        .ok_or_else(|| resource(Resource::Arithmetic))
}
impl PreparedLoopUnrollNativeOutputV1 {
    pub(crate) fn output(&self) -> &Graph {
        self.owner.output()
    }
    pub(crate) fn forwarding_output(&self) -> &Graph {
        self.owner.source().output()
    }
    pub(crate) fn original(&self) -> Result<&Graph> {
        self.owner.original()
    }
    pub(crate) fn origins(&self) -> &[UnrollOrigin] {
        self.owner.origins()
    }
    pub(crate) fn limits(&self) -> UnrollLimits {
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
            let scratch = fe2o3_kernel_opt::POLICY7_EXECUTION_HEADER_BYTES_V1
                .checked_add(size_of::<FinalSource<'_>>())
                .ok_or_else(|| resource(Resource::Arithmetic))?;
            budget.reserve_storage(scratch).map_err(resource)?;
            self.prefix_execution
                .check_history_v1(self.owner.source().history(), budget)?;
            budget.release_storage(scratch).map_err(resource)?;
            check_native_text_with_errors_v1(
                self.output(),
                self.profile,
                &self.llvm,
                budget,
                resource,
                || mismatch("exact actual U native LLVM"),
            )?;
            budget.charge_work(1).map_err(resource)
        })
    }
}
fn prepare_source_seed_v1(
    prefix: Prefix6,
    profile: Profile,
    refinement: Limits,
    forwarding: ForwardingLimits,
    unroll: UnrollLimits,
    budget: &mut Budget<'_>,
) -> Result<(PreparedLoopUnrollSourceSeedV1, LoopUnrollNativeStorageV1)> {
    let floor = budget.storage();
    let erased = matches!(&prefix, Prefix6::Erased(_));
    scoped(prefix.minimum()?, budget, move |budget| {
        budget.charge_work(3).map_err(resource)?;
        // Pay both the final enum/header slack and transient Composed slack
        // before those values coexist with any controlled source/native work.
        let header = source_seed_header(erased)?;
        budget.reserve_storage(header).map_err(resource)?;
        let transient = composed_header(erased)?;
        budget.reserve_storage(transient).map_err(resource)?;
        let (
            f,
            prefix_execution,
            history_added,
            promoted_added,
            preheaders_added,
            licm_added,
            refined_added,
            forwarded_added,
        ) = prepare_refined_forwarding_source_prefix_v1(prefix, refinement, forwarding, budget)?;
        let (owner, unrolled_added) = Unrolled::continue_once(f, unroll, budget)?;
        budget.release_storage(transient).map_err(resource)?;
        let retained = [
            history_added,
            prefix_execution.retained_storage(),
            promoted_added,
            preheaders_added,
            licm_added,
            refined_added,
            forwarded_added,
            unrolled_added,
            header,
        ]
        .into_iter()
        .try_fold(0usize, |n, v| {
            n.checked_add(v)
                .ok_or_else(|| resource(Resource::Arithmetic))
        })?;
        let value = PreparedLoopUnrollSourceSeedV1 {
            owner,
            prefix_execution,
            profile,
            retained_floor: floor
                .checked_add(retained)
                .ok_or_else(|| resource(Resource::Arithmetic))?,
        };
        Ok((value, LoopUnrollNativeStorageV1(retained)))
    })
}

fn prepare(
    prefix: Prefix6,
    profile: Profile,
    refinement: Limits,
    forwarding: ForwardingLimits,
    unroll: UnrollLimits,
    budget: &mut Budget<'_>,
) -> Result<(PreparedLoopUnrollNativeOutputV1, LoopUnrollNativeStorageV1)> {
    let floor = budget.storage();
    let erased = matches!(&prefix, Prefix6::Erased(_));
    scoped(prefix.minimum()?, budget, move |budget| {
        let (seed, receipt) =
            prepare_source_seed_v1(prefix, profile, refinement, forwarding, unroll, budget)?;
        finish_source_seed_native_v1(seed, receipt, erased, floor, budget)
    })
}

// Keep native owning temporaries off the stack during the preceding source
// replay. The caller's original scope still owns every refund and unwind.
#[inline(never)]
fn finish_source_seed_native_v1(
    seed: PreparedLoopUnrollSourceSeedV1,
    receipt: LoopUnrollNativeStorageV1,
    erased: bool,
    floor: usize,
    budget: &mut Budget<'_>,
) -> Result<(PreparedLoopUnrollNativeOutputV1, LoopUnrollNativeStorageV1)> {
    budget
        .reserve_storage(receipt.retained_storage())
        .map_err(resource)?;
    if budget.storage() < seed.retained_floor {
        return Err(resource(Resource::Accounting));
    }
    let PreparedLoopUnrollSourceSeedV1 {
        owner,
        prefix_execution,
        profile,
        ..
    } = seed;
    // String storage pays its own header; only the replacement wrapper
    // slack is transferred here, after the string-free seed was consumed.
    let old_header = source_seed_header(erased)?;
    let new_header = prepared_header(erased)?;
    if new_header >= old_header {
        budget
            .reserve_storage(new_header - old_header)
            .map_err(resource)?;
    } else {
        budget
            .release_storage(old_header - new_header)
            .map_err(resource)?;
    }
    let (llvm, native_storage) = lower_native(owner.output(), profile, budget)?;
    budget.reserve_storage(native_storage).map_err(resource)?;
    let retained = receipt
        .retained_storage()
        .checked_sub(old_header)
        .and_then(|n| n.checked_add(new_header))
        .and_then(|n| n.checked_add(native_storage))
        .ok_or_else(|| resource(Resource::Arithmetic))?;
    let value = PreparedLoopUnrollNativeOutputV1 {
        owner,
        prefix_execution,
        llvm,
        profile,
        retained_floor: floor
            .checked_add(retained)
            .ok_or_else(|| resource(Resource::Arithmetic))?,
    };
    value.verify_equivalence(budget)?;
    budget.charge_work(1).map_err(resource)?;
    Ok((value, LoopUnrollNativeStorageV1(retained)))
}

/// Complete original authenticated custody plus separate F history and actual U.
/// This does not implement the F worker or wire interfaces.
pub(crate) struct LoopUnrollNativeProductionCompilationV1 {
    native: PreparedLoopUnrollNativeOutputV1,
    history: PreparedRefinedForwardingHistoryClaimsV1,
    ranked_verification:
        crate::production_ranked_projection_v1::AuthenticatedRankedVerificationRosterV1,
    bindings: AuthenticatedProductionBindings,
    retained_floor: usize,
}
impl RankedVerifiedProductionCompilation {
    pub(crate) fn lower_bounded_loop_unroll_native_with_budget_v1(
        self,
        refinement: Limits,
        forwarding: ForwardingLimits,
        unroll: UnrollLimits,
        budget: &mut Budget<'_>,
    ) -> Result<(
        LoopUnrollNativeProductionCompilationV1,
        LoopUnrollNativeStorageV1,
    )> {
        let floor = budget.storage();
        scoped(floor, budget, move |budget| {
            let wrapper = size_of::<LoopUnrollNativeProductionCompilationV1>()
                .checked_sub(size_of::<PreparedLoopUnrollNativeOutputV1>())
                .and_then(|n| n.checked_sub(size_of::<PreparedRefinedForwardingHistoryClaimsV1>()))
                .ok_or_else(|| resource(Resource::Arithmetic))?;
            budget.reserve_storage(wrapper).map_err(resource)?;
            let (prefix, ranked_verification, bindings) = self.prepare_native_prefix_v1(budget)?;
            let (native, receipt) = prepare(
                prefix,
                bindings.rustc_target.profile(),
                refinement,
                forwarding,
                unroll,
                budget,
            )?;
            budget
                .reserve_storage(receipt.retained_storage())
                .map_err(resource)?;
            let history = PreparedRefinedForwardingHistoryClaimsV1::prepare_source_v1(
                native.owner.source(),
                &native.prefix_execution,
                budget,
            )?;
            budget
                .reserve_storage(history.retained_storage())
                .map_err(resource)?;
            let value = LoopUnrollNativeProductionCompilationV1 {
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
            Ok((value, LoopUnrollNativeStorageV1(retained)))
        })
    }
}
impl LoopUnrollNativeProductionCompilationV1 {
    pub(crate) fn output(&self) -> &Graph {
        self.native.output()
    }
    pub(crate) fn forwarding_output(&self) -> &Graph {
        self.native.forwarding_output()
    }
    pub(crate) fn original(&self) -> Result<&Graph> {
        self.native.original()
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
                return Err(mismatch("complete U target/ranked custody"));
            }
            self.history.check_source_v1(
                self.native.owner.source(),
                &self.native.prefix_execution,
                budget,
            )?;
            validate_unrolled_descriptor_evidence_v1(
                self.native.owner.descriptor(),
                &self.bindings.typed_descriptor_roots,
                self.native.profile,
                budget,
            )
            .map_err(|e| error(LoopUnrollNativeStageErrorV1::Descriptor(Box::new(e))))?;
            self.native.verify_equivalence(budget)
        })
    }
}

#[cfg(test)]
#[path = "production_pipeline_loop_unroll_native_v1_tests.rs"]
mod tests;
