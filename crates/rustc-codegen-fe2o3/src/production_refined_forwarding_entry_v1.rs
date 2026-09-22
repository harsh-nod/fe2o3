//! One fixed, nondefault route from ranked source to paired source/final-F bytes.
use super::super::{
    PreparedRefinedForwardingWireV1, RefinedForwardingWireErrorV1, RefinedForwardingWireStorageV1,
};
use super::{Budget, MAX_STORAGE, Resource, scoped};
use crate::production_pipeline::RankedVerifiedProductionCompilation;
use fe2o3_kernel_analysis::{
    CanonicalKirCrossBlockForwardingLimitsV1 as ForwardLimits,
    CanonicalKirLoopLimitsV1 as RefineLimits,
};

type E = RefinedForwardingWireErrorV1;
type R<T> = Result<T, E>;

/// Added native, signed-source worker, and wire backing, returned unreserved.
/// Reserve before controlled use; drop the wire owner before releasing it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RankedRefinedForwardingWireStorageV1(usize);
impl RankedRefinedForwardingWireStorageV1 {
    pub(crate) const fn retained_storage(self) -> usize {
        self.0
    }
}

fn limits() -> (RefineLimits, ForwardLimits) {
    (RefineLimits::default(), ForwardLimits::default())
}

fn entry(budget: &mut Budget<'_>) -> R<()> {
    budget.charge_work(1)?;
    if budget.storage_limit() > MAX_STORAGE {
        return Err(E::Mismatch("bounded storage cap"));
    }
    Ok(())
}

fn transfer(
    added: usize,
    owner_floor: usize,
    floor: usize,
    retained: &mut usize,
    budget: &mut Budget<'_>,
) -> R<()> {
    let current = floor.checked_add(*retained).ok_or(Resource::Arithmetic)?;
    let next = retained.checked_add(added).ok_or(Resource::Arithmetic)?;
    let expected = floor.checked_add(next).ok_or(Resource::Arithmetic)?;
    if budget.storage() != current || owner_floor != expected {
        return Err(Resource::Accounting.into());
    }
    budget.reserve_storage(added)?;
    budget.charge_work(1)?;
    *retained = next;
    Ok(())
}

impl RankedVerifiedProductionCompilation {
    pub(crate) fn prepare_inert_refined_forwarding_wire_with_budget_v1(
        self,
        budget: &mut Budget<'_>,
    ) -> R<(
        PreparedRefinedForwardingWireV1,
        RankedRefinedForwardingWireStorageV1,
    )> {
        let floor = budget.storage();
        scoped(budget, move |budget| {
            entry(budget)?;
            let (refinement, forwarding) = limits();
            let (native, native_storage) = self
                .lower_refined_cross_block_forwarding_native_with_budget_v1(
                    refinement, forwarding, budget,
                )
                .map_err(E::Live)?;
            let mut retained = 0;
            transfer(
                native_storage.retained_storage(),
                native.retained_storage_floor_v1(),
                floor,
                &mut retained,
                budget,
            )?;
            let (live, live_storage) = native
                .prepare_refined_forwarding_worker_handoff_v1(budget)
                .map_err(E::Live)?;
            transfer(
                live_storage.retained_storage(),
                live.retained_storage_floor_v1(),
                floor,
                &mut retained,
                budget,
            )?;
            let (wire, wire_storage): (_, RefinedForwardingWireStorageV1) =
                live.into_inert_output_wire_v1(budget)?;
            transfer(
                wire_storage.retained_storage(),
                wire.retained_storage_floor_v1(),
                floor,
                &mut retained,
                budget,
            )?;
            wire.verify_equivalence(budget)?;
            Ok((wire, RankedRefinedForwardingWireStorageV1(retained)))
        })
    }
}

#[cfg(test)]
#[path = "production_refined_forwarding_entry_v1_tests.rs"]
mod tests;
