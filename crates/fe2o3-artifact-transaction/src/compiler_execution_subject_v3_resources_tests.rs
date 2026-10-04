use super::*;
use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    sync::Arc,
};

fn boundaries(input: usize, mut run: impl FnMut(&mut Budget<'_>) -> Result<()>) {
    for shortage in 0..3 {
        let floor = input + 13;
        let mut work = Work::new(17 + WORK - usize::from(shortage == 1));
        let mut budget = Budget::new(&mut work, floor + SCRATCH - usize::from(shortage == 2));
        budget.charge_work(17).unwrap();
        budget.reserve_storage(floor).unwrap();
        let address = &budget as *const Budget<'_> as usize;
        let ledger = budget.work_ledger_identity_v1();
        let result = run(&mut budget);
        match shortage {
            0 => result.unwrap(),
            1 => assert!(matches!(result, Err(Failure::Resource(Resource::Work(_))))),
            _ => assert!(matches!(
                result,
                Err(Failure::Resource(Resource::Storage(_)))
            )),
        }
        assert_eq!(&budget as *const Budget<'_> as usize, address);
        assert!(budget.work_ledger_identity_v1() == ledger);
        assert_eq!(budget.storage(), floor);
        assert_eq!(budget.work(), 17 + if shortage == 0 { WORK } else { 8 });
        assert_eq!(
            budget.peak_storage(),
            if shortage == 2 {
                floor
            } else {
                floor + SCRATCH
            }
        );
        assert_eq!(
            budget.failed_storage(),
            if shortage == 2 {
                Some(floor + SCRATCH)
            } else {
                None
            }
        );
        drop(budget);
        assert_eq!(
            work.failed_work(),
            if shortage == 1 { Some(17 + WORK) } else { None }
        );
    }
}

#[test]
fn conditional_subject_v3_decode_and_identity_exact_and_one_short() {
    let bytes = wire();
    boundaries(bytes.len(), |budget| {
        let (subject, charge) = Subject::decode(&bytes, budget)?;
        assert_eq!(charge.0, RETAINED);
        assert_eq!(subject.canonical_bytes(), &bytes);
        Ok(())
    });
    let id = Subject::from_fields(fields()).unwrap().identity();
    boundaries(bytes.len(), |budget| {
        assert!(id.matches_canonical_bytes(&bytes, budget)?);
        Ok(())
    });
}

#[test]
fn conditional_subject_v3_typed_entries_exact_and_one_short() {
    let f = Fixture::new();
    let mut setup_work = Work::new(usize::MAX);
    let mut setup = Budget::new(&mut setup_work, LIMIT);
    f.reserve(&mut setup);
    let receipt = f.publish(&mut setup).unwrap();
    boundaries(handoff_floor(&f.handoff).unwrap(), |budget| {
        let (_, charge) = replay(&f.handoff, budget)?;
        assert_eq!(charge.0, RETAINED);
        Ok(())
    });
    boundaries(handoff_floor(&f.handoff).unwrap(), |budget| {
        let (_, charge) = Subject::from_publication(receipt, &f.handoff, budget)?;
        assert_eq!(charge.0, RETAINED);
        Ok(())
    });
    let lease = f.lease(receipt, &mut setup);
    let (token, charge) = lease.acquire_current_token(&mut setup).unwrap();
    setup.reserve_storage(charge.retained_storage()).unwrap();
    let consumed =
        crate::consume_compiler_module_handoff_with_currentness_v5(&lease, token, &mut setup)
            .unwrap();
    boundaries(consumed.storage().retained_storage(), |budget| {
        let (_, charge) = Subject::from_consumed(&consumed, budget)?;
        assert_eq!(charge.0, RETAINED);
        Ok(())
    });
}

#[test]
fn conditional_subject_v3_spare_and_unselected_backing_must_be_prepaid() {
    let source = outer();
    let mut setup_work = Work::new(WORK);
    let mut setup = Budget::new(&mut setup_work, LIMIT);
    setup
        .reserve_storage(handoff_floor(&source).unwrap())
        .unwrap();
    let (original, storage) = replay(&source, &mut setup).unwrap();
    setup.reserve_storage(storage.retained_storage()).unwrap();
    let bytes = source.canonical_bytes();
    let mut backing = Vec::with_capacity(bytes.len() + 8192);
    backing.extend_from_slice(&[0x55; 31]);
    backing.extend_from_slice(bytes);
    backing.extend_from_slice(&[0x77; 47]);
    let capacity = backing.capacity();
    let handoff = Handoff::decode_shared_vec(Arc::new(backing), 31..31 + bytes.len()).unwrap();
    assert_eq!(handoff.backing_capacity(), capacity);
    assert_eq!(handoff.canonical_bytes(), bytes);
    let floor = capacity + METADATA;
    for paid in [bytes.len() + METADATA, floor - 1] {
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, floor + SCRATCH);
        budget.reserve_storage(paid).unwrap();
        assert!(matches!(
            replay(&handoff, &mut budget),
            Err(Failure::Resource(Resource::Accounting))
        ));
        assert_eq!(budget.work(), 8);
        assert_eq!(budget.storage(), paid);
        assert_eq!(budget.peak_storage(), paid);
    }
    boundaries(floor, |budget| {
        let (subject, storage) = replay(&handoff, budget)?;
        budget.reserve_storage(storage.retained_storage())?;
        assert_eq!(subject.canonical_bytes(), original.canonical_bytes());
        drop(subject);
        budget.release_storage(storage.retained_storage())?;
        Ok(())
    });
}

#[test]
fn conditional_subject_v3_entry_floor_cap_and_precharge_refuse_before_content() {
    for (work_limit, storage_limit, paid, expected_work) in [
        (7, 690 + SCRATCH, 690, 0),
        (WORK, LIMIT + 1, 690, 8),
        (WORK, 690 + SCRATCH, 689, 8),
    ] {
        let mut work = Work::new(work_limit);
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.reserve_storage(paid).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let error = Subject::decode(&[0; 690], &mut budget).unwrap_err();
        if work_limit < 8 {
            assert!(matches!(error, Failure::Resource(Resource::Work(_))));
        } else {
            assert!(matches!(error, Failure::Resource(Resource::Accounting)));
        }
        assert_eq!(budget.work(), expected_work);
        assert_eq!(budget.storage(), paid);
        assert_eq!(budget.peak_storage(), paid);
        assert!(budget.work_ledger_identity_v1() == ledger);
    }
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, SCRATCH - 1);
    // Malformed physical input still pays the fixed codec entry before parsing.
    assert!(matches!(
        Subject::decode(&[], &mut budget),
        Err(Failure::Resource(Resource::Storage(_)))
    ));
    assert_eq!(budget.work(), 8);
    assert_eq!(budget.storage(), 0);
}

#[test]
fn conditional_subject_v3_closed_scope_preserves_prior_terminal_floor_on_all_exits() {
    for outcome in 0..3 {
        let mut work = Work::new(WORK + 17);
        let mut budget = Budget::new(&mut work, 37 + SCRATCH);
        // Opaque caller-owned reservations cannot be refunded by subject scratch.
        budget.reserve_storage(37).unwrap();
        budget.charge_work(17).unwrap();
        assert!(budget.reserve_storage(usize::MAX).is_err());
        assert!(budget.charge_work(usize::MAX).is_err());
        let ledger = budget.work_ledger_identity_v1();
        let result = catch_unwind(AssertUnwindSafe(|| {
            metered(&mut budget, 37, || match outcome {
                0 => Ok(()),
                1 => Err(Failure::HandoffIdentityMismatch),
                _ => panic!("closed subject codec unwind"),
            })
        }));
        match outcome {
            0 => result.unwrap().unwrap(),
            1 => assert!(matches!(
                result.unwrap(),
                Err(Failure::HandoffIdentityMismatch)
            )),
            _ => assert!(result.is_err()),
        }
        assert_eq!(budget.storage(), 37);
        assert_eq!(budget.peak_storage(), 37 + SCRATCH);
        assert_eq!(budget.work(), WORK + 17);
        assert_eq!(budget.failed_storage(), Some(usize::MAX));
        assert!(budget.work_ledger_identity_v1() == ledger);
        drop(budget);
        assert_eq!(work.failed_work(), Some(usize::MAX));
    }
}

#[test]
fn conditional_subject_v3_work_overflow_is_terminal_without_counter_reset() {
    let mut work = Work::new(usize::MAX);
    let mut budget = Budget::new(&mut work, 690 + SCRATCH);
    budget.reserve_storage(690).unwrap();
    budget.charge_work(usize::MAX - 7).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    assert!(matches!(
        Subject::decode(&wire(), &mut budget),
        Err(Failure::Resource(Resource::Work(_)))
    ));
    assert_eq!(budget.work(), usize::MAX - 7);
    assert_eq!(budget.storage(), 690);
    assert_eq!(budget.peak_storage(), 690);
    assert!(budget.work_ledger_identity_v1() == ledger);
    drop(budget);
    assert_eq!(work.failed_work(), Some(usize::MAX));
}

#[test]
fn conditional_subject_v3_repeated_calls_charge_work_and_transfer_storage_once() {
    let bytes = wire();
    let floor = bytes.len() + 13;
    let mut work = Work::new(17 + 2 * WORK);
    let mut budget = Budget::new(&mut work, floor + RETAINED + SCRATCH);
    budget.charge_work(17).unwrap();
    budget.reserve_storage(floor).unwrap();
    let (first, storage) = Subject::decode(&bytes, &mut budget).unwrap();
    assert_eq!(budget.storage(), floor);
    budget.reserve_storage(storage.0).unwrap();
    let (second, second_storage) = Subject::decode(&bytes, &mut budget).unwrap();
    assert_eq!(budget.storage(), floor + RETAINED);
    budget.reserve_storage(second_storage.0).unwrap();
    assert_eq!(first, second);
    assert_eq!(budget.work(), 17 + 2 * WORK);
    drop(first);
    drop(second);
    budget
        .release_storage(storage.0 + second_storage.0)
        .unwrap();
    assert_eq!(budget.storage(), floor);
}
