//! Actual source-owned private forwarding before the first LLVM emission.
use super::*;
use fe2o3_kernel_analysis::CanonicalKirCrossBlockForwardingLimitsV1 as Limits;
use fe2o3_lower_mir_kernel::{
    ProductionCrossBlockForwardingErrorV1 as ForwardingAdmissionError,
    ProductionCrossBlockForwardingOriginV1 as SourceOrigin,
    ProductionOwnedCrossBlockForwardingContinuationV1 as DirectForwarded,
    ProductionOwnedUnitLocalCrossBlockForwardingContinuationV1 as ErasedForwarded,
};

#[derive(Debug)]
pub(crate) enum CrossBlockForwardingNativeStageErrorV1 {
    Resource(Resource),
    Admission(Box<ForwardingAdmissionError>),
    Mismatch(&'static str),
}
impl fmt::Display for CrossBlockForwardingNativeStageErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for CrossBlockForwardingNativeStageErrorV1 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Admission(error) => Some(error.as_ref()),
            _ => None,
        }
    }
}
fn error(value: CrossBlockForwardingNativeStageErrorV1) -> ProductionPipelineError {
    ProductionPipelineError::LicmNativeStage(LicmNativeStageErrorV1::CrossBlockForwarding(value))
}
fn resource(value: Resource) -> ProductionPipelineError {
    error(CrossBlockForwardingNativeStageErrorV1::Resource(value))
}
fn mismatch(detail: &'static str) -> ProductionPipelineError {
    error(CrossBlockForwardingNativeStageErrorV1::Mismatch(detail))
}
fn admission(value: ForwardingAdmissionError) -> ProductionPipelineError {
    error(CrossBlockForwardingNativeStageErrorV1::Admission(Box::new(
        value,
    )))
}

#[allow(
    clippy::large_enum_variant,
    reason = "one consumed source-bearing owner"
)]
enum Forwarded {
    Direct(DirectForwarded),
    Erased(ErasedForwarded),
}
impl Forwarded {
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
                .ok_or_else(|| mismatch("original direct private-forwarding source N")),
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
            Self::Direct(_) => size_of::<DirectForwarded>(),
            Self::Erased(_) => size_of::<ErasedForwarded>(),
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
            Licm::Direct(value) => {
                let (owner, storage) = value
                    .continue_cross_block_forwarding_v1(limits, budget)
                    .map_err(admission)?;
                (Self::Direct(owner), storage)
            }
            Licm::Erased(value) => {
                let (owner, storage) = value
                    .continue_cross_block_forwarding_v1(limits, budget)
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

/// Unreserved new ownership, excluding the caller's actual source and siblings.
#[derive(Clone, Copy)]
pub(crate) struct CrossBlockForwardingNativeStorageV1(usize);
impl CrossBlockForwardingNativeStorageV1 {
    pub(crate) const fn retained_storage(self) -> usize {
        self.0
    }
}
pub(crate) struct PreparedCrossBlockForwardingNativeOutputV1 {
    owner: Forwarded,
    prefix_execution: Policy7ExecutionWitnessV1,
    llvm: String,
    profile: Profile,
    retained_floor: usize,
}
impl PreparedCrossBlockForwardingNativeOutputV1 {
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
        mismatch("exact cross-block private-forwarding native LLVM")
    })
}
fn prepare(
    prefix: Prefix6,
    profile: Profile,
    limits: Limits,
    budget: &mut Budget<'_>,
) -> Result<(
    PreparedCrossBlockForwardingNativeOutputV1,
    CrossBlockForwardingNativeStorageV1,
)> {
    let floor = budget.storage();
    scoped(prefix.minimum()?, budget, move |budget| {
        let (licm, prefix_execution, history_added, promoted_added, preheaders_added, licm_added) =
            prepare_licm_source_prefix_v1(prefix, budget)?;
        let (owner, forwarded_added) = Forwarded::continue_once(licm, limits, budget)?;
        let (llvm, native_storage) = lower_native(owner.output(), profile, budget)?;
        budget.reserve_storage(native_storage).map_err(resource)?;
        let header = size_of::<PreparedCrossBlockForwardingNativeOutputV1>()
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
            .and_then(|bytes| bytes.checked_add(forwarded_added))
            .and_then(|bytes| bytes.checked_add(native_storage))
            .and_then(|bytes| bytes.checked_add(header))
            .ok_or_else(|| resource(Resource::Arithmetic))?;
        let value = PreparedCrossBlockForwardingNativeOutputV1 {
            owner,
            prefix_execution,
            llvm,
            profile,
            retained_floor: floor
                .checked_add(retained)
                .ok_or_else(|| resource(Resource::Arithmetic))?,
        };
        value.verify_equivalence(budget)?;
        Ok((value, CrossBlockForwardingNativeStorageV1(retained)))
    })
}

/// Actual authenticated source/ranked owner and final text, without wire authority.
pub(crate) struct CrossBlockForwardingNativeProductionCompilationV1 {
    native: PreparedCrossBlockForwardingNativeOutputV1,
    ranked_verification:
        crate::production_ranked_projection_v1::AuthenticatedRankedVerificationRosterV1,
    bindings: AuthenticatedProductionBindings,
    retained_floor: usize,
}
impl RankedVerifiedProductionCompilation {
    /// Consumes the genuine entry through the same history and retained source
    /// before emitting its actual final graph. Reserve the returned addition.
    pub(crate) fn lower_cross_block_forwarding_native_with_budget_v1(
        self,
        limits: Limits,
        budget: &mut Budget<'_>,
    ) -> Result<(
        CrossBlockForwardingNativeProductionCompilationV1,
        CrossBlockForwardingNativeStorageV1,
    )> {
        let floor = budget.storage();
        scoped(floor, budget, move |budget| {
            let (prefix, ranked_verification, bindings) = self.prepare_native_prefix_v1(budget)?;
            let (native, storage) =
                prepare(prefix, bindings.rustc_target.profile(), limits, budget)?;
            budget
                .reserve_storage(storage.retained_storage())
                .map_err(resource)?;
            let wrapper = size_of::<CrossBlockForwardingNativeProductionCompilationV1>()
                .checked_sub(size_of::<PreparedCrossBlockForwardingNativeOutputV1>())
                .ok_or_else(|| resource(Resource::Arithmetic))?;
            budget.reserve_storage(wrapper).map_err(resource)?;
            let value = CrossBlockForwardingNativeProductionCompilationV1 {
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
            Ok((value, CrossBlockForwardingNativeStorageV1(retained)))
        })
    }
}
impl CrossBlockForwardingNativeProductionCompilationV1 {
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
                    "complete cross-block forwarding target/ranked custody",
                ));
            }
            self.native.verify_equivalence(budget)
        })
    }
}

#[cfg(test)]
#[path = "production_pipeline_cross_block_forwarding_native_v1_tests.rs"]
mod tests;
