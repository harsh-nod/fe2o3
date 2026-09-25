use super::*;
use crate::context_read_leases::StableReadFaultV1;
use alloc::vec;

fn fixture(
    count: usize,
    unknown: bool,
) -> (
    ContextProducerReadJournalV1,
    ContextWriterReferenceV1,
    Vec<ContextAllocationWriteV1>,
) {
    let mut owner = ContextProducerReadJournalV1::new(7, count + 1, 4, 4).unwrap();
    if count == 3 {
        owner.stable.fault_enrollment_for_test_v1(&[3, 1, 2, 0], 4);
    }
    let device = ContextJournalDeviceKeyV1 {
        context_generation: 7,
        local: 1,
    };
    let entries: Vec<_> = (0..count)
        .map(|i| ContextAllocationEnrollmentV1 {
            key: ContextAllocationKeyV1 {
                context_generation: 7,
                local: 100 + i as u64,
            },
            device,
            byte_extent: 64,
        })
        .collect();
    let mut output = vec![None; count];
    owner.enroll_allocations(&entries, &mut output).unwrap();
    let roster: Vec<_> = output
        .into_iter()
        .map(|allocation| ContextAllocationWriteV1 {
            allocation: allocation.unwrap(),
            device,
            byte_extent: 64,
        })
        .collect();
    let writer = owner.register_writer(key(10)).unwrap();
    owner.begin_write(writer, &roster).unwrap();
    if unknown {
        owner.mark_unknown(writer).unwrap();
    }
    (owner, writer, roster)
}

fn compare(
    owner: &mut ContextProducerReadJournalV1,
    writer: ContextWriterReferenceV1,
    evidence: ContextWriterReferenceV1,
    roster: &[ContextAllocationWriteV1],
    stable_only: bool,
    validate: (Result<(), Error>, usize),
    execute: (Result<(), Error>, usize),
) {
    let saved = owner.guard_copy_for_test_v1();
    let before = snapshot(owner);
    let storage = owner.guard_owner_storage_v1();
    for baseline in [true, false] {
        owner.reset_access_count_for_test_v1();
        let result = match (stable_only, baseline) {
            (true, true) => owner
                .stable
                .baseline_validate_unknown_disposal_v1(writer, roster),
            (true, false) => owner.stable.validate_unknown_disposal(writer, roster),
            (false, true) => owner.baseline_validate_unknown_disposal_v1(writer, roster),
            (false, false) => owner.validate_unknown_disposal(writer, roster),
        };
        assert_eq!(result, validate.0);
        assert_eq!(owner.guard_accesses_for_test_v1(), validate.1);
        assert_eq!(snapshot(owner), before);
        assert_eq!(owner.guard_owner_storage_v1(), storage);
    }
    let evidence = ContextWriterDisposalEvidenceV1 {
        writer: evidence,
        allocations: roster,
    };
    owner.reset_access_count_for_test_v1();
    let result = if stable_only {
        owner.stable.baseline_dispose_unknown_v1(writer, &evidence)
    } else {
        owner.baseline_dispose_unknown_v1(writer, &evidence)
    };
    assert_eq!(result, execute.0);
    assert_eq!(owner.guard_accesses_for_test_v1(), execute.1);
    let frozen = snapshot(owner);
    assert_eq!(owner.guard_owner_storage_v1(), storage);
    if execute.0.is_ok() {
        owner.restore_disposal_for_test_v1(&saved, writer);
    } else {
        assert_eq!(frozen, before);
        owner.reset_access_count_for_test_v1();
    }
    assert_eq!(snapshot(owner), before);
    let result = if stable_only {
        owner.stable.dispose_unknown(writer, &evidence)
    } else {
        owner.dispose_unknown(writer, &evidence)
    };
    assert_eq!(result, execute.0);
    assert_eq!(owner.guard_accesses_for_test_v1(), execute.1);
    assert_eq!(snapshot(owner), frozen);
    assert_eq!(owner.guard_owner_storage_v1(), storage);
}

#[test]
fn disposal_shared_outer_success_retains_repeated_validation_and_linear_accesses() {
    for stable in [true, false] {
        for count in [0, 1, 3, 4097] {
            let (mut owner, writer, roster) = fixture(count, true);
            let validate = 1 + if stable { 5 } else { 7 } * count;
            let execute = if stable {
                5 + 9 * count
            } else {
                6 + 16 * count
            };
            compare(
                &mut owner,
                writer,
                writer,
                &roster,
                stable,
                (Ok(()), validate),
                (Ok(()), execute),
            );
            assert_invariant(&owner);
        }
    }
}

#[test]
fn disposal_shared_outer_unknown_validation_precedes_bad_evidence() {
    for stable in [true, false] {
        for unknown in [true, false] {
            let (mut owner, writer, roster) = fixture(2, unknown);
            let mut evidence = writer;
            evidence.key.kind = ContextWriterKindV1::Synchronous;
            let (validate, execute) = if unknown {
                (
                    (Ok(()), if stable { 11 } else { 15 }),
                    (
                        Err(Error::SettlementEvidenceMismatch),
                        if stable { 12 } else { 27 },
                    ),
                )
            } else {
                let count = if stable { 5 } else { 9 };
                (
                    (Err(Error::InvalidState), count),
                    (Err(Error::InvalidState), count),
                )
            };
            compare(
                &mut owner, writer, evidence, &roster, stable, validate, execute,
            );
        }
    }
}

#[test]
fn disposal_shared_outer_roster_length_follows_unread_guards() {
    for stable in [true, false] {
        for length in [0, 2, 4] {
            let (mut owner, writer, mut roster) = fixture(3, true);
            roster.push(roster[0]);
            roster[0].byte_extent += 1;
            let accesses = 1 + if stable { 2 } else { 4 } * length;
            compare(
                &mut owner,
                writer,
                writer,
                &roster[..length],
                stable,
                (Err(Error::SettlementEvidenceMismatch), accesses),
                (Err(Error::SettlementEvidenceMismatch), accesses),
            );
        }
        for coordinate in 0..4 {
            let (mut owner, writer, roster) = fixture(3, true);
            let mut invalid = writer;
            match coordinate {
                0 => invalid.slot = usize::MAX,
                1 => invalid.key.context_generation += 1,
                2 => invalid.key.local += 1,
                _ => invalid.key.kind = ContextWriterKindV1::Synchronous,
            }
            let accesses = if stable { 7 } else { 13 };
            compare(
                &mut owner,
                invalid,
                writer,
                &roster,
                stable,
                (Err(Error::InvalidReference), accesses),
                (Err(Error::InvalidReference), accesses),
            );
        }
    }
}

#[test]
fn disposal_shared_outer_unread_guard_precedes_writer_and_evidence_checks() {
    for stable in [true, false] {
        let (mut owner, mut writer, roster) = fixture(3, true);
        owner
            .stable
            .fault_reads_for_test_v1(StableReadFaultV1::ReaderCount {
                allocation_slot: roster[1].allocation.slot,
                value: 1,
            });
        writer.slot = usize::MAX;
        compare(
            &mut owner,
            writer,
            writer,
            &roster,
            stable,
            (Err(Error::AllocationBusy), 4),
            (Err(Error::AllocationBusy), 4),
        );
    }
}

#[test]
fn disposal_shared_outer_requires_only_reached_count_storage() {
    for stable in [true, false] {
        for busy in [true, false] {
            let (mut owner, writer, mut roster) = fixture(3, true);
            owner.counts.truncate(1);
            owner
                .stable
                .fault_reads_for_test_v1(StableReadFaultV1::TruncateReaders(1));
            let (error, accesses) = if busy {
                owner
                    .stable
                    .fault_reads_for_test_v1(StableReadFaultV1::ReaderCount {
                        allocation_slot: 0,
                        value: 1,
                    });
                (Error::AllocationBusy, 2)
            } else {
                roster[0].allocation.slot = usize::MAX;
                owner.counts.clear();
                owner
                    .stable
                    .fault_reads_for_test_v1(StableReadFaultV1::TruncateReaders(0));
                (Error::InvalidAllocationReference, 1)
            };
            compare(
                &mut owner,
                writer,
                writer,
                &roster,
                stable,
                (Err(error), accesses),
                (Err(error), accesses),
            );
        }
        let (mut owner, writer, roster) = fixture(1, true);
        owner.counts.truncate(1);
        owner
            .stable
            .fault_reads_for_test_v1(StableReadFaultV1::TruncateReaders(1));
        owner.next_incarnation = 0;
        owner.free[0] = usize::MAX;
        compare(
            &mut owner,
            writer,
            writer,
            &roster,
            stable,
            (Ok(()), if stable { 6 } else { 8 }),
            (Ok(()), if stable { 14 } else { 22 }),
        );
    }
}

#[test]
fn disposal_shared_preserves_unrelated_leases_and_all_retained_producer_statuses() {
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
            match status {
                Status::Pending => {}
                Status::Unknown => f.journal.mark_unknown(f.producer).unwrap(),
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
            }
            let allocation = f
                .journal
                .enroll_allocation(
                    ContextAllocationKeyV1 {
                        context_generation: 7,
                        local: 30,
                    },
                    request.device,
                    request.byte_extent,
                )
                .unwrap();
            let writer = f.journal.register_writer(key(30)).unwrap();
            if matches!(status, Status::Success | Status::NoEffect) {
                assert_eq!(writer.slot, f.producer.slot);
            }
            let roster = [f.member(allocation)];
            f.journal.begin_write(writer, &roster).unwrap();
            f.journal.mark_unknown(writer).unwrap();
            let retained = f.journal.lookup_allocation(f.source).unwrap();
            compare(
                &mut f.journal,
                writer,
                writer,
                &roster,
                stable,
                (Ok(()), if stable { 6 } else { 8 }),
                (Ok(()), if stable { 14 } else { 22 }),
            );
            assert_eq!(f.journal.lookup_read(lease[0].unwrap()), Ok(request));
            assert_eq!(f.journal.lookup_producer_read(producer), Ok(f.request));
            assert_eq!(f.journal.producer_read_status(producer), Ok(status));
            assert_eq!(f.journal.lookup_allocation(f.source), Ok(retained));
            assert_eq!(
                f.journal.lookup_allocation(allocation),
                Err(Error::InvalidAllocationReference)
            );
            assert_invariant(&f.journal);
        }
    }
}

#[test]
fn disposal_shared_producer_retention_is_enforced_only_by_the_producer_layer() {
    for stable in [true, false] {
        let mut f = Fixture::new(2);
        f.acquire(20);
        f.journal.mark_unknown(f.producer).unwrap();
        let roster = [f.member(f.source)];
        let (validate, execute) = if stable {
            ((Ok(()), 6), (Ok(()), 14))
        } else {
            (
                (Err(Error::AllocationBusy), 2),
                (Err(Error::AllocationBusy), 2),
            )
        };
        // Private stable access cannot see the containing producer owner's counts.
        compare(
            &mut f.journal,
            f.producer,
            f.producer,
            &roster,
            stable,
            validate,
            execute,
        );
        if !stable {
            assert_invariant(&f.journal);
        }
    }
}
