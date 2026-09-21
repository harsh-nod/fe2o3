use super::*;
use alloc::string::String;

type Error = ContextVersionJournalErrorV1;
type Status = ContextProducerReadStatusV1;

fn key(local: u64) -> ContextWriterKeyV1 {
    ContextWriterKeyV1 {
        context_generation: 7,
        local,
        kind: ContextWriterKindV1::Submission,
    }
}

struct Fixture {
    journal: ContextProducerReadJournalV1,
    source: ContextAllocationReferenceV1,
    other: ContextAllocationReferenceV1,
    producer: ContextWriterReferenceV1,
    request: ContextProducerReadV1,
}

impl Fixture {
    fn new(reads: usize) -> Self {
        let mut journal = ContextProducerReadJournalV1::new(7, 4, 4, reads).unwrap();
        let device = ContextJournalDeviceKeyV1 {
            context_generation: 7,
            local: 1,
        };
        let source = journal
            .enroll_allocation(
                ContextAllocationKeyV1 {
                    context_generation: 7,
                    local: 1,
                },
                device,
                64,
            )
            .unwrap();
        let other = journal
            .enroll_allocation(
                ContextAllocationKeyV1 {
                    context_generation: 7,
                    local: 2,
                },
                device,
                64,
            )
            .unwrap();
        let producer = journal.register_writer(key(10)).unwrap();
        journal
            .begin_write(
                producer,
                &[ContextAllocationWriteV1 {
                    allocation: source,
                    device,
                    byte_extent: 64,
                }],
            )
            .unwrap();
        let state = journal.lookup_allocation(source).unwrap();
        let request = ContextProducerReadV1 {
            read: ContextAllocationReadV1 {
                allocation: source,
                device,
                byte_extent: 64,
                byte_offset: 3,
                byte_len: 17,
                attempt_epoch: state.attempt_epoch,
                content_lineage: state.content_lineage,
            },
            producer,
        };
        Self {
            journal,
            source,
            other,
            producer,
            request,
        }
    }

    fn acquire(&mut self, consumer: u64) -> ContextProducerReadReferenceV1 {
        let mut output = [None];
        self.journal
            .acquire_producer_reads(key(consumer), &[self.request], &mut output)
            .unwrap();
        output[0].unwrap()
    }

    fn release(&mut self, reference: ContextProducerReadReferenceV1) {
        self.journal
            .release_producer_reads(
                reference.consumer,
                &[reference],
                &ContextReadQuiescenceEvidenceV1 {
                    consumer: reference.consumer,
                },
            )
            .unwrap();
    }

    fn member(&self, allocation: ContextAllocationReferenceV1) -> ContextAllocationWriteV1 {
        let state = self.journal.lookup_allocation(allocation).unwrap();
        ContextAllocationWriteV1 {
            allocation,
            device: state.device,
            byte_extent: state.byte_extent,
        }
    }

    fn stable_read(&self, allocation: ContextAllocationReferenceV1) -> ContextAllocationReadV1 {
        let state = self.journal.lookup_allocation(allocation).unwrap();
        ContextAllocationReadV1 {
            allocation,
            device: state.device,
            byte_extent: state.byte_extent,
            byte_offset: 0,
            byte_len: 8,
            attempt_epoch: state.attempt_epoch,
            content_lineage: state.content_lineage,
        }
    }
}

fn snapshot(journal: &ContextProducerReadJournalV1) -> String {
    journal.reset_access_count_for_test_v1();
    alloc::format!("{journal:?}")
}

#[test]
fn begin_checks_combined_unread_roster_before_raw_writer_or_destination_faults() {
    for reader_kind in 0..3 {
        for protected_position in 0..3 {
            for raw_fault in 0..5 {
                let mut f = Fixture::new(4);
                if reader_kind != 0 {
                    f.acquire(20);
                }
                f.journal
                    .settle_no_effect(
                        f.producer,
                        &ContextWriterNoEffectEvidenceV1 { writer: f.producer },
                    )
                    .unwrap();
                if reader_kind != 1 {
                    let request = f.stable_read(f.source);
                    f.journal
                        .acquire_reads(key(21), &[request], &mut [None])
                        .unwrap();
                }
                let mut writer = f.journal.register_writer(key(30)).unwrap();
                let mut members = alloc::vec![f.member(f.other); 3];
                members[protected_position] = f.member(f.source);
                match raw_fault {
                    0 => writer.slot = usize::MAX,
                    1 => members[0].device.local += 1,
                    2 => members[0].byte_extent = 0,
                    3 => members.extend_from_slice(&[f.member(f.other); 2]),
                    // The repeated other allocation already makes this roster noncanonical.
                    _ => {}
                }
                let before = snapshot(&f.journal);
                assert_eq!(
                    f.journal.begin_write(writer, &members),
                    Err(Error::AllocationBusy)
                );
                assert_eq!(snapshot(&f.journal), before);
            }
        }
    }
}

#[test]
fn begin_unread_faults_follow_caller_order_before_raw_preflight() {
    for protected_position in 0..3 {
        for invalid_position in 0..3 {
            if protected_position == invalid_position {
                continue;
            }
            let mut f = Fixture::new(2);
            f.acquire(20);
            let mut writer = f.journal.register_writer(key(30)).unwrap();
            writer.slot = usize::MAX;
            let mut members = [f.member(f.other); 3];
            members[protected_position] = f.member(f.source);
            members[invalid_position].allocation.slot = usize::MAX;
            let expected = if invalid_position < protected_position {
                Error::InvalidAllocationReference
            } else {
                Error::AllocationBusy
            };
            let before = snapshot(&f.journal);
            assert_eq!(f.journal.begin_write(writer, &members), Err(expected));
            assert_eq!(snapshot(&f.journal), before);
        }
    }
}

#[test]
fn producer_settlement_preserves_custody_and_never_promotes_failure() {
    for status in [
        Status::Pending,
        Status::Success,
        Status::NoEffect,
        Status::Unknown,
    ] {
        let mut f = Fixture::new(4);
        let a = f.acquire(20);
        let b = f.acquire(21);
        match status {
            Status::Pending => (),
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
        for reference in [a, b] {
            assert_eq!(f.journal.producer_read_status(reference), Ok(status));
            assert_eq!(f.journal.lookup_producer_read(reference), Ok(f.request));
        }
        assert_eq!(f.journal.reader_count(f.source), Ok(2));
        assert_eq!(f.journal.remaining_read_slots(), 2);
        let writer = f.journal.register_writer(key(30)).unwrap();
        let before = snapshot(&f.journal);
        assert_eq!(
            f.journal.begin_write(writer, &[f.member(f.source)]),
            Err(Error::AllocationBusy)
        );
        assert_eq!(snapshot(&f.journal), before);
        assert_eq!(
            f.journal.validate_allocation_retirement(&[f.source]),
            Err(Error::AllocationBusy)
        );
        assert_eq!(
            f.journal.retire_allocations(&[f.source]),
            Err(Error::AllocationBusy)
        );
        f.release(b);
        assert_eq!(f.journal.reader_count(f.source), Ok(1));
        f.release(a);
        assert_eq!(f.journal.reader_count(f.source), Ok(0));
        assert_eq!(f.journal.retained_read_count(), 0);
        assert_eq!(
            f.journal.lookup_producer_read(a),
            Err(Error::InvalidReference)
        );
    }
}

#[test]
fn resolved_reservations_survive_producer_writer_slot_reuse() {
    for success in [true, false] {
        let mut f = Fixture::new(2);
        let reference = f.acquire(20);
        if success {
            f.journal
                .settle_success(
                    f.producer,
                    &ContextWriterSuccessEvidenceV1 { writer: f.producer },
                )
                .unwrap();
        } else {
            f.journal
                .settle_no_effect(
                    f.producer,
                    &ContextWriterNoEffectEvidenceV1 { writer: f.producer },
                )
                .unwrap();
        }
        let replacement = f.journal.register_writer(key(30)).unwrap();
        assert_eq!(replacement.slot, f.producer.slot);
        f.journal
            .begin_write(replacement, &[f.member(f.other)])
            .unwrap();
        assert!(f.journal.lookup_writer(f.producer).is_err());
        assert_eq!(
            f.journal.producer_read_status(reference),
            Ok(if success {
                Status::Success
            } else {
                Status::NoEffect
            })
        );
        f.release(reference);
    }
}

#[test]
fn unknown_disposal_waits_for_all_consumers_without_asserting_data_success() {
    let mut f = Fixture::new(2);
    let a = f.acquire(20);
    let b = f.acquire(21);
    f.journal.mark_unknown(f.producer).unwrap();
    let members = [f.member(f.source)];
    let evidence = ContextWriterDisposalEvidenceV1 {
        writer: f.producer,
        allocations: &members,
    };
    assert_eq!(
        f.journal.validate_unknown_disposal(f.producer, &members),
        Err(Error::AllocationBusy)
    );
    assert_eq!(
        f.journal.dispose_unknown(f.producer, &evidence),
        Err(Error::AllocationBusy)
    );
    assert!(
        f.journal
            .settle_success(
                f.producer,
                &ContextWriterSuccessEvidenceV1 { writer: f.producer }
            )
            .is_err()
    );
    f.release(a);
    assert_eq!(
        f.journal.dispose_unknown(f.producer, &evidence),
        Err(Error::AllocationBusy)
    );
    f.release(b);
    f.journal.dispose_unknown(f.producer, &evidence).unwrap();
    assert!(f.journal.lookup_allocation(f.source).is_err());
}

#[test]
fn stable_and_producer_arenas_share_one_total_budget_and_exclusion_count() {
    for producer_first in [true, false] {
        let mut f = Fixture::new(2);
        let read = f.stable_read(f.other);
        let mut stable = [None];
        let reference;
        if producer_first {
            reference = f.acquire(20);
            f.journal
                .acquire_reads(key(21), &[read], &mut stable)
                .unwrap();
        } else {
            f.journal
                .acquire_reads(key(21), &[read], &mut stable)
                .unwrap();
            reference = f.acquire(20);
        }
        assert_eq!(f.journal.remaining_read_slots(), 0);
        assert_eq!(f.journal.retained_read_count(), 2);
        assert_eq!(f.journal.retained_producer_read_count(), 1);
        let before = snapshot(&f.journal);
        let mut extra_stable = [None];
        let mut extra_future = [None];
        assert_eq!(
            f.journal.acquire_reads(key(22), &[read], &mut extra_stable),
            Err(Error::MemberCapacity)
        );
        assert_eq!(
            f.journal
                .acquire_producer_reads(key(22), &[f.request], &mut extra_future),
            Err(Error::MemberCapacity)
        );
        assert_eq!(snapshot(&f.journal), before);
        assert_eq!(extra_stable, [None]);
        assert_eq!(extra_future, [None]);
        f.release(reference);
        f.journal
            .release_reads(
                key(21),
                &[stable[0].unwrap()],
                &ContextReadQuiescenceEvidenceV1 { consumer: key(21) },
            )
            .unwrap();
        assert_eq!(f.journal.remaining_read_slots(), 2);
    }
}

#[test]
fn stable_read_can_coexist_after_success_without_migrating_reservation() {
    let mut f = Fixture::new(2);
    let reference = f.acquire(20);
    f.journal
        .settle_success(
            f.producer,
            &ContextWriterSuccessEvidenceV1 { writer: f.producer },
        )
        .unwrap();
    let mut output = [None];
    f.journal
        .acquire_reads(key(21), &[f.stable_read(f.source)], &mut output)
        .unwrap();
    assert_eq!(f.journal.reader_count(f.source), Ok(2));
    assert_eq!(f.journal.retained_producer_read_count(), 1);
    f.release(reference);
    assert_eq!(f.journal.reader_count(f.source), Ok(1));
    let next = f.journal.register_writer(key(30)).unwrap();
    assert_eq!(
        f.journal.begin_write(next, &[f.member(f.source)]),
        Err(Error::AllocationBusy)
    );
    f.journal
        .release_reads(
            key(21),
            &[output[0].unwrap()],
            &ContextReadQuiescenceEvidenceV1 { consumer: key(21) },
        )
        .unwrap();
    f.journal.begin_write(next, &[f.member(f.source)]).unwrap();
}

#[test]
fn admission_rejects_changed_identity_epoch_range_and_nonpending_producer_atomically() {
    let mut f = Fixture::new(4);
    let original = f.request;
    let mutations: &[fn(&mut ContextProducerReadV1)] = &[
        |r| r.producer.key.local += 1,
        |r| r.producer.slot += 1,
        |r| r.producer.key.context_generation += 1,
        |r| r.producer.key.kind = ContextWriterKindV1::Synchronous,
        |r| r.read.allocation.key.local += 1,
        |r| r.read.allocation.slot += 1,
        |r| r.read.device.local += 1,
        |r| r.read.byte_extent += 1,
        |r| r.read.byte_len = 0,
        |r| r.read.byte_offset = u64::MAX,
        |r| r.read.byte_len = 65,
        |r| r.read.attempt_epoch += 1,
        |r| r.read.content_lineage = r.read.attempt_epoch,
    ];
    for mutate in mutations {
        let mut bad = original;
        mutate(&mut bad);
        let before = snapshot(&f.journal);
        let mut output = [None];
        assert!(
            f.journal
                .acquire_producer_reads(key(20), &[bad], &mut output)
                .is_err()
        );
        assert_eq!(snapshot(&f.journal), before);
        assert_eq!(output, [None]);
    }
    for consumer in [
        key(0),
        key(10),
        key(9),
        key(u64::MAX),
        ContextWriterKeyV1 {
            context_generation: 8,
            ..key(20)
        },
        ContextWriterKeyV1 {
            kind: ContextWriterKindV1::Synchronous,
            ..key(20)
        },
    ] {
        let before = snapshot(&f.journal);
        assert!(
            f.journal
                .acquire_producer_reads(consumer, &[original], &mut [None])
                .is_err()
        );
        assert_eq!(snapshot(&f.journal), before);
    }
    f.journal.mark_unknown(f.producer).unwrap();
    assert_eq!(
        f.journal.validate_producer_read(&original),
        Err(Error::AllocationBusy)
    );
}

#[test]
fn roster_rejection_and_release_failures_never_partially_change_custody() {
    let mut f = Fixture::new(4);
    let first = f.request;
    let second = ContextProducerReadV1 {
        read: ContextAllocationReadV1 {
            byte_offset: 4,
            ..first.read
        },
        ..first
    };
    for requests in [
        alloc::vec![first, first],
        alloc::vec![second, first],
        alloc::vec![
            first,
            ContextProducerReadV1 {
                producer: ContextWriterReferenceV1 {
                    slot: 3,
                    ..first.producer
                },
                ..second
            }
        ],
    ] {
        let before = snapshot(&f.journal);
        let mut output = [None, None];
        assert!(
            f.journal
                .acquire_producer_reads(key(20), &requests, &mut output)
                .is_err()
        );
        assert_eq!(snapshot(&f.journal), before);
        assert_eq!(output, [None, None]);
    }
    let mut output = [None, None];
    f.journal
        .acquire_producer_reads(key(20), &[first, second], &mut output)
        .unwrap();
    let [a, b] = output.map(Option::unwrap);
    for refs in [
        alloc::vec![a, a],
        alloc::vec![b, a],
        alloc::vec![
            a,
            ContextProducerReadReferenceV1 {
                incarnation: b.incarnation + 1,
                ..b
            }
        ],
    ] {
        let before = snapshot(&f.journal);
        assert!(
            f.journal
                .release_producer_reads(
                    key(20),
                    &refs,
                    &ContextReadQuiescenceEvidenceV1 { consumer: key(20) }
                )
                .is_err()
        );
        assert_eq!(snapshot(&f.journal), before);
    }
    assert_eq!(
        f.journal.release_producer_reads(
            key(20),
            &[a, b],
            &ContextReadQuiescenceEvidenceV1 { consumer: key(21) }
        ),
        Err(Error::SettlementEvidenceMismatch)
    );
    f.journal
        .release_producer_reads(
            key(20),
            &[a, b],
            &ContextReadQuiescenceEvidenceV1 { consumer: key(20) },
        )
        .unwrap();
    let reused = f.acquire(21);
    assert!(reused.slot == a.slot || reused.slot == b.slot);
    assert!(reused.incarnation > b.incarnation);
    assert_eq!(
        f.journal.lookup_producer_read(a),
        Err(Error::InvalidReference)
    );
    assert_eq!(
        f.journal.lookup_producer_read(b),
        Err(Error::InvalidReference)
    );
}

#[test]
fn exhausted_incarnation_does_not_wrap_or_change_output() {
    let mut f = Fixture::new(2);
    for next in [0, u64::MAX] {
        f.journal.next_incarnation = next;
        let before = snapshot(&f.journal);
        let mut output = [None];
        assert_eq!(
            f.journal
                .acquire_producer_reads(key(20), &[f.request], &mut output),
            Err(Error::EpochExhausted)
        );
        assert_eq!(snapshot(&f.journal), before);
        assert_eq!(output, [None]);
    }
}

fn assert_invariant(journal: &ContextProducerReadJournalV1) {
    let mut free = alloc::vec![false; journal.reservations.len()];
    for &slot in &journal.free {
        assert!(slot < free.len());
        assert!(!free[slot]);
        free[slot] = true;
    }
    let mut counts = alloc::vec![0; journal.counts.len()];
    let mut incarnations = alloc::collections::BTreeSet::new();
    for (slot, entry) in journal.reservations.iter().enumerate() {
        assert_eq!(entry.is_none(), free[slot]);
        if let Some(entry) = entry {
            assert_eq!(entry.reference.slot, slot);
            assert!((1..journal.next_incarnation).contains(&entry.reference.incarnation));
            assert!(incarnations.insert(entry.reference.incarnation));
            assert_eq!(
                entry.reference.consumer.context_generation,
                journal.context_generation()
            );
            assert_eq!(
                entry.reference.consumer.kind,
                ContextWriterKindV1::Submission
            );
            assert!(entry.request.producer.key.local < entry.reference.consumer.local);
            assert_eq!(
                journal.lookup_producer_read(entry.reference),
                Ok(entry.request)
            );
            counts[entry.request.read.allocation.slot] += 1;
        }
    }
    assert_eq!(counts, journal.counts);
    assert_eq!(
        counts.iter().sum::<usize>(),
        journal.retained_producer_read_count()
    );
    assert!(journal.retained_read_count() <= journal.reservations.len());
    assert_eq!(
        journal.remaining_read_slots() + journal.retained_read_count(),
        journal.reservations.len()
    );
}

fn permutations(values: &mut [usize], offset: usize, result: &mut Vec<Vec<usize>>) {
    if offset == values.len() {
        result.push(values.to_vec());
    } else {
        for index in offset..values.len() {
            values.swap(index, offset);
            permutations(values, offset + 1, result);
            values.swap(index, offset);
        }
    }
}

#[test]
fn all_small_fanout_release_and_settlement_interleavings_preserve_partition() {
    for count in 1..=4 {
        let mut orders = Vec::new();
        permutations(&mut (0..count).collect::<Vec<_>>(), 0, &mut orders);
        for order in orders {
            for settle_at in 0..=count {
                for status in [Status::Success, Status::NoEffect, Status::Unknown] {
                    let mut f = Fixture::new(count);
                    let storage = [
                        (
                            f.journal.reservations.as_ptr() as usize,
                            f.journal.reservations.capacity(),
                        ),
                        (f.journal.free.as_ptr() as usize, f.journal.free.capacity()),
                        (
                            f.journal.counts.as_ptr() as usize,
                            f.journal.counts.capacity(),
                        ),
                    ];
                    let mut references = Vec::new();
                    for index in 0..count {
                        references.push(f.acquire(20 + index as u64));
                        assert_invariant(&f.journal);
                    }
                    for step in 0..=count {
                        if step == settle_at {
                            match status {
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
                                Status::Pending => unreachable!(),
                            }
                            assert_invariant(&f.journal);
                        }
                        if step < count {
                            let reference = references[order[step]];
                            assert_eq!(
                                f.journal.producer_read_status(reference),
                                Ok(if step < settle_at {
                                    Status::Pending
                                } else {
                                    status
                                })
                            );
                            f.release(reference);
                            assert_invariant(&f.journal);
                        }
                    }
                    assert_eq!(f.journal.retained_read_count(), 0);
                    assert_eq!(
                        storage,
                        [
                            (
                                f.journal.reservations.as_ptr() as usize,
                                f.journal.reservations.capacity()
                            ),
                            (f.journal.free.as_ptr() as usize, f.journal.free.capacity()),
                            (
                                f.journal.counts.as_ptr() as usize,
                                f.journal.counts.capacity()
                            ),
                        ]
                    );
                }
            }
        }
    }
}

#[test]
fn independent_producers_in_one_roster_settle_without_crossing_lineages() {
    let mut f = Fixture::new(2);
    let other_producer = f.journal.register_writer(key(11)).unwrap();
    f.journal
        .begin_write(other_producer, &[f.member(f.other)])
        .unwrap();
    let other_request = ContextProducerReadV1 {
        read: f.stable_read(f.other),
        producer: other_producer,
    };
    let mut output = [None, None];
    f.journal
        .acquire_producer_reads(key(20), &[f.request, other_request], &mut output)
        .unwrap();
    let [first, second] = output.map(Option::unwrap);
    assert_invariant(&f.journal);
    f.journal
        .settle_success(
            other_producer,
            &ContextWriterSuccessEvidenceV1 {
                writer: other_producer,
            },
        )
        .unwrap();
    assert_eq!(f.journal.producer_read_status(first), Ok(Status::Pending));
    assert_eq!(f.journal.producer_read_status(second), Ok(Status::Success));
    f.journal
        .settle_no_effect(
            f.producer,
            &ContextWriterNoEffectEvidenceV1 { writer: f.producer },
        )
        .unwrap();
    assert_eq!(f.journal.producer_read_status(first), Ok(Status::NoEffect));
    assert_eq!(f.journal.producer_read_status(second), Ok(Status::Success));
    assert_invariant(&f.journal);
    f.journal
        .release_producer_reads(
            key(20),
            &[first, second],
            &ContextReadQuiescenceEvidenceV1 { consumer: key(20) },
        )
        .unwrap();
    assert_invariant(&f.journal);
}

#[test]
fn allocation_reuse_and_last_incarnation_cannot_revive_old_reservations() {
    let mut f = Fixture::new(1);
    f.journal.next_incarnation = u64::MAX - 1;
    let reference = f.acquire(20);
    assert_eq!(reference.incarnation, u64::MAX - 1);
    f.journal
        .settle_success(
            f.producer,
            &ContextWriterSuccessEvidenceV1 { writer: f.producer },
        )
        .unwrap();
    f.release(reference);
    f.journal.retire_allocations(&[f.source]).unwrap();
    let replacement = f
        .journal
        .enroll_allocation(
            ContextAllocationKeyV1 {
                context_generation: 7,
                local: 100,
            },
            f.request.read.device,
            64,
        )
        .unwrap();
    assert_eq!(replacement.slot, f.source.slot);
    assert_eq!(f.journal.reader_count(replacement), Ok(0));
    assert!(f.journal.validate_producer_read(&f.request).is_err());
    assert_eq!(
        f.journal.lookup_producer_read(reference),
        Err(Error::InvalidReference)
    );
    assert_eq!(
        f.journal.validate_producer_read_capacity(1),
        Err(Error::EpochExhausted)
    );
    // An exhausted producer arena does not consume the stable arena's identities.
    let mut stable = [None];
    f.journal
        .acquire_reads(key(30), &[f.stable_read(replacement)], &mut stable)
        .unwrap();
    assert_eq!(f.journal.retained_read_count(), 1);
}

#[test]
fn malformed_roster_headers_preserve_both_arenas_and_caller_output() {
    let mut f = Fixture::new(2);
    assert_eq!(
        f.journal.acquire_producer_reads(key(20), &[], &mut []),
        Err(Error::RosterCapacity)
    );
    assert_eq!(
        f.journal
            .acquire_producer_reads(key(20), &[f.request], &mut []),
        Err(Error::RosterCapacity)
    );
    let reference = f.acquire(20);
    let mut occupied = [Some(reference)];
    let before = snapshot(&f.journal);
    assert_eq!(
        f.journal
            .acquire_producer_reads(key(21), &[f.request], &mut occupied),
        Err(Error::InvalidState)
    );
    assert_eq!(occupied, [Some(reference)]);
    assert_eq!(
        f.journal.release_producer_reads(
            key(20),
            &[],
            &ContextReadQuiescenceEvidenceV1 { consumer: key(20) }
        ),
        Err(Error::RosterCapacity)
    );
    assert_eq!(
        f.journal.release_producer_reads(
            key(21),
            &[reference],
            &ContextReadQuiescenceEvidenceV1 { consumer: key(21) }
        ),
        Err(Error::InvalidReference)
    );
    assert_eq!(snapshot(&f.journal), before);
    assert_invariant(&f.journal);
}

#[test]
fn full_arena_preserves_stable_header_error_precedence() {
    let mut wrapped = Fixture::new(1);
    let mut baseline = Fixture::new(1);
    let read = wrapped.stable_read(wrapped.other);
    let mut occupied = [None];
    let mut baseline_occupied = [None];
    wrapped
        .journal
        .acquire_reads(key(20), &[read], &mut occupied)
        .unwrap();
    baseline
        .journal
        .stable
        .acquire_reads(key(20), &[read], &mut baseline_occupied)
        .unwrap();
    assert_eq!(occupied, baseline_occupied);
    for (consumer, requests, output) in [
        (
            ContextWriterKeyV1 {
                context_generation: 8,
                ..key(21)
            },
            alloc::vec![read],
            alloc::vec![None],
        ),
        (key(0), alloc::vec![read], alloc::vec![None]),
        (key(u64::MAX), alloc::vec![read], alloc::vec![None]),
        (key(21), alloc::vec![], alloc::vec![]),
        (key(21), alloc::vec![read], alloc::vec![]),
        (key(21), alloc::vec![read], occupied.to_vec()),
        (key(21), alloc::vec![read], alloc::vec![None]),
    ] {
        let mut actual_output = output.clone();
        let mut expected_output = output.clone();
        let before = snapshot(&wrapped.journal);
        let actual = wrapped
            .journal
            .acquire_reads(consumer, &requests, &mut actual_output);
        let expected =
            baseline
                .journal
                .stable
                .acquire_reads(consumer, &requests, &mut expected_output);
        assert!(expected.is_err());
        assert_eq!(actual, expected);
        assert_eq!(actual_output, output);
        assert_eq!(expected_output, output);
        assert_eq!(snapshot(&wrapped.journal), before);
    }
}

#[test]
fn protected_later_member_blocks_whole_writer_disposal_and_retirement() {
    for unknown in [false, true] {
        let mut f = Fixture::new(2);
        f.journal
            .settle_no_effect(
                f.producer,
                &ContextWriterNoEffectEvidenceV1 { writer: f.producer },
            )
            .unwrap();
        let producer = f.journal.register_writer(key(30)).unwrap();
        let members = [f.member(f.source), f.member(f.other)];
        f.journal.begin_write(producer, &members).unwrap();
        let request = ContextProducerReadV1 {
            read: f.stable_read(f.other),
            producer,
        };
        let mut output = [None];
        f.journal
            .acquire_producer_reads(key(40), &[request], &mut output)
            .unwrap();
        let reference = output[0].unwrap();
        if unknown {
            f.journal.mark_unknown(producer).unwrap();
            let evidence = ContextWriterDisposalEvidenceV1 {
                writer: producer,
                allocations: &members,
            };
            let before = snapshot(&f.journal);
            assert_eq!(
                f.journal.validate_unknown_disposal(producer, &members),
                Err(Error::AllocationBusy)
            );
            assert_eq!(
                f.journal.dispose_unknown(producer, &evidence),
                Err(Error::AllocationBusy)
            );
            assert_eq!(snapshot(&f.journal), before);
            f.release(reference);
            f.journal.dispose_unknown(producer, &evidence).unwrap();
        } else {
            f.journal
                .settle_success(
                    producer,
                    &ContextWriterSuccessEvidenceV1 { writer: producer },
                )
                .unwrap();
            let next = f.journal.register_writer(key(50)).unwrap();
            let before = snapshot(&f.journal);
            assert_eq!(
                f.journal.begin_write(next, &members),
                Err(Error::AllocationBusy)
            );
            assert_eq!(
                f.journal
                    .validate_allocation_retirement(&[f.source, f.other]),
                Err(Error::AllocationBusy)
            );
            assert_eq!(
                f.journal.retire_allocations(&[f.source, f.other]),
                Err(Error::AllocationBusy)
            );
            assert_eq!(snapshot(&f.journal), before);
            f.release(reference);
            f.journal.retire_allocations(&[f.source, f.other]).unwrap();
        }
        assert!(f.journal.lookup_allocation(f.source).is_err());
        assert!(f.journal.lookup_allocation(f.other).is_err());
        assert_invariant(&f.journal);
    }
}
