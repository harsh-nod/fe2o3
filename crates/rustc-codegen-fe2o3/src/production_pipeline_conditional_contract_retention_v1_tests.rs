//! Storage-lifecycle tests only. These arbitrary bytes do not stand in for a
//! checked contract, source-bound proof, protected runtime, or launch authority.
use super::*;
use crate::production_reference_effect_join_v2::{
    conditional::finish_replayed_transport_v2,
    conditional_source_v1::{ConditionalReplayTransportV2, copy_inert_transport_v2},
};
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;

fn copy(bytes: &[u8], budget: &mut Budget<'_>) -> Result<RetainedConditionalContractV1, Error> {
    budget.with_prepaid_scope(0, 1, 1, 0, |budget| {
        RetainedConditionalContractV1::copy_unreserved(bytes, budget)
    })
}

#[test]
fn conditional_contract_copy_returns_unreserved_storage_preserving_work_and_peak() {
    let mut work = Work::new(100);
    let mut budget = Budget::new(&mut work, 100);
    budget.reserve_storage(7).unwrap();
    let bytes = copy(&[1, 2, 3], &mut budget).unwrap();
    assert_eq!(bytes.bytes, [1, 2, 3]);
    assert_eq!(budget.storage(), 7);
    assert_eq!(budget.work(), 5);
    assert_eq!(budget.peak_storage(), 7 + bytes.retained_storage_v1());
    budget.reserve_storage(bytes.retained_storage_v1()).unwrap();
    assert_eq!(budget.storage(), budget.peak_storage());
}

#[test]
fn conditional_contract_first_replay_transfers_only_duplicate_arena_charge() {
    let mut work = Work::new(100);
    let mut budget = Budget::new(&mut work, 100);
    budget.reserve_storage(7).unwrap();
    let bytes = copy(&[1, 2, 3], &mut budget).unwrap();
    let retained = bytes.retained_storage_v1();
    budget.reserve_storage(retained + 11).unwrap();
    let bytes = bytes.finish_replay_v1(None, 11, 7, &mut budget).unwrap();
    assert_eq!(bytes.bytes, [1, 2, 3]);
    assert_eq!(budget.storage(), 7 + retained);
    assert_eq!(budget.peak_storage(), 7 + retained + 11);
    drop(bytes);
    budget.release_storage(retained).unwrap();
    assert_eq!(budget.storage(), 7);
}

#[test]
fn conditional_contract_repeated_replay_keeps_both_copies_live_until_postchecks() {
    let mut work = Work::new(100);
    let mut budget = Budget::new(&mut work, 100);
    budget.reserve_storage(7).unwrap();
    let old = copy(&[1, 2, 3], &mut budget).unwrap();
    let retained = old.retained_storage_v1();
    budget.reserve_storage(retained).unwrap();
    let floor = budget.storage();
    let new = copy(&[1, 2, 3], &mut budget).unwrap();
    budget.reserve_storage(retained + 11).unwrap();
    let new = new
        .finish_replay_v1(Some(old), 11, floor, &mut budget)
        .unwrap();
    assert_eq!(new.bytes, [1, 2, 3]);
    assert_eq!(budget.storage(), floor);
    assert_eq!(budget.peak_storage(), 7 + 2 * retained + 11);
    assert_eq!(budget.work(), 14);
}

#[test]
fn conditional_contract_changed_replay_is_terminal_without_refunding_live_account() {
    let mut work = Work::new(100);
    let mut budget = Budget::new(&mut work, 100);
    budget.reserve_storage(7).unwrap();
    let old = copy(&[1, 2, 3], &mut budget).unwrap();
    let retained = old.retained_storage_v1();
    budget.reserve_storage(retained).unwrap();
    let floor = budget.storage();
    let new = copy(&[1, 2, 4], &mut budget).unwrap();
    budget.reserve_storage(retained + 11).unwrap();
    let before = budget.storage();
    assert!(matches!(
        new.finish_replay_v1(Some(old), 11, floor, &mut budget),
        Err(Error::Mismatch("retained conditional contract changed"))
    ));
    assert_eq!(budget.storage(), before);
    // The consumed root's enclosing phase owns this terminal cleanup.
    budget.release_storage(before).unwrap();
    assert_eq!(budget.storage(), 0);
    assert_eq!(budget.work(), 14);
}

#[test]
fn conditional_contract_unpaid_and_extra_reservations_cannot_commit() {
    for extra in [-1isize, 1] {
        let mut work = Work::new(100);
        let mut budget = Budget::new(&mut work, 100);
        budget.reserve_storage(7).unwrap();
        let next = copy(&[1, 2, 3], &mut budget).unwrap();
        budget
            .reserve_storage(
                next.retained_storage_v1()
                    .checked_add_signed(extra)
                    .unwrap()
                    + 11,
            )
            .unwrap();
        let before = budget.storage();
        assert!(matches!(
            next.finish_replay_v1(None, 11, 7, &mut budget),
            Err(Error::Resource(Resource::Accounting))
        ));
        assert_eq!(budget.storage(), before);
    }
}

#[test]
fn conditional_contract_one_short_copy_preserves_denial_without_escaping_payload() {
    let charge = std::mem::size_of::<RetainedConditionalContractV1>() + 3;
    let mut work = Work::new(100);
    let mut budget = Budget::new(&mut work, 7 + charge - 1);
    budget.reserve_storage(7).unwrap();
    assert!(matches!(
        copy(&[1, 2, 3], &mut budget),
        Err(Error::Resource(Resource::Storage(_)))
    ));
    assert_eq!(budget.storage(), 7);
    assert_eq!(budget.peak_storage(), 7);
    assert_eq!(budget.failed_storage(), Some(7 + charge));
    assert_eq!(budget.work(), 5);
}

#[test]
fn conditional_contract_one_short_work_does_not_allocate_or_commit() {
    let mut work = Work::new(4);
    let mut budget = Budget::new(&mut work, 100);
    budget.reserve_storage(7).unwrap();
    assert!(matches!(
        copy(&[1, 2, 3], &mut budget),
        Err(Error::Resource(Resource::Work(_)))
    ));
    assert_eq!(budget.storage(), 7);
    assert_eq!(budget.peak_storage(), 7);
    assert_eq!(budget.work(), 1);
    assert_eq!(budget.failed_work(), Some(5));
}

#[test]
fn conditional_contract_unwind_releases_scratch_not_original_floor() {
    let mut work = Work::new(100);
    let mut budget = Budget::new(&mut work, 100);
    budget.reserve_storage(7).unwrap();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _: Result<(), Error> = budget.with_prepaid_scope(7, 1, 1, 0, |budget| {
            let _bytes = RetainedConditionalContractV1::copy_unreserved(&[1, 2, 3], budget)?;
            panic!("post-projection rejection");
        });
    }));
    assert!(result.is_err());
    assert_eq!(budget.storage(), 7);
    assert_eq!(budget.work(), 5);
    assert_eq!(
        budget.peak_storage(),
        7 + std::mem::size_of::<RetainedConditionalContractV1>() + 3
    );
}

#[test]
fn conditional_contract_bounds_fail_before_copy_allocation() {
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, usize::MAX);
    for bytes in [&[][..], &vec![0; MAX_CONDITIONAL_INVOCATION_BYTES_V1 + 1]] {
        assert!(matches!(copy(bytes, &mut budget), Err(Error::Mismatch(_))));
    }
    assert_eq!(budget.work(), 2);
    assert_eq!(budget.peak_storage(), 0);
    assert_eq!(budget.storage(), 0);
}

fn inert_transport(tag: u8, budget: &mut Budget<'_>) -> ConditionalReplayTransportV2 {
    copy_inert_transport_v2(
        &[tag; 3],
        std::iter::empty(),
        fe2o3_verifier::InertFunctionalRefinementReceiptSignatureV2::from_untrusted_parts(
            [0; fe2o3_functional_proof::FUNCTIONAL_REFINEMENT_RECEIPT_WIRE_BYTES_V2],
            [0; 32],
        ),
        budget,
    )
    .unwrap()
}

#[test]
fn conditional_transport_finish_preserves_old_and_new_charges_through_comparison() {
    for previous in [false, true] {
        let mut work = Work::new(1_000_000);
        let mut budget = Budget::new(&mut work, 1_000_000);
        budget.reserve_storage(7).unwrap();
        let old = previous.then(|| {
            let contract = copy(&[1, 2, 3], &mut budget).unwrap();
            budget
                .reserve_storage(contract.retained_storage_v1())
                .unwrap();
            contract
        });
        let old_transport = previous.then(|| inert_transport(4, &mut budget));
        let floor = budget.storage();
        let next = copy(&[1, 2, 3], &mut budget).unwrap();
        let contract_storage = next.retained_storage_v1();
        budget.reserve_storage(contract_storage + 11).unwrap();
        let transport = inert_transport(4, &mut budget);
        let transport_storage = transport.retained_storage_v2();
        assert_eq!(
            budget.storage(),
            floor + contract_storage + transport_storage + 11
        );
        let (next, transport) = finish_replayed_transport_v2(
            next,
            old,
            Some(transport),
            old_transport,
            11,
            floor,
            &mut budget,
        )
        .unwrap();
        assert_eq!(budget.storage(), 7 + contract_storage + transport_storage);
        assert_eq!(next.observed_bytes(), &[1, 2, 3]);
        assert_eq!(transport.as_ref().unwrap().cpu_input_v1(), &[4; 3]);
        drop((next, transport));
        budget
            .release_storage(contract_storage + transport_storage)
            .unwrap();
        assert_eq!(budget.storage(), 7);
    }
}

#[test]
fn conditional_transport_finish_comparison_or_contract_failure_refunds_neither_previous() {
    for changed_transport in [false, true] {
        let mut work = Work::new(1_000_000);
        let mut budget = Budget::new(&mut work, 1_000_000);
        budget.reserve_storage(7).unwrap();
        let old = copy(&[1, 2, 3], &mut budget).unwrap();
        budget.reserve_storage(old.retained_storage_v1()).unwrap();
        let old_transport = inert_transport(4, &mut budget);
        let floor = budget.storage();
        let bytes: &[u8] = if changed_transport {
            &[1, 2, 3]
        } else {
            &[1, 2, 4]
        };
        let next = copy(bytes, &mut budget).unwrap();
        budget
            .reserve_storage(next.retained_storage_v1() + 11)
            .unwrap();
        let transport = inert_transport(if changed_transport { 5 } else { 4 }, &mut budget);
        let before = budget.storage();
        assert!(
            finish_replayed_transport_v2(
                next,
                Some(old),
                Some(transport),
                Some(old_transport),
                11,
                floor,
                &mut budget,
            )
            .is_err()
        );
        assert_eq!(budget.storage(), before);
        // All provisional owners dropped; only the terminal phase may clean up.
        budget.release_storage(before - 7).unwrap();
    }
}

#[test]
fn conditional_transport_finish_new_capture_cannot_mask_missing_previous_custody() {
    let mut work = Work::new(1_000_000);
    let mut budget = Budget::new(&mut work, 1_000_000);
    let old = copy(&[1, 2, 3], &mut budget).unwrap();
    let old_storage = old.retained_storage_v1();
    budget.reserve_storage(old_storage).unwrap();
    let old_transport = inert_transport(4, &mut budget);
    budget.release_storage(1).unwrap();
    let floor = budget.storage();
    let next = copy(&[1, 2, 3], &mut budget).unwrap();
    budget
        .reserve_storage(next.retained_storage_v1() + 11)
        .unwrap();
    let transport = inert_transport(4, &mut budget);
    let before = budget.storage();
    assert!(
        finish_replayed_transport_v2(
            next,
            Some(old),
            Some(transport),
            Some(old_transport),
            11,
            floor,
            &mut budget,
        )
        .is_err()
    );
    assert_eq!(budget.storage(), before);
}

#[test]
fn conditional_transport_finish_default_replay_keeps_existing_capture_reserved() {
    let mut work = Work::new(1_000_000);
    let mut budget = Budget::new(&mut work, 1_000_000);
    budget.reserve_storage(7).unwrap();
    let old = copy(&[1, 2, 3], &mut budget).unwrap();
    budget.reserve_storage(old.retained_storage_v1()).unwrap();
    let old_transport = inert_transport(4, &mut budget);
    let cpu_address = old_transport.cpu_input_v1().as_ptr();
    let floor = budget.storage();
    let next = copy(&[1, 2, 3], &mut budget).unwrap();
    budget
        .reserve_storage(next.retained_storage_v1() + 11)
        .unwrap();
    let (next, transport) = finish_replayed_transport_v2(
        next,
        Some(old),
        None,
        Some(old_transport),
        11,
        floor,
        &mut budget,
    )
    .unwrap();
    assert_eq!(budget.storage(), floor);
    assert_eq!(transport.unwrap().cpu_input_v1().as_ptr(), cpu_address);
    drop(next);
}

#[test]
fn conditional_transport_finish_unpaid_or_extra_capture_storage_cannot_commit() {
    for delta in [-1isize, 1] {
        let mut work = Work::new(1_000_000);
        let mut budget = Budget::new(&mut work, 1_000_000);
        budget.reserve_storage(7).unwrap();
        let next = copy(&[1, 2, 3], &mut budget).unwrap();
        budget
            .reserve_storage(next.retained_storage_v1() + 11)
            .unwrap();
        let transport = inert_transport(4, &mut budget);
        if delta < 0 {
            budget.release_storage(1).unwrap();
        } else {
            budget.reserve_storage(1).unwrap();
        }
        let before = budget.storage();
        assert!(
            finish_replayed_transport_v2(next, None, Some(transport), None, 11, 7, &mut budget,)
                .is_err()
        );
        assert_eq!(budget.storage(), before);
    }
}
