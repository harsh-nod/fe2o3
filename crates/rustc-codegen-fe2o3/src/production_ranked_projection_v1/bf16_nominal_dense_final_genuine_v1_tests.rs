//! Final C2/C3 visitor controls using only the existing genuine owner ladder.
//! Isolated damaged-ledger probes are prepaid on the surrounding original
//! account; they never authorize source/ranked continuation.
use super::*;
use crate::production_ranked_projection_v1::bf16_nominal_call_routing_v1::with_nominal_summary_v1;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceErrorV1 as Resource, CanonicalKernelIrWorkBudgetV1 as Work,
};
use std::cell::Cell;
use std::mem::size_of;
use std::panic::{AssertUnwindSafe, catch_unwind};

const PROBE_WORK: usize = 8 * 1024 * 1024;
const PROBE_SCRATCH: usize = 8 * 1024 * 1024;
const HEADERS: usize = 4096;
const EXTRA_STORAGE: usize = 23;
const EXTRA_WORK: usize = 17;

// This callback remains higher-ranked over the original work borrow. In
// particular, no captured stack-borrowed foreign Budget can replace it.
fn final_route(
    owner: &ProductionPreRankedKirOwnerV1,
    source: &CheckedBf16CallInstanceV1<'_>,
    inventory: &CanonicalKirInventoryV1<'_>,
    budget: &mut Budget<'_>,
    inspect: &mut FinalOccurrenceVisitor<'_>,
) -> Result<()> {
    with_nominal_summary_v1(
        owner,
        inventory,
        source.root(),
        source.root(),
        source.call_block(),
        source.source_call(),
        budget,
        |_summaries, budget| with_prepared_dense_final(owner, source, inventory, budget, inspect),
    )
}

fn with_headers<'w>(
    budget: &mut Budget<'w>,
    bytes: usize,
    inspect: impl FnOnce(&mut Budget<'w>) -> Result<()>,
) -> Result<()> {
    let ledger = budget.work_ledger_identity_v1();
    budget.reserve_storage(bytes)?;
    let protected = budget.storage();
    let caught = catch_unwind(AssertUnwindSafe(|| inspect(budget)));
    let result = match caught {
        Ok(result) => result,
        Err(payload) => {
            drop(payload);
            Err(QueryError::CallbackPanicked)
        }
    };
    if budget.work_ledger_identity_v1() != ledger || budget.storage() < protected {
        return Err(Resource::Accounting.into());
    }
    budget.release_storage(bytes)?;
    result
}

fn final_callback_charges(
    owner: &ProductionPreRankedKirOwnerV1,
    source: &CheckedBf16CallInstanceV1<'_>,
    inventory: &CanonicalKirInventoryV1<'_>,
    budget: &mut Budget<'_>,
) -> Result<()> {
    for mode in 0..3 {
        let before = budget.storage();
        let work_before = budget.work();
        let original = budget.work_ledger_identity_v1();
        let visits = Cell::new(0usize);
        let callback_peak = Cell::new(0usize);
        let result = final_route(owner, source, inventory, budget, &mut |_, budget| {
            visits.set(visits.get() + 1);
            assert_eq!(visits.get(), 1, "actual final occurrence entered once");
            assert!(budget.work_ledger_identity_v1() == original);
            budget.reserve_storage(EXTRA_STORAGE)?;
            budget.charge_work(EXTRA_WORK)?;
            callback_peak.set(budget.peak_storage());
            match mode {
                0 => Ok(()),
                1 => Err(QueryError::Unavailable(
                    "genuine final dense visitor refusal",
                )),
                _ => panic!("controlled genuine final dense visitor unwind"),
            }
        });
        assert_eq!(visits.get(), 1, "all three real passes reached C3");
        assert_eq!(
            result,
            match mode {
                0 => Ok(()),
                1 => Err(QueryError::Unavailable(
                    "genuine final dense visitor refusal"
                )),
                _ => Err(QueryError::CallbackPanicked),
            }
        );
        assert!(budget.work_ledger_identity_v1() == original);
        assert_eq!(budget.storage(), before + EXTRA_STORAGE);
        assert!(budget.work() >= work_before + EXTRA_WORK);
        assert!(budget.peak_storage() >= callback_peak.get());
        assert_eq!(
            (budget.failed_work(), budget.failed_storage()),
            (None, None)
        );
        budget.release_storage(EXTRA_STORAGE)?;
        assert_eq!(budget.storage(), before);
    }
    Ok(())
}

fn final_custody_probes(
    owner: &ProductionPreRankedKirOwnerV1,
    source: &CheckedBf16CallInstanceV1<'_>,
    inventory: &CanonicalKirInventoryV1<'_>,
    inventory_storage: usize,
    original: &mut Budget<'_>,
) -> Result<()> {
    let occurrence = owner
        .semantic_ssa()
        .occurrence_storage()
        .expect("genuine owner retains occurrence receipt")
        .retained_storage();
    let floor = owner
        .retained_analysis_storage_v1()
        .checked_add(occurrence)
        .and_then(|n| n.checked_add(inventory_storage))
        .ok_or(Resource::Arithmetic)?;
    let limit = floor
        .checked_add(PROBE_SCRATCH)
        .ok_or(Resource::Arithmetic)?;
    assert!(original.storage() >= floor);
    // Whole positive source and all derived owners remain charged to ORIGINAL.
    // Probe ledgers model accounting faults only, never new owner authority.
    with_headers(original, PROBE_SCRATCH + HEADERS, |original| {
        original.charge_work(3 * PROBE_WORK + 128)?;
        for mode in 0..3 {
            let mut work = Work::new(PROBE_WORK);
            let mut budget = Budget::new(&mut work, limit);
            budget.reserve_storage(floor)?;
            let ledger = budget.work_ledger_identity_v1();
            let visits = Cell::new(0usize);
            let protected = Cell::new(0usize);
            let work_at_callback = Cell::new(0usize);
            let peak_at_callback = Cell::new(0usize);
            let result = final_route(owner, source, inventory, &mut budget, &mut |_, budget| {
                visits.set(visits.get() + 1);
                assert_eq!(visits.get(), 1);
                assert!(budget.work_ledger_identity_v1() == ledger);
                protected.set(budget.storage());
                work_at_callback.set(budget.work());
                peak_at_callback.set(budget.peak_storage());
                match mode {
                    0 => {
                        let _ = budget.charge_work(PROBE_WORK + 1);
                    }
                    1 => {
                        let _ = budget.reserve_storage(limit + 1);
                    }
                    _ => {
                        budget.release_storage(1)?;
                    }
                }
                Ok(())
            });
            assert_eq!(visits.get(), 1, "fault is inside actual final C3 callback");
            assert_eq!(result, Err(Resource::Accounting.into()));
            assert!(budget.work_ledger_identity_v1() == ledger);
            assert!(budget.work() >= work_at_callback.get());
            assert!(budget.peak_storage() >= peak_at_callback.get());
            if mode < 2 {
                assert_eq!(budget.storage(), floor);
                assert_eq!(budget.failed_work().is_some(), mode == 0);
                assert_eq!(budget.failed_storage().is_some(), mode == 1);
            } else {
                // Damaged owned scopes must not be repaired. Outer scopes can
                // refund only their still-valid own reservations.
                assert!(budget.storage() >= floor);
                assert!(budget.storage() < protected.get());
                assert_eq!(
                    (budget.failed_work(), budget.failed_storage()),
                    (None, None)
                );
            }
        }
        Ok(())
    })
}

pub(super) fn inspect(
    owner: &ProductionPreRankedKirOwnerV1,
    source: &CheckedBf16CallInstanceV1<'_>,
    inventory: &CanonicalKirInventoryV1<'_>,
    inventory_storage: usize,
    budget: &mut Budget<'_>,
) -> Result<()> {
    let before = budget.storage();
    let identity = budget.work_ledger_identity_v1();
    with_headers(budget, HEADERS, |budget| {
        budget.charge_work(128)?;
        final_callback_charges(owner, source, inventory, budget)?;
        final_custody_probes(owner, source, inventory, inventory_storage, budget)
    })?;
    assert!(budget.work_ledger_identity_v1() == identity);
    assert_eq!(budget.storage(), before);
    assert_eq!(
        (budget.failed_work(), budget.failed_storage()),
        (None, None)
    );
    Ok(())
}

#[test]
fn final_control_fixed_headers_fit_the_existing_test_envelope() {
    // Synthetic type-size check only. The actual visitors run solely from the
    // genuine owner ladder; no source/capability constructors exist in this leaf.
    assert!(
        2 * size_of::<Budget<'static>>()
            + 2 * size_of::<Work>()
            + 16 * size_of::<Cell<usize>>()
            + 8 * size_of::<Result<()>>()
            + 1024
            <= HEADERS
    );
}
