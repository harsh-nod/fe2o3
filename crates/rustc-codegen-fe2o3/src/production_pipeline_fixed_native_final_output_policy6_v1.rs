//! Consuming fixed facade for the existing typed final-I receipt consumer.
//! No F2NOUT1 admission, signed proof creation, or publication is added here.
use super::*;
use native::policy6::final_receipts::{
    NativeFinalOutputReceiptsPolicy6V1 as Receipts,
    PreparedNativeFinalOutputPolicy6V1 as FinalOwner,
};
use std::mem::size_of;

#[cfg(test)]
#[path = "production_pipeline_fixed_native_final_output_policy6_v1_tests.rs"]
mod tests;

/// Retains the sole signed native owner, exact typed final receipt bytes, and
/// the original fixed facade's logical header envelope. No raw constructor.
pub(crate) struct FixedNativeFinalOutputProductionCompilationPolicy6V1 {
    native: FinalOwner,
    retained_storage_floor: usize,
}

fn receipt_floors(
    facade: usize,
    native: usize,
    receipts: usize,
) -> Result<(usize, usize), ProductionPipelineError> {
    if facade < native {
        return Err(resource(Resource::Accounting));
    }
    let required = facade
        .checked_add(receipts)
        .ok_or_else(|| resource(Resource::Arithmetic))?;
    let branch = native
        .checked_add(receipts)
        .ok_or_else(|| resource(Resource::Arithmetic))?;
    Ok((required, branch))
}

impl FixedNativeCheckedOutputProductionCompilationPolicy6V1 {
    /// Consumes already-reserved facade and receipt owners once. The existing
    /// native owner performs all semantic, signed, native, and receipt checks.
    /// Both incoming reservations remain caller-owned on every exit. Success
    /// returns only additional wrapper growth, unreserved, without double
    /// charging the receipt bytes or releasing the carried facade header.
    pub(crate) fn try_admit_final_output_receipts_v1(
        self,
        receipts: Receipts,
        budget: &mut Budget<'_>,
    ) -> Result<
        (
            FixedNativeFinalOutputProductionCompilationPolicy6V1,
            FixedNativeCheckedOutputStoragePolicy6V1,
        ),
        ProductionPipelineError,
    > {
        let (inherited, branch_floor) = receipt_floors(
            self.retained_storage_floor,
            self.native.retained_storage_floor_v1(),
            receipts.retained_storage(),
        )?;
        let entry_floor = budget.storage();
        transfer(inherited, budget, move |budget, ledger, slot| {
            let (native, added) = self
                .native
                .try_admit_final_output_receipts_v1(receipts, budget)
                .map_err(native_error)?;
            let branch_added = added.retained_storage();
            reserve_current(budget, ledger, slot, branch_added)?;
            if entry_floor.checked_add(branch_added) != Some(budget.storage()) {
                return Err(resource(Resource::Accounting));
            }
            let wrapper_header = size_of::<FixedNativeFinalOutputProductionCompilationPolicy6V1>()
                .checked_sub(size_of::<FinalOwner>())
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
                FixedNativeFinalOutputProductionCompilationPolicy6V1 {
                    native,
                    retained_storage_floor,
                },
                receipt,
            ))
        })
    }
}

impl FixedNativeFinalOutputProductionCompilationPolicy6V1 {
    pub(crate) fn output(&self) -> &Owner {
        self.native.output()
    }
    pub(crate) fn handoff(&self) -> &fe2o3_compiler_ffi::CompilerModuleHandoffV2 {
        self.native.handoff()
    }
    pub(crate) fn receipts(&self) -> &Receipts {
        self.native.receipts()
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
