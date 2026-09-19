//! Consuming fixed-route native preparation, still without publication authority.
use super::*;
use crate::production_pipeline::native_checked_output_handoff_v1::{
    self as native, NativeOutputHandoffErrorV1,
    policy6::PreparedNativeCheckedOutputWorkerHandoffPolicy6V1 as NativeOwner,
};
use fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1 as Ledger;

#[path = "production_pipeline_fixed_native_final_output_policy6_v1.rs"]
#[allow(
    dead_code,
    reason = "typed final receipt custody is not protected publication"
)]
pub(crate) mod final_continuation;

#[cfg(test)]
#[path = "production_pipeline_fixed_native_checked_output_policy6_v1_tests.rs"]
mod tests;

/// The original source/ranked/collector/protected invocation stays inside the
/// existing native owner. The carried facade header remains in its logical floor.
#[allow(
    dead_code,
    reason = "protected publication is a separate closed boundary"
)]
pub(crate) struct FixedNativeCheckedOutputProductionCompilationPolicy6V1 {
    native: NativeOwner,
    retained_storage_floor: usize,
}

/// Additional retained payload, transferred unreserved on success. The caller
/// keeps the original facade receipt reserved and reserves this before more work.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct FixedNativeCheckedOutputStoragePolicy6V1(usize);
#[allow(
    dead_code,
    reason = "future caller reserves the returned additional receipt"
)]
impl FixedNativeCheckedOutputStoragePolicy6V1 {
    pub(crate) const fn retained_storage(self) -> usize {
        self.0
    }
}

fn native_error(error: NativeOutputHandoffErrorV1) -> ProductionPipelineError {
    ProductionPipelineError::CheckedOutputPolicy6Stage(
        CheckedOutputPolicy6StageErrorV1::NativeWorker(Box::new(error)),
    )
}

const TRANSFER_WORK: usize = 6;

fn reserve_current(
    budget: &mut Budget<'_>,
    ledger: Ledger,
    slot: usize,
    bytes: usize,
) -> Result<(), ProductionPipelineError> {
    if budget.work_ledger_identity_v1() != ledger || budget as *const Budget<'_> as usize != slot {
        return Err(resource(Resource::Accounting));
    }
    budget.reserve_storage(bytes).map_err(resource)
}

/// Reuses native preparation's scope/ledger/drop checks. Nested Result keeps
/// original source-proof errors intact instead of reclassifying them as native.
fn transfer<'work, T>(
    required_floor: usize,
    budget: &mut Budget<'work>,
    action: impl FnOnce(&mut Budget<'work>, Ledger, usize) -> Result<T, ProductionPipelineError>,
) -> Result<T, ProductionPipelineError> {
    if budget.storage() < required_floor {
        return Err(resource(Resource::Accounting));
    }
    let ledger = budget.work_ledger_identity_v1();
    let slot = budget as *const Budget<'_> as usize;
    native::scoped(budget, |budget| {
        budget.charge_work(TRANSFER_WORK)?;
        Ok(action(budget, ledger, slot))
    })
    .map_err(native_error)?
}

fn finish_receipt(
    inherited: usize,
    branch_floor: usize,
    prepared_floor: usize,
    branch_added: usize,
    wrapper_header: usize,
) -> Result<(usize, usize, FixedNativeCheckedOutputStoragePolicy6V1), ProductionPipelineError> {
    let carried_header = inherited
        .checked_sub(branch_floor)
        .ok_or_else(|| resource(Resource::Accounting))?;
    if branch_floor.checked_add(branch_added) != Some(prepared_floor) {
        return Err(resource(Resource::Accounting));
    }
    // Reuse the old facade header envelope; only additional new header bytes
    // are charged. A larger old envelope is conservatively carried, not released.
    let extra_header = wrapper_header.saturating_sub(carried_header);
    let added = branch_added
        .checked_add(extra_header)
        .ok_or_else(|| resource(Resource::Arithmetic))?;
    let retained = inherited
        .checked_add(added)
        .ok_or_else(|| resource(Resource::Arithmetic))?;
    Ok((
        retained,
        extra_header,
        FixedNativeCheckedOutputStoragePolicy6V1(added),
    ))
}

impl FixedCheckedOutputProductionCompilationPolicy6V1 {
    /// Consumes the source-derived route once. Genuine signed source proof is
    /// required by the unchanged branch API; no unsigned or extraction fallback.
    /// Reserve this owner's full receipt before entry. All exits preserve the
    /// incoming storage floor and work history; success transfers only the new
    /// receipt unreserved, alongside complete move-only custody.
    #[allow(dead_code, reason = "not a protected publisher or a default selector")]
    pub(crate) fn prepare_native_checked_output_v1(
        self,
        budget: &mut Budget<'_>,
    ) -> Result<
        (
            FixedNativeCheckedOutputProductionCompilationPolicy6V1,
            FixedNativeCheckedOutputStoragePolicy6V1,
        ),
        ProductionPipelineError,
    > {
        let inherited = self.retained_storage_floor;
        let entry_floor = budget.storage();
        transfer(inherited, budget, move |budget, ledger, slot| {
            let branch_floor;
            let native;
            let source_added;
            let worker_added;
            match self.stage {
                Stage::Direct(stage) => {
                    branch_floor = stage.retained_storage_floor_v1();
                    let (source, receipt) = stage.prepare_native_source_lineage_v1(budget)?;
                    source_added = receipt.retained_storage();
                    reserve_current(budget, ledger, slot, source_added)?;
                    let (owner, receipt) = source
                        .prepare_native_worker_handoff_v1(budget)
                        .map_err(native_error)?;
                    native = owner;
                    worker_added = receipt.retained_storage();
                }
                Stage::Erased(stage) => {
                    branch_floor = stage.retained_storage_floor_v1();
                    let (source, receipt) = stage.prepare_native_source_lineage_v1(budget)?;
                    source_added = receipt.retained_storage();
                    reserve_current(budget, ledger, slot, source_added)?;
                    let (owner, receipt) = source
                        .prepare_native_worker_handoff_v1(budget)
                        .map_err(native_error)?;
                    native = owner;
                    worker_added = receipt.retained_storage();
                }
            }
            reserve_current(budget, ledger, slot, worker_added)?;
            let branch_added = source_added
                .checked_add(worker_added)
                .ok_or_else(|| resource(Resource::Arithmetic))?;
            if entry_floor.checked_add(branch_added) != Some(budget.storage()) {
                return Err(resource(Resource::Accounting));
            }
            let wrapper_header =
                std::mem::size_of::<FixedNativeCheckedOutputProductionCompilationPolicy6V1>()
                    .checked_sub(std::mem::size_of::<NativeOwner>())
                    .ok_or_else(|| resource(Resource::Arithmetic))?;
            let (retained_storage_floor, extra_header, receipt) = finish_receipt(
                inherited,
                branch_floor,
                native.retained_storage_floor_v1(),
                branch_added,
                wrapper_header,
            )?;
            reserve_current(budget, ledger, slot, extra_header)?;
            Ok((
                FixedNativeCheckedOutputProductionCompilationPolicy6V1 {
                    native,
                    retained_storage_floor,
                },
                receipt,
            ))
        })
    }
}

#[allow(
    dead_code,
    reason = "borrowed observations for a future protected consumer"
)]
impl FixedNativeCheckedOutputProductionCompilationPolicy6V1 {
    pub(crate) fn output(&self) -> &Owner {
        self.native.output()
    }
    pub(crate) fn handoff(&self) -> &fe2o3_compiler_ffi::CompilerModuleHandoffV2 {
        self.native.handoff()
    }
    pub(crate) const fn retained_storage_floor_v1(&self) -> usize {
        self.retained_storage_floor
    }
    pub(crate) const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
    pub(crate) fn verify_equivalence(
        &self,
        budget: &mut Budget<'_>,
    ) -> Result<(), ProductionPipelineError> {
        if budget.storage() < self.retained_storage_floor {
            return Err(resource(Resource::Accounting));
        }
        self.native.verify_equivalence(budget).map_err(native_error)
    }
}
