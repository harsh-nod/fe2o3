use super::*;

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
