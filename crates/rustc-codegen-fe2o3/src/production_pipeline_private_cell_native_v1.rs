//! Unnumbered source-bound promotion after the unchanged P8 prefix.
//! The promoted graph is not K, a P8 descriptor, a worker handoff or a default
//! policy. Native text is inert and retains its actual source-bearing owner.
#![allow(
    dead_code,
    reason = "pending explicitly versioned successor composition"
)]

use super::checked_output_policy7_v1::{
    Admitted7, Admitted7Ref, Policy7ExecutionWitnessV1, Prefix6, prepare_history_v1, scoped,
};
use super::{
    AuthenticatedProductionBindings, ProductionPipelineError, RankedVerifiedProductionCompilation,
};
use fe2o3_amd_target::ProductionAmdTargetProfileV1 as Profile;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    VerifiedCanonicalKernelIrModuleV12 as Graph,
};
use fe2o3_lower_mir_kernel::{
    ProductionOwnedPrivateCellPromotionContinuationV1 as Direct,
    ProductionOwnedUnitLocalPrivateCellPromotionContinuationV1 as Erased,
};
use std::{fmt, mem::size_of};

#[derive(Debug)]
pub(crate) enum PrivateCellNativeStageErrorV1 {
    Resource(Resource),
    Admission(Box<fe2o3_lower_mir_kernel::ProductionPrivateCellPromotionContinuationErrorV1>),
    Mismatch(&'static str),
}
impl fmt::Display for PrivateCellNativeStageErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for PrivateCellNativeStageErrorV1 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Admission(error) => Some(error.as_ref()),
            _ => None,
        }
    }
}
type Result<T> = std::result::Result<T, ProductionPipelineError>;
fn error(value: PrivateCellNativeStageErrorV1) -> ProductionPipelineError {
    ProductionPipelineError::PrivateCellNativeStage(value)
}
fn resource(value: Resource) -> ProductionPipelineError {
    error(PrivateCellNativeStageErrorV1::Resource(value))
}
fn mismatch(detail: &'static str) -> ProductionPipelineError {
    error(PrivateCellNativeStageErrorV1::Mismatch(detail))
}
fn admission(
    value: fe2o3_lower_mir_kernel::ProductionPrivateCellPromotionContinuationErrorV1,
) -> ProductionPipelineError {
    error(PrivateCellNativeStageErrorV1::Admission(Box::new(value)))
}

#[allow(
    clippy::large_enum_variant,
    reason = "one consumed source-bearing owner"
)]
enum Promoted {
    Direct(Direct),
    Erased(Erased),
}
impl Promoted {
    fn output(&self) -> &Graph {
        match self {
            Self::Direct(v) => v.output(),
            Self::Erased(v) => v.output(),
        }
    }
    fn historical_k(&self) -> &Graph {
        match self {
            Self::Direct(v) => v.prefix().output(),
            Self::Erased(v) => v.prefix().output(),
        }
    }
    fn history(&self) -> Admitted7Ref<'_> {
        match self {
            Self::Direct(v) => Admitted7Ref::Direct(v.prefix().prefix()),
            Self::Erased(v) => Admitted7Ref::Erased(v.prefix().prefix()),
        }
    }
    fn original(&self) -> &Graph {
        match self {
            Self::Direct(v) => v
                .prefix()
                .prefix()
                .prefix()
                .source_semantic_kir()
                .pre_ranked_executable()
                .expect("admitted direct source N"),
            Self::Erased(v) => v.prefix().prefix().prefix().original_source().executable(),
        }
    }
    fn header(&self) -> usize {
        match self {
            Self::Direct(_) => size_of::<Direct>(),
            Self::Erased(_) => size_of::<Erased>(),
        }
    }
    fn replay(&self, budget: &mut Budget<'_>) -> Result<()> {
        match self {
            Self::Direct(v) => v.verify_equivalence(budget),
            Self::Erased(v) => v.verify_equivalence(budget),
        }
        .map_err(admission)
    }
    fn continue_once(history: Admitted7, budget: &mut Budget<'_>) -> Result<(Self, usize)> {
        let (owner, prefix_added, promoted_added) = match history {
            Admitted7::Direct(v) => {
                let (prefix, added) = v
                    .continue_commutative_bitwise_cse_v1(budget)
                    .map_err(super::checked_output_policy8_v1::admission)?;
                budget
                    .reserve_storage(added.retained_storage())
                    .map_err(resource)?;
                let (promoted, storage) = prefix
                    .continue_private_cell_promotion_v1(budget)
                    .map_err(admission)?;
                (
                    Self::Direct(promoted),
                    added.retained_storage(),
                    storage.retained_storage(),
                )
            }
            Admitted7::Erased(v) => {
                let (prefix, added) = v
                    .continue_commutative_bitwise_cse_v1(budget)
                    .map_err(super::checked_output_policy8_v1::admission)?;
                budget
                    .reserve_storage(added.retained_storage())
                    .map_err(resource)?;
                let (promoted, storage) = prefix
                    .continue_private_cell_promotion_v1(budget)
                    .map_err(admission)?;
                (
                    Self::Erased(promoted),
                    added.retained_storage(),
                    storage.retained_storage(),
                )
            }
        };
        budget.reserve_storage(promoted_added).map_err(resource)?;
        Ok((
            owner,
            prefix_added
                .checked_add(promoted_added)
                .ok_or_else(|| resource(Resource::Arithmetic))?,
        ))
    }
}

/// Complete added storage, returned unreserved. Inherited source reservations
/// stay live; reserve this receipt before any subsequent controlled operation.
#[derive(Clone, Copy)]
pub(crate) struct PrivateCellNativeStorageV1(usize);
impl PrivateCellNativeStorageV1 {
    pub(crate) const fn retained_storage(self) -> usize {
        self.0
    }
}

pub(crate) struct PreparedPrivateCellNativeOutputV1 {
    owner: Promoted,
    prefix_execution: Policy7ExecutionWitnessV1,
    llvm: String,
    profile: Profile,
    retained_floor: usize,
}
impl PreparedPrivateCellNativeOutputV1 {
    pub(crate) fn output(&self) -> &Graph {
        self.owner.output()
    }
    pub(crate) fn historical_p8_output(&self) -> &Graph {
        self.owner.historical_k()
    }
    pub(crate) fn original(&self) -> &Graph {
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

/// Uses the existing complete-module native preflight and actual-owner semantic
/// anchor path. Its bounded lowering engine has a separate internal policy;
/// here both live strings are prepaid and returned capacity is retained once.
fn lower_native(
    output: &Graph,
    profile: Profile,
    budget: &mut Budget<'_>,
) -> Result<(String, usize)> {
    scoped(budget.storage(), budget, |budget| {
        budget.charge_work(3).map_err(resource)?;
        let scratch = dialect_amdgcn::MAX_COMPILER_MODULE_TEXT_BYTES
            .checked_mul(3)
            .and_then(|n| n.checked_add(size_of::<String>() * 2))
            .ok_or_else(|| resource(Resource::Arithmetic))?;
        budget.reserve_storage(scratch).map_err(resource)?;
        let native = match profile {
            Profile::Gfx942 => dialect_amdgcn::lower_canonical_v12_compiler_module_to_gfx942_xnack_minus_llvm_ir_with_semantic_anchors_v1(output),
            Profile::Gfx950 => dialect_amdgcn::lower_canonical_v12_compiler_module_to_gfx950_xnack_minus_llvm_ir_with_semantic_anchors_v1(output),
        }.map_err(ProductionPipelineError::TargetLowering)?;
        let llvm = dialect_amdgcn::bind_production_llvm22_worker_layout_v1(&native)
            .map_err(ProductionPipelineError::UpstreamLlvmLayoutBinding)?;
        let live = native
            .capacity()
            .checked_add(llvm.capacity())
            .and_then(|n| n.checked_add(size_of::<String>() * 2))
            .ok_or_else(|| resource(Resource::Arithmetic))?;
        if live > scratch {
            budget.reserve_storage(live - scratch).map_err(resource)?;
        }
        drop(native);
        let retained = llvm
            .capacity()
            .checked_add(size_of::<String>())
            .ok_or_else(|| resource(Resource::Arithmetic))?;
        Ok((llvm, retained))
    })
}

fn check_native_text(
    output: &Graph,
    profile: Profile,
    actual: &str,
    budget: &mut Budget<'_>,
) -> Result<()> {
    scoped(budget.storage(), budget, |budget| {
        let (expected, storage) = lower_native(output, profile, budget)?;
        budget.reserve_storage(storage).map_err(resource)?;
        budget
            .charge_work(
                expected
                    .len()
                    .checked_add(actual.len())
                    .and_then(|n| n.checked_add(1))
                    .ok_or_else(|| resource(Resource::Arithmetic))?,
            )
            .map_err(resource)?;
        if expected != actual {
            return Err(mismatch("exact promoted native LLVM"));
        }
        Ok(())
    })
}

fn prepare(
    prefix: Prefix6,
    profile: Profile,
    budget: &mut Budget<'_>,
) -> Result<(
    PreparedPrivateCellNativeOutputV1,
    PrivateCellNativeStorageV1,
)> {
    let floor = budget.storage();
    scoped(prefix.minimum()?, budget, move |budget| {
        let (history, prefix_execution, history_added) = prepare_history_v1(prefix, budget)?;
        let (owner, added) = Promoted::continue_once(history, budget)?;
        let (llvm, native_storage) = lower_native(owner.output(), profile, budget)?;
        budget.reserve_storage(native_storage).map_err(resource)?;
        // Nested owning receipts already count these three embedded headers.
        let header = size_of::<PreparedPrivateCellNativeOutputV1>()
            .checked_sub(owner.header())
            .and_then(|n| n.checked_sub(size_of::<Policy7ExecutionWitnessV1>()))
            .and_then(|n| n.checked_sub(size_of::<String>()))
            .ok_or_else(|| resource(Resource::Arithmetic))?;
        budget.reserve_storage(header).map_err(resource)?;
        let retained = history_added
            .checked_add(prefix_execution.retained_storage())
            .and_then(|n| n.checked_add(added))
            .and_then(|n| n.checked_add(native_storage))
            .and_then(|n| n.checked_add(header))
            .ok_or_else(|| resource(Resource::Arithmetic))?;
        let value = PreparedPrivateCellNativeOutputV1 {
            owner,
            prefix_execution,
            llvm,
            profile,
            retained_floor: floor
                .checked_add(retained)
                .ok_or_else(|| resource(Resource::Arithmetic))?,
        };
        value.verify_equivalence(budget)?;
        Ok((value, PrivateCellNativeStorageV1(retained)))
    })
}

/// Collector/ranked and genuine prefix custody, with actual promoted native
/// text. No serialized successor policy or worker/publication path is implied.
pub(crate) struct PrivateCellNativeProductionCompilationV1 {
    native: PreparedPrivateCellNativeOutputV1,
    ranked_verification:
        crate::production_ranked_projection_v1::AuthenticatedRankedVerificationRosterV1,
    bindings: AuthenticatedProductionBindings,
    retained_floor: usize,
}
impl RankedVerifiedProductionCompilation {
    /// Returns a complete added receipt on the original caller ledger. Every
    /// failure and unwind restores its entry floor; work remains cumulative.
    pub(crate) fn lower_private_cell_native_with_budget_v1(
        self,
        budget: &mut Budget<'_>,
    ) -> Result<(
        PrivateCellNativeProductionCompilationV1,
        PrivateCellNativeStorageV1,
    )> {
        let floor = budget.storage();
        scoped(floor, budget, move |budget| {
            use fe2o3_lower_mir_kernel::ProductionHelperSourcePolicyV1;
            let policy = self.ranked.materialized().helper_source_policy_v1();
            let (prefix, ranked_verification, bindings) = match policy {
                ProductionHelperSourcePolicyV1::RawEmpty => {
                    let stage = self.prepare_admitted_policy6_v1(budget)?;
                    (
                        Prefix6::Direct(stage.admitted),
                        stage.ranked_verification,
                        stage.bindings,
                    )
                }
                ProductionHelperSourcePolicyV1::UnitLocal => {
                    let stage = self.prepare_admitted_erased_policy6_v1(budget)?;
                    (
                        Prefix6::Erased(stage.admitted),
                        stage.ranked_verification,
                        stage.bindings,
                    )
                }
            };
            let (native, storage) = prepare(prefix, bindings.rustc_target.profile(), budget)?;
            budget
                .reserve_storage(storage.retained_storage())
                .map_err(resource)?;
            budget
                .reserve_storage(
                    size_of::<PrivateCellNativeProductionCompilationV1>()
                        .checked_sub(size_of::<PreparedPrivateCellNativeOutputV1>())
                        .ok_or_else(|| resource(Resource::Arithmetic))?,
                )
                .map_err(resource)?;
            let value = PrivateCellNativeProductionCompilationV1 {
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
            Ok((value, PrivateCellNativeStorageV1(retained)))
        })
    }
}
impl PrivateCellNativeProductionCompilationV1 {
    #[cfg(test)]
    pub(crate) fn source_test_historical_p8_output_v1(&self) -> &Graph {
        self.native.historical_p8_output()
    }
    #[cfg(test)]
    pub(crate) fn source_test_promotion_v1(&self) -> (usize, bool) {
        match &self.native.owner {
            Promoted::Direct(v) => (v.continuation().selected_allocations().len(), false),
            Promoted::Erased(v) => (v.continuation().selected_allocations().len(), true),
        }
    }
    pub(crate) fn output(&self) -> &Graph {
        self.native.output()
    }
    pub(crate) fn llvm_ir(&self) -> &str {
        self.native.llvm_ir()
    }
    pub(crate) fn original(&self) -> &Graph {
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
                return Err(mismatch("complete promoted target/ranked custody"));
            }
            self.native.verify_equivalence(budget)
        })
    }
}

#[cfg(test)]
#[path = "production_pipeline_private_cell_native_v1_tests.rs"]
mod tests;
