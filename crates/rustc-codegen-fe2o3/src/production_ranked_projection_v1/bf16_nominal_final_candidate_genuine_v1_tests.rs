//! Actual source owners only: postflight candidate observations and controls.
//! No positive source fixture, owned recipe, normal admission or ranked position.
use super::*;
use crate::production_ranked_projection_v1::bf16_nominal_final_candidate_v1::{
    NominalFinalRankedCandidateV1, with_nominal_final_ranked_candidate_v1,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkBudgetV1 as Work, Terminator,
};
use std::cell::Cell;
use std::panic::{AssertUnwindSafe, catch_unwind};

const PROBE_WORK: usize = 8 * 1024 * 1024;
const PROBE_SCRATCH: usize = 8 * 1024 * 1024;
const HEADERS: usize = 8192;
const EXTRA_STORAGE: usize = 23;
const EXTRA_WORK: usize = 17;

fn observe_view(
    view: &NominalFinalRankedCandidateV1<'_, '_>,
    owner: &ProductionPreRankedKirOwnerV1,
    inventory: &CanonicalKirInventoryV1<'_>,
    source: &CheckedBf16CallInstanceV1<'_>,
    budget: &mut Budget<'_>,
) -> Result<()> {
    budget.charge_work(256)?;
    assert!(std::ptr::eq(view.owner(), owner));
    assert!(std::ptr::eq(view.inventory(), inventory));
    assert!(std::ptr::eq(view.source_call(), source.source_call()));
    assert!(view.inventory().belongs_to(owner.executable()));
    assert_ne!(
        view.call().coordinate.block.function,
        view.matrix().coordinate.block.function
    );
    assert_eq!(
        view.returned().coordinate.function,
        view.matrix().coordinate.block.function
    );
    assert_eq!(
        view.permutation(),
        owner
            .bf16_call_instance_emission_v1()
            .unwrap()
            .return_permutation()
    );
    let Terminator::Return { values } = view.returned().terminator else {
        panic!("actual Return");
    };
    for (index, row) in view.rows().iter().enumerate() {
        assert_eq!(row.caller_value(), view.call().operation.results[index].id);
        assert_eq!(row.return_value(), values[index]);
        assert_eq!(
            row.matrix_value(),
            view.matrix().operation.results[usize::from(view.permutation()[index])].id
        );
    }
    let crate::production_ranked_projection_v1::ProductionRankedOperationV1::TensorLayout {
        active_lanes: 64,
        binding: Some(binding),
        ..
    } = view.operation()
    else {
        panic!("fixed candidate tensor");
    };
    assert_eq!(binding.argument_count(), 4);
    // No equality of Return IDs with Matrix IDs and no ranked block index.
    Ok(())
}

pub(super) fn observe(
    owner: &ProductionPreRankedKirOwnerV1,
    source: &CheckedBf16CallInstanceV1<'_>,
    inventory: &CanonicalKirInventoryV1<'_>,
    budget: &mut Budget<'_>,
) -> Result<()> {
    let entry = budget.storage();
    let original = budget.work_ledger_identity_v1();
    let entered = Cell::new(0usize);
    with_nominal_final_ranked_candidate_v1(
        owner,
        inventory,
        source.root(),
        source.call_block(),
        source.source_call(),
        budget,
        |view, budget| {
            entered.set(entered.get() + 1);
            assert!(budget.work_ledger_identity_v1() == original);
            observe_view(view, owner, inventory, source, budget)
        },
    )?;
    assert_eq!(entered.get(), 1);
    assert!(budget.work_ledger_identity_v1() == original);
    assert_eq!(budget.storage(), entry);
    Ok(())
}

fn with_headers<'w>(
    budget: &mut Budget<'w>,
    bytes: usize,
    body: impl FnOnce(&mut Budget<'w>) -> Result<()>,
) -> Result<()> {
    let ledger = budget.work_ledger_identity_v1();
    budget.reserve_storage(bytes)?;
    let protected = budget.storage();
    let caught = catch_unwind(AssertUnwindSafe(|| body(budget)));
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

pub(super) fn controls(
    owner: &ProductionPreRankedKirOwnerV1,
    source: &CheckedBf16CallInstanceV1<'_>,
    inventory: &CanonicalKirInventoryV1<'_>,
    inventory_storage: usize,
    original: &mut Budget<'_>,
) -> Result<()> {
    large_entry_controls(owner, source, inventory, inventory_storage, original)?;
    with_headers(original, HEADERS, |original| {
        original.charge_work(128)?;
        for mode in 0..3 {
            let entry = original.storage();
            let work = original.work();
            let identity = original.work_ledger_identity_v1();
            let entered = Cell::new(0usize);
            let result = with_nominal_final_ranked_candidate_v1(
                owner,
                inventory,
                source.root(),
                source.call_block(),
                source.source_call(),
                original,
                |view, budget| {
                    entered.set(entered.get() + 1);
                    assert!(budget.work_ledger_identity_v1() == identity);
                    observe_view(view, owner, inventory, source, budget)?;
                    budget.reserve_storage(EXTRA_STORAGE)?;
                    budget.charge_work(EXTRA_WORK)?;
                    match mode {
                        0 => Ok(()),
                        1 => Err(QueryError::Unavailable(
                            "genuine final-candidate continuation refusal",
                        )),
                        _ => panic!("controlled genuine final-candidate continuation unwind"),
                    }
                },
            );
            assert_eq!(entered.get(), 1);
            assert_eq!(
                result,
                match mode {
                    0 => Ok(()),
                    1 => Err(QueryError::Unavailable(
                        "genuine final-candidate continuation refusal"
                    )),
                    _ => Err(QueryError::CallbackPanicked),
                }
            );
            assert!(original.work_ledger_identity_v1() == identity);
            assert_eq!(original.storage(), entry + EXTRA_STORAGE);
            assert!(original.work() >= work + EXTRA_WORK);
            assert_eq!(
                (original.failed_work(), original.failed_storage()),
                (None, None)
            );
            original.release_storage(EXTRA_STORAGE)?;
        }
        let occurrence = owner
            .semantic_ssa()
            .occurrence_storage()
            .unwrap()
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
        with_headers(original, PROBE_SCRATCH, |original| {
            // Direct initial-N1 refusal and wrapped initial-N1 refusal calibrate
            // the actually accepted strict entry frame. Every probe has a fresh
            // floor/peak and unchanged caps; all five are prepaid on ORIGINAL.
            // These accounting-only ledgers grant no separate source authority.
            original.charge_work(5 * PROBE_WORK + 256)?;
            let invalid_block =
                crate::production_ranked_projection_v1::SemanticBlockIdV1::from_index(u32::MAX);
            assert_ne!(invalid_block, source.call_block());
            let direct_peak = {
                let mut work = Work::new(PROBE_WORK);
                let mut budget = Budget::new(&mut work, limit);
                budget.reserve_storage(floor)?;
                let identity = budget.work_ledger_identity_v1();
                let entered = Cell::new(false);
                let result = owner.with_checked_bf16_nominal_call_v1(
                    inventory,
                    source.root(),
                    source.root(),
                    invalid_block,
                    source.source_call(),
                    &mut budget,
                    |_, _| {
                        entered.set(true);
                        Ok(())
                    },
                );
                assert_eq!(
                    result,
                    Err(QueryError::Unavailable("source root/caller/block differs"))
                );
                assert!(!entered.get());
                assert!(budget.work_ledger_identity_v1() == identity);
                assert_eq!(budget.storage(), floor);
                assert!(budget.work() > 0);
                assert!(budget.peak_storage() > floor);
                assert_eq!(
                    (budget.failed_work(), budget.failed_storage()),
                    (None, None)
                );
                budget.peak_storage()
            };
            let mut measured_entry_frame = None;
            // This ONE lexical callback expression gives the invalid-block case
            // and all three fault cases exactly the same generic F and R=().
            for mode in 0..4 {
                let mut work = Work::new(PROBE_WORK);
                let mut budget = Budget::new(&mut work, limit);
                budget.reserve_storage(floor)?;
                let identity = budget.work_ledger_identity_v1();
                let entered = Cell::new(0usize);
                let callback_storage = Cell::new(0usize);
                let callback_work = Cell::new(0usize);
                let callback_peak = Cell::new(0usize);
                let block = if mode == 0 {
                    invalid_block
                } else {
                    source.call_block()
                };
                let result = with_nominal_final_ranked_candidate_v1(
                    owner,
                    inventory,
                    source.root(),
                    block,
                    source.source_call(),
                    &mut budget,
                    |view, budget| {
                        entered.set(entered.get() + 1);
                        assert!(budget.work_ledger_identity_v1() == identity);
                        observe_view(view, owner, inventory, source, budget)?;
                        callback_storage.set(budget.storage());
                        callback_work.set(budget.work());
                        callback_peak.set(budget.peak_storage());
                        match mode {
                            0 => panic!("invalid source block reached a final candidate"),
                            1 => {
                                let _ = budget.charge_work(PROBE_WORK + 1);
                            }
                            2 => {
                                let _ = budget.reserve_storage(limit + 1);
                            }
                            _ => budget.release_storage(1)?,
                        }
                        Ok(())
                    },
                );
                assert!(budget.work_ledger_identity_v1() == identity);
                if mode == 0 {
                    assert_eq!(
                        result,
                        Err(QueryError::Unavailable("source root/caller/block differs"))
                    );
                    assert_eq!(entered.get(), 0);
                    assert_eq!(callback_storage.get(), 0);
                    assert_eq!(callback_work.get(), 0);
                    assert_eq!(callback_peak.get(), 0);
                    assert_eq!(budget.storage(), floor);
                    assert!(budget.work() > 0);
                    assert_eq!(
                        (budget.failed_work(), budget.failed_storage()),
                        (None, None)
                    );
                    // N1 refuses at its first source-coordinate join before any
                    // candidate scope. Its unit-result scratch is identical to
                    // the direct query above; the peak difference is therefore
                    // this exact monomorphized entry envelope, not an ABI guess.
                    let frame = budget
                        .peak_storage()
                        .checked_sub(direct_peak)
                        .ok_or(Resource::Arithmetic)?;
                    assert!(frame > 0);
                    measured_entry_frame = Some(frame);
                    continue;
                }
                assert_eq!(entered.get(), 1);
                assert_eq!(result, Err(Resource::Accounting.into()));
                assert_eq!(budget.work(), callback_work.get());
                assert_eq!(budget.peak_storage(), callback_peak.get());
                if mode < 3 {
                    assert_eq!(budget.storage(), floor);
                    assert_eq!(budget.failed_work().is_some(), mode == 1);
                    assert_eq!(budget.failed_storage().is_some(), mode == 2);
                } else {
                    // The inner candidate's damaged reservation is NOT refunded.
                    // The outer entry still owns its intact original+entry floor,
                    // so it refunds ONLY the calibrated entry frame on this Err.
                    let entry_frame = measured_entry_frame
                        .expect("wrapped initial refusal calibrated entry before fault probes");
                    let expected = callback_storage
                        .get()
                        .checked_sub(1)
                        .and_then(|n| n.checked_sub(entry_frame))
                        .ok_or(Resource::Arithmetic)?;
                    assert_eq!(budget.storage(), expected);
                    assert!(budget.storage() > floor);
                    assert_eq!(
                        (budget.failed_work(), budget.failed_storage()),
                        (None, None)
                    );
                }
            }
            Ok(())
        })
    })
}

#[test]
fn fixed_control_frames_fit_the_prepaid_genuine_envelope() {
    use std::mem::size_of;
    assert!(
        2 * size_of::<Budget<'static>>()
            + 2 * size_of::<Work>()
            + 32 * size_of::<Cell<usize>>()
            + 12 * size_of::<Result<()>>()
            + 2048
            <= HEADERS
    );
}

// Actual source-owned API entry, including its initial N1, with generic frames
// larger than either historical fixed query envelope. Test-owned values outside
// the call have their own prepaid header reservation.
fn large_entry_controls(
    owner: &ProductionPreRankedKirOwnerV1,
    source: &CheckedBf16CallInstanceV1<'_>,
    inventory: &CanonicalKirInventoryV1<'_>,
    inventory_storage: usize,
    original: &mut Budget<'_>,
) -> Result<()> {
    with_headers(original, 4 * 8192 + HEADERS, |original| {
        let entry = original.storage();
        let payload = [7u8; 8192];
        let result = with_nominal_final_ranked_candidate_v1(
            owner,
            inventory,
            source.root(),
            source.call_block(),
            source.source_call(),
            original,
            move |view, budget| {
                observe_view(view, owner, inventory, source, budget)?;
                assert_eq!(payload[8191], 7);
                Ok(payload)
            },
        )?;
        assert_eq!(result, [7; 8192]);
        assert_eq!(original.storage(), entry);
        let occurrence = owner
            .semantic_ssa()
            .occurrence_storage()
            .unwrap()
            .retained_storage();
        let floor = owner
            .retained_analysis_storage_v1()
            .checked_add(occurrence)
            .and_then(|n| n.checked_add(inventory_storage))
            .ok_or(Resource::Arithmetic)?;
        let short = floor.checked_sub(1).ok_or(Resource::Arithmetic)?;
        let limit = floor
            .checked_add(PROBE_SCRATCH)
            .ok_or(Resource::Arithmetic)?;
        with_headers(original, PROBE_SCRATCH, |original| {
            // One additional accounting-only ledger; the whole cost remains
            // prepaid on ORIGINAL with unchanged per-probe caps.
            original.charge_work(PROBE_WORK + 128)?;
            let mut work = Work::new(PROBE_WORK);
            let mut budget = Budget::new(&mut work, limit);
            budget.reserve_storage(short)?;
            let entered = Cell::new(false);
            let entered_ref = &entered;
            let payload = [9u8; 8192];
            let result = with_nominal_final_ranked_candidate_v1(
                owner,
                inventory,
                source.root(),
                source.call_block(),
                source.source_call(),
                &mut budget,
                move |_, _| {
                    entered_ref.set(true);
                    Ok(payload)
                },
            );
            assert_eq!(result, Err(Resource::Accounting.into()));
            assert!(!entered.get());
            assert_eq!(budget.storage(), short);
            // New entry frame was really admitted; it did not conceal F-1.
            assert!(budget.peak_storage() > floor);
            assert_eq!(
                (budget.failed_work(), budget.failed_storage()),
                (None, None)
            );
            Ok(())
        })
    })
}
