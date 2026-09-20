//! Test-only consuming observation after the genuine fixed7 construction.
//! Historical J artifacts stay J artifacts; no K descriptor or emitter exists.
use super::*;
use fe2o3_lower_mir_kernel::{
    ProductionOwnedCommutativeContinuationV1 as DirectTail,
    ProductionOwnedUnitLocalCommutativeContinuationV1 as ErasedTail,
};

#[allow(
    clippy::large_enum_variant,
    reason = "one moved source/prefix, never cloned"
)]
pub(crate) enum Owner {
    Direct(DirectTail),
    Erased(ErasedTail),
}
impl Owner {
    pub(crate) fn original(&self) -> &Graph {
        match self {
            Self::Direct(v) => v
                .prefix()
                .prefix()
                .source_semantic_kir()
                .pre_ranked_executable()
                .expect("admitted source N"),
            Self::Erased(v) => v.prefix().prefix().original_source().executable(),
        }
    }
    pub(crate) fn historical_j(&self) -> &Graph {
        match self {
            Self::Direct(v) => v.prefix().output(),
            Self::Erased(v) => v.prefix().output(),
        }
    }
    pub(crate) fn output(&self) -> &Graph {
        match self {
            Self::Direct(v) => v.output(),
            Self::Erased(v) => v.output(),
        }
    }
    pub(crate) fn replay(&self, budget: &mut Budget<'_>) -> Result<(), String> {
        match self {
            Self::Direct(v) => v.verify_equivalence(budget),
            Self::Erased(v) => v.verify_equivalence(budget),
        }
        .map_err(|e| format!("complete source/prefix/J/K replay: {e:?}"))
    }
}

pub(crate) struct View<'a> {
    pub(crate) owner: &'a Owner,
    pub(crate) baseline_execution: &'a Policy7ExecutionWitnessV1,
    pub(crate) baseline_llvm: &'a str,
    pub(crate) baseline_descriptor: &'a fe2o3_compiler_ffi::CompilerDescriptorSourceV1,
    pub(crate) profile: Profile,
    pub(crate) entry_work: usize,
    pub(crate) stage_work: usize,
    pub(crate) tail_work: usize,
    pub(crate) stage_floor: usize,
    pub(crate) tail_floor: usize,
}

impl RankedVerifiedProductionCompilation {
    /// One shared ledger from real fixed7 construction through a consuming tail.
    /// The callback cannot return an owner or a borrowed graph. No production
    /// selector, old extraction hook, native stage or descriptor contract changes.
    pub(crate) fn with_commutative_tail_source_observation_v1(
        self,
        budget: &mut Budget<'_>,
        observe: impl for<'a, 'w> FnOnce(View<'a>, &mut Budget<'w>) -> Result<(), String>,
    ) -> Result7<Result<(), String>> {
        let entry_floor = budget.storage();
        let entry_work = budget.work();
        let ledger = budget.work_ledger_identity_v1();
        let slot = budget as *const Budget<'_> as usize;
        scoped(entry_floor, budget, move |budget| {
            let stage = self.lower_fixed_checked_output_policy7_with_budget_v1(budget)?;
            stage.verify_equivalence(budget)?;
            if !stage
                .bindings
                .transaction
                .compiler_custody
                .is_extraction_only()
            {
                return Err(error(
                    CheckedOutputPolicy7StageErrorV1::NativePublicationUnavailable,
                ));
            }
            let stage_floor = stage.retained_floor;
            let stage_work = budget.work();
            if budget.storage() != stage_floor
                || budget.work_ledger_identity_v1() != ledger
                || budget as *const Budget<'_> as usize != slot
                || stage_work <= entry_work
            {
                return Err(resource(Resource::Accounting));
            }
            let original_functions = stage.original().module().functions.as_ptr();
            let original_bytes = stage.original().canonical().canonical_bytes().as_ptr();
            let j_functions = stage.output().module().functions.as_ptr();
            let j_bytes = stage.output().canonical().canonical_bytes().as_ptr();
            let profile = stage.bindings.rustc_target.profile();
            let CheckedOutputTargetProductionCompilationPolicy7V1 {
                artifacts,
                ranked_verification,
                bindings,
                ..
            } = stage;
            let PreparedPolicy7ArtifactsV1 {
                admitted,
                execution,
                parts,
                ..
            } = artifacts;

            // Conservatively keep the entire inherited stage/artifact floor.
            // This additional test wrapper is prepaid before the real move.
            budget
                .reserve_storage(size_of::<Owner>())
                .map_err(resource)?;
            let before_tail_floor = budget.storage();
            let continued = match admitted {
                Admitted7::Direct(v) => v
                    .continue_commutative_bitwise_cse_v1(budget)
                    .map(|(v, receipt)| (Owner::Direct(v), receipt)),
                Admitted7::Erased(v) => v
                    .continue_commutative_bitwise_cse_v1(budget)
                    .map(|(v, receipt)| (Owner::Erased(v), receipt)),
            };
            let (owner, receipt) = match continued {
                Ok(value) => value,
                Err(error) => {
                    return Ok(Err(format!(
                        "actual source-bearing J/K continuation: {error:?}"
                    )));
                }
            };
            if budget.storage() != before_tail_floor
                || budget.work_ledger_identity_v1() != ledger
                || budget as *const Budget<'_> as usize != slot
                || budget.work() <= stage_work
                || owner.original().module().functions.as_ptr() != original_functions
                || owner.original().canonical().canonical_bytes().as_ptr() != original_bytes
                || owner.historical_j().module().functions.as_ptr() != j_functions
                || owner.historical_j().canonical().canonical_bytes().as_ptr() != j_bytes
            {
                return Err(resource(Resource::Accounting));
            }
            budget
                .reserve_storage(receipt.retained_storage())
                .map_err(resource)?;
            let tail_floor = budget.storage();
            let tail_work = budget.work();
            let result = observe(
                View {
                    owner: &owner,
                    baseline_execution: &execution,
                    baseline_llvm: parts.prepared.llvm_ir(),
                    baseline_descriptor: parts.prepared.descriptor_source(),
                    profile,
                    entry_work,
                    stage_work,
                    tail_work,
                    stage_floor,
                    tail_floor,
                },
                budget,
            );
            if budget.storage() != tail_floor
                || budget.work_ledger_identity_v1() != ledger
                || budget as *const Budget<'_> as usize != slot
                || budget.work() < tail_work
                || owner.original().module().functions.as_ptr() != original_functions
                || owner.original().canonical().canonical_bytes().as_ptr() != original_bytes
                || owner.historical_j().module().functions.as_ptr() != j_functions
                || owner.historical_j().canonical().canonical_bytes().as_ptr() != j_bytes
            {
                return Err(resource(Resource::Accounting));
            }
            // Full ranked/source custody remains live and independently checked
            // through the callback; the immutable lowerer prefix replays itself.
            if bindings
                .rustc_preflight_plan
                .rustc_identity_inventory_sha256()
                != bindings.rustc_identity_inventory.sha256()
                || ranked_verification.root_count() != owner.output().module().kernels.len()
                || !ranked_verification.every_functional_verification_is_coherent()
            {
                return Err(ProductionPipelineError::RustcLineageMismatch);
            }
            if let Err(error) = owner.replay(budget) {
                return Ok(Err(error));
            }
            if budget.storage() != tail_floor || budget.work_ledger_identity_v1() != ledger {
                return Err(resource(Resource::Accounting));
            }
            drop((owner, execution, parts, ranked_verification, bindings));
            Ok(result)
        })
    }
}
