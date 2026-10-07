use super::*;
use crate::context_read_leases::StableReadFaultV1;
use alloc::vec;

fn enroll(owner: &mut ContextProducerReadJournalV1, local: u64) -> ContextAllocationReferenceV1 {
    owner
        .enroll_allocation(
            ContextAllocationKeyV1 {
                context_generation: 7,
                local,
            },
            ContextJournalDeviceKeyV1 {
                context_generation: 7,
                local: 1,
            },
            64,
        )
        .unwrap()
}

fn plain() -> (
    ContextProducerReadJournalV1,
    Vec<ContextAllocationReferenceV1>,
) {
    let mut owner = ContextProducerReadJournalV1::new(7, 4, 2, 2).unwrap();
    owner.stable.fault_enrollment_for_test_v1(&[3, 1, 2, 0], 4);
    let references = [10, 20, 30]
        .into_iter()
        .map(|local| enroll(&mut owner, local))
        .collect();
    (owner, references)
}

fn compare(
    owner: &mut ContextProducerReadJournalV1,
    references: &[ContextAllocationReferenceV1],
    stable_only: bool,
    expected: Result<(), Error>,
    validate_accesses: usize,
    retire_accesses: usize,
) {
    let saved = owner.guard_copy_for_test_v1();
    let before = snapshot(owner);
    let storage = owner.guard_owner_storage_v1();
    for baseline in [true, false] {
        owner.reset_access_count_for_test_v1();
        let result = match (stable_only, baseline) {
            (true, true) => owner
                .stable
                .baseline_validate_allocation_retirement_v1(references),
            (true, false) => owner.stable.validate_allocation_retirement(references),
            (false, true) => owner.baseline_validate_allocation_retirement_v1(references),
            (false, false) => owner.validate_allocation_retirement(references),
        };
        assert_eq!(result, expected);
        assert_eq!(owner.guard_accesses_for_test_v1(), validate_accesses);
        assert_eq!(snapshot(owner), before);
        assert_eq!(owner.guard_owner_storage_v1(), storage);
    }
    owner.reset_access_count_for_test_v1();
    let result = if stable_only {
        owner.stable.baseline_retire_allocations_v1(references)
    } else {
        owner.baseline_retire_allocations_v1(references)
    };
    assert_eq!(result, expected);
    assert_eq!(owner.guard_accesses_for_test_v1(), retire_accesses);
    let frozen = snapshot(owner);
    if expected.is_err() {
        assert_eq!(frozen, before);
    }
    assert_eq!(owner.guard_owner_storage_v1(), storage);
    owner.restore_retirement_for_test_v1(&saved, references);
    assert_eq!(snapshot(owner), before);
    let result = if stable_only {
        owner.stable.retire_allocations(references)
    } else {
        owner.retire_allocations(references)
    };
    assert_eq!(result, expected);
    assert_eq!(owner.guard_accesses_for_test_v1(), retire_accesses);
    assert_eq!(snapshot(owner), frozen);
    assert_eq!(owner.guard_owner_storage_v1(), storage);
}

#[test]
fn retirement_shared_owner_scans_precede_journal_roster_checks() {
    for stable in [true, false] {
        for fault in 0..3 {
            let (mut owner, mut references) = plain();
            let (error, accesses) = match fault {
                0 => {
                    references.extend([references[2]; 2]);
                    (Error::RosterCapacity, if stable { 5 } else { 10 })
                }
                1 => {
                    references.swap(0, 1);
                    (Error::NonCanonicalRoster, if stable { 5 } else { 8 })
                }
                _ => {
                    owner
                        .stable
                        .fault_reads_for_test_v1(StableReadFaultV1::ReaderCount {
                            allocation_slot: references[0].slot,
                            value: 1,
                        });
                    owner.stable.fault_enrollment_for_test_v1(&[usize::MAX], 0);
                    (Error::AllocationBusy, 1)
                }
            };
            compare(
                &mut owner,
                &references,
                stable,
                Err(error),
                accesses,
                accesses,
            );
        }
    }
}

#[test]
fn retirement_shared_owner_lookup_errors_precede_pending_and_count_storage() {
    for stable in [true, false] {
        let mut f = Fixture::new(2);
        let mut invalid = f.other;
        invalid.slot = usize::MAX;
        compare(
            &mut f.journal,
            &[f.source, invalid],
            stable,
            Err(Error::InvalidAllocationReference),
            3,
            3,
        );
        f.journal.stable.guard_break_backlink_for_test_v1(f.source);
        f.journal.counts.clear();
        f.journal
            .stable
            .fault_reads_for_test_v1(StableReadFaultV1::TruncateReaders(0));
        compare(
            &mut f.journal,
            &[f.source],
            stable,
            Err(Error::InvalidState),
            2,
            2,
        );
        compare(
            &mut f.journal,
            &[invalid, f.source],
            stable,
            Err(Error::InvalidAllocationReference),
            1,
            1,
        );
    }
}

#[test]
fn retirement_shared_owner_requires_only_reached_count_storage() {
    for stable in [true, false] {
        for count in [1, usize::MAX] {
            let (mut owner, references) = plain();
            owner
                .stable
                .fault_reads_for_test_v1(StableReadFaultV1::TruncateReaders(1));
            owner
                .stable
                .fault_reads_for_test_v1(StableReadFaultV1::ReaderCount {
                    allocation_slot: 0,
                    value: count,
                });
            owner.counts.truncate(1);
            owner.free.extend([usize::MAX; 3]);
            owner.next_incarnation = 0;
            compare(
                &mut owner,
                &references,
                stable,
                Err(Error::AllocationBusy),
                1,
                1,
            );
        }
        let (mut owner, references) = plain();
        owner
            .stable
            .fault_reads_for_test_v1(StableReadFaultV1::TruncateReaders(1));
        owner
            .stable
            .fault_reads_for_test_v1(StableReadFaultV1::NextIncarnation(0));
        owner.counts.truncate(1);
        owner.free.extend([usize::MAX; 3]);
        owner.next_incarnation = 0;
        compare(
            &mut owner,
            &references[..1],
            stable,
            Ok(()),
            if stable { 2 } else { 3 },
            if stable { 3 } else { 6 },
        );
        let (mut owner, _) = plain();
        owner.counts.clear();
        owner
            .stable
            .fault_reads_for_test_v1(StableReadFaultV1::TruncateReaders(0));
        owner.stable.fault_enrollment_for_test_v1(&[usize::MAX], 0);
        compare(&mut owner, &[], stable, Err(Error::InvalidState), 0, 0);
        owner.stable.fault_enrollment_for_test_v1(&[], 0);
        compare(&mut owner, &[], stable, Ok(()), 0, 0);
    }
    let (mut owner, references) = plain();
    owner.counts.truncate(1);
    owner.counts[0] = usize::MAX;
    compare(
        &mut owner,
        &references,
        false,
        Err(Error::AllocationBusy),
        1,
        1,
    );
}

fn set_status(f: &mut Fixture, status: Status) {
    match status {
        Status::Pending => {}
        Status::Success => f
            .journal
            .settle_success(
                f.producer,
                &ContextWriterSuccessEvidenceV1 { writer: f.producer },
            )
            .unwrap(),
        Status::NoEffect => f
            .journal
            .settle_no_effect(
                f.producer,
                &ContextWriterNoEffectEvidenceV1 { writer: f.producer },
            )
            .unwrap(),
        Status::Unknown => f.journal.mark_unknown(f.producer).unwrap(),
    }
}

#[test]
fn retirement_shared_preserves_unselected_live_reads_and_all_producer_statuses() {
    for stable in [true, false] {
        for status in [
            Status::Pending,
            Status::Success,
            Status::NoEffect,
            Status::Unknown,
        ] {
            let mut f = Fixture::new(4);
            let producer = f.acquire(20);
            let request = f.stable_read(f.other);
            let mut lease = [None];
            f.journal
                .acquire_reads(key(21), &[request], &mut lease)
                .unwrap();
            set_status(&mut f, status);
            let unselected = f.journal.lookup_allocation(f.source).unwrap();
            let retired = enroll(&mut f.journal, 30);
            compare(
                &mut f.journal,
                &[retired],
                stable,
                Ok(()),
                if stable { 2 } else { 3 },
                if stable { 3 } else { 6 },
            );
            assert_eq!(f.journal.producer_read_status(producer), Ok(status));
            assert_eq!(f.journal.lookup_producer_read(producer), Ok(f.request));
            assert_eq!(f.journal.lookup_read(lease[0].unwrap()), Ok(request));
            assert_eq!(f.journal.lookup_allocation(f.source), Ok(unselected));
            let replacement = enroll(&mut f.journal, 31);
            assert_eq!(replacement.slot, retired.slot);
            assert_eq!(
                f.journal.lookup_allocation(retired),
                Err(Error::InvalidAllocationReference)
            );
            assert_invariant(&f.journal);
        }
    }
}

#[test]
fn retirement_shared_selected_producer_custody_is_layer_specific() {
    for stable in [true, false] {
        for status in [
            Status::Pending,
            Status::Success,
            Status::NoEffect,
            Status::Unknown,
        ] {
            let mut f = Fixture::new(2);
            f.acquire(20);
            set_status(&mut f, status);
            let pending = matches!(status, Status::Pending | Status::Unknown);
            let (result, validation, retirement) = if stable {
                if pending {
                    (Err(Error::AllocationBusy), 3, 3)
                } else {
                    (Ok(()), 2, 3)
                }
            } else {
                let count = if pending { 2 } else { 1 };
                (Err(Error::AllocationBusy), count, count)
            };
            // Direct access to the private stable field cannot observe producer counts.
            compare(
                &mut f.journal,
                &[f.source],
                stable,
                result,
                validation,
                retirement,
            );
        }
    }
}

#[test]
fn retirement_shared_repeated_validation_has_linear_counted_accesses() {
    for stable in [true, false] {
        for count in [1, 3, 4097] {
            let mut owner = ContextProducerReadJournalV1::new(7, count + 1, 1, 1).unwrap();
            let entries: Vec<_> = (0..count)
                .map(|i| ContextAllocationEnrollmentV1 {
                    key: ContextAllocationKeyV1 {
                        context_generation: 7,
                        local: i as u64 + 1,
                    },
                    device: ContextJournalDeviceKeyV1 {
                        context_generation: 7,
                        local: 1,
                    },
                    byte_extent: 64,
                })
                .collect();
            let mut output = vec![None; count];
            owner.enroll_allocations(&entries, &mut output).unwrap();
            let references: Vec<_> = output.into_iter().map(Option::unwrap).collect();
            compare(
                &mut owner,
                &references,
                stable,
                Ok(()),
                count * if stable { 2 } else { 3 },
                count * if stable { 3 } else { 6 },
            );
            assert_invariant(&owner);
        }
    }
}
