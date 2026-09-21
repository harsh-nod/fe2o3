//! Explicit unnumbered source-owned neutral preheaders through native lowering.
//! Only the final actual owner is emitted. Historical P8 and promoted inputs
//! remain distinct; no descriptor, handoff, default or publication is added.

use super::*;
use fe2o3_lower_mir_kernel::{
    ProductionLoopPreheadersErrorV1 as AdmissionError,
    ProductionOwnedLoopPreheadersContinuationV1 as DirectPreheaders,
    ProductionOwnedUnitLocalLoopPreheadersContinuationV1 as ErasedPreheaders,
};

#[path = "production_pipeline_licm_native_v1.rs"]
mod licm_native_v1;
pub(crate) use licm_native_v1::LicmNativeStageErrorV1;

#[derive(Debug)]
pub(crate) enum LoopPreheadersNativeStageErrorV1 {
    Resource(Resource),
    Admission(Box<AdmissionError>),
    Mismatch(&'static str),
}
impl fmt::Display for LoopPreheadersNativeStageErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for LoopPreheadersNativeStageErrorV1 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Admission(error) => Some(error.as_ref()),
            _ => None,
        }
    }
}
fn error(value: LoopPreheadersNativeStageErrorV1) -> ProductionPipelineError {
    ProductionPipelineError::LoopPreheadersNativeStage(value)
}
fn resource(value: Resource) -> ProductionPipelineError {
    error(LoopPreheadersNativeStageErrorV1::Resource(value))
}
fn mismatch(detail: &'static str) -> ProductionPipelineError {
    error(LoopPreheadersNativeStageErrorV1::Mismatch(detail))
}
fn admission(value: AdmissionError) -> ProductionPipelineError {
    error(LoopPreheadersNativeStageErrorV1::Admission(Box::new(value)))
}

#[allow(
    clippy::large_enum_variant,
    reason = "one consumed source-bearing owner"
)]
enum Preheaders {
    Direct(DirectPreheaders),
    Erased(ErasedPreheaders),
}
impl Preheaders {
    fn output(&self) -> &Graph {
        match self {
            Self::Direct(v) => v.output(),
            Self::Erased(v) => v.output(),
        }
    }
    fn promoted_input(&self) -> &Graph {
        match self {
            Self::Direct(v) => v.prefix().output(),
            Self::Erased(v) => v.prefix().output(),
        }
    }
    fn historical_p8(&self) -> &Graph {
        match self {
            Self::Direct(v) => v.prefix().prefix().output(),
            Self::Erased(v) => v.prefix().prefix().output(),
        }
    }
    fn history(&self) -> Admitted7Ref<'_> {
        match self {
            Self::Direct(v) => Admitted7Ref::Direct(v.prefix().prefix().prefix()),
            Self::Erased(v) => Admitted7Ref::Erased(v.prefix().prefix().prefix()),
        }
    }
    fn original(&self) -> Result<&Graph> {
        match self {
            Self::Direct(v) => v
                .prefix()
                .prefix()
                .prefix()
                .prefix()
                .source_semantic_kir()
                .pre_ranked_executable()
                .ok_or_else(|| mismatch("original direct source N")),
            Self::Erased(v) => Ok(v
                .prefix()
                .prefix()
                .prefix()
                .prefix()
                .original_source()
                .executable()),
        }
    }
    fn header(&self) -> usize {
        match self {
            Self::Direct(_) => size_of::<DirectPreheaders>(),
            Self::Erased(_) => size_of::<ErasedPreheaders>(),
        }
    }
    fn replay(&self, budget: &mut Budget<'_>) -> Result<()> {
        match self {
            Self::Direct(v) => v.verify_equivalence(budget),
            Self::Erased(v) => v.verify_equivalence(budget),
        }
        .map_err(admission)
    }
    fn continue_once(promoted: Promoted, budget: &mut Budget<'_>) -> Result<(Self, usize)> {
        let (owner, storage) = match promoted {
            Promoted::Direct(v) => {
                let (owner, storage) = v.continue_loop_preheaders_v1(budget).map_err(admission)?;
                (Self::Direct(owner), storage)
            }
            Promoted::Erased(v) => {
                let (owner, storage) = v.continue_loop_preheaders_v1(budget).map_err(admission)?;
                (Self::Erased(owner), storage)
            }
        };
        budget
            .reserve_storage(storage.retained_storage())
            .map_err(resource)?;
        Ok((owner, storage.retained_storage()))
    }
}

/// Added storage returned unreserved, excluding retained source and siblings.
#[derive(Clone, Copy)]
pub(crate) struct LoopPreheadersNativeStorageV1(usize);
impl LoopPreheadersNativeStorageV1 {
    pub(crate) const fn retained_storage(self) -> usize {
        self.0
    }
}

pub(crate) struct PreparedLoopPreheadersNativeOutputV1 {
    owner: Preheaders,
    prefix_execution: Policy7ExecutionWitnessV1,
    llvm: String,
    profile: Profile,
    retained_floor: usize,
}
impl PreparedLoopPreheadersNativeOutputV1 {
    pub(crate) fn output(&self) -> &Graph {
        self.owner.output()
    }
    pub(crate) fn promoted_input(&self) -> &Graph {
        self.owner.promoted_input()
    }
    pub(crate) fn historical_p8_output(&self) -> &Graph {
        self.owner.historical_p8()
    }
    pub(crate) fn original(&self) -> Result<&Graph> {
        self.owner.original()
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
        mismatch("exact loop-preheaders native LLVM")
    })
}

fn check_native_text_with_errors_v1(
    output: &Graph,
    profile: Profile,
    actual: &str,
    budget: &mut Budget<'_>,
    resource_error: fn(Resource) -> ProductionPipelineError,
    text_mismatch: fn() -> ProductionPipelineError,
) -> Result<()> {
    scoped(budget.storage(), budget, |budget| {
        let (expected, storage) = lower_native(output, profile, budget)?;
        budget.reserve_storage(storage).map_err(resource_error)?;
        budget
            .charge_work(
                expected
                    .len()
                    .checked_add(actual.len())
                    .and_then(|n| n.checked_add(1))
                    .ok_or_else(|| resource_error(Resource::Arithmetic))?,
            )
            .map_err(resource_error)?;
        if expected != actual {
            return Err(text_mismatch());
        }
        Ok(())
    })
}

fn prepare(
    prefix: Prefix6,
    profile: Profile,
    budget: &mut Budget<'_>,
) -> Result<(
    PreparedLoopPreheadersNativeOutputV1,
    LoopPreheadersNativeStorageV1,
)> {
    let floor = budget.storage();
    scoped(prefix.minimum()?, budget, move |budget| {
        let (history, prefix_execution, history_added) = prepare_history_v1(prefix, budget)?;
        let (promoted, promoted_added) = Promoted::continue_once(history, budget)?;
        let (owner, preheaders_added) = Preheaders::continue_once(promoted, budget)?;
        let (llvm, native_storage) = lower_native(owner.output(), profile, budget)?;
        budget.reserve_storage(native_storage).map_err(resource)?;
        // The genuine owner, witness and String receipts already paid these headers.
        let header = size_of::<PreparedLoopPreheadersNativeOutputV1>()
            .checked_sub(owner.header())
            .and_then(|n| n.checked_sub(size_of::<Policy7ExecutionWitnessV1>()))
            .and_then(|n| n.checked_sub(size_of::<String>()))
            .ok_or_else(|| resource(Resource::Arithmetic))?;
        budget.reserve_storage(header).map_err(resource)?;
        let retained = history_added
            .checked_add(prefix_execution.retained_storage())
            .and_then(|n| n.checked_add(promoted_added))
            .and_then(|n| n.checked_add(preheaders_added))
            .and_then(|n| n.checked_add(native_storage))
            .and_then(|n| n.checked_add(header))
            .ok_or_else(|| resource(Resource::Arithmetic))?;
        let value = PreparedLoopPreheadersNativeOutputV1 {
            owner,
            prefix_execution,
            llvm,
            profile,
            retained_floor: floor
                .checked_add(retained)
                .ok_or_else(|| resource(Resource::Arithmetic))?,
        };
        value.verify_equivalence(budget)?;
        Ok((value, LoopPreheadersNativeStorageV1(retained)))
    })
}

/// Actual source/ranked custody and final native text, without wire authority.
pub(crate) struct LoopPreheadersNativeProductionCompilationV1 {
    native: PreparedLoopPreheadersNativeOutputV1,
    ranked_verification:
        crate::production_ranked_projection_v1::AuthenticatedRankedVerificationRosterV1,
    bindings: AuthenticatedProductionBindings,
    retained_floor: usize,
}
impl RankedVerifiedProductionCompilation {
    /// Explicit unnumbered continuation. The complete returned receipt is not
    /// reserved; callers retain their original floor and must reserve the addition.
    pub(crate) fn lower_loop_preheaders_native_with_budget_v1(
        self,
        budget: &mut Budget<'_>,
    ) -> Result<(
        LoopPreheadersNativeProductionCompilationV1,
        LoopPreheadersNativeStorageV1,
    )> {
        let floor = budget.storage();
        scoped(floor, budget, move |budget| {
            let (prefix, ranked_verification, bindings) = self.prepare_native_prefix_v1(budget)?;
            let (native, storage) = prepare(prefix, bindings.rustc_target.profile(), budget)?;
            budget
                .reserve_storage(storage.retained_storage())
                .map_err(resource)?;
            let wrapper = size_of::<LoopPreheadersNativeProductionCompilationV1>()
                .checked_sub(size_of::<PreparedLoopPreheadersNativeOutputV1>())
                .ok_or_else(|| resource(Resource::Arithmetic))?;
            budget.reserve_storage(wrapper).map_err(resource)?;
            let value = LoopPreheadersNativeProductionCompilationV1 {
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
            Ok((value, LoopPreheadersNativeStorageV1(retained)))
        })
    }
}
impl LoopPreheadersNativeProductionCompilationV1 {
    pub(crate) fn output(&self) -> &Graph {
        self.native.output()
    }
    pub(crate) fn promoted_input(&self) -> &Graph {
        self.native.promoted_input()
    }
    pub(crate) fn historical_p8_output(&self) -> &Graph {
        self.native.historical_p8_output()
    }
    pub(crate) fn llvm_ir(&self) -> &str {
        self.native.llvm_ir()
    }
    pub(crate) fn original(&self) -> Result<&Graph> {
        self.native.original()
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
                return Err(mismatch("complete loop-preheaders target/ranked custody"));
            }
            self.native.verify_equivalence(budget)
        })
    }
}

#[cfg(test)]
#[path = "production_pipeline_loop_preheaders_native_v1_tests.rs"]
mod tests;
