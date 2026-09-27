use super::*;

#[test]
fn queued_read_indexed_work_is_affine_in_same_producer_roster_size() {
    let mut work = Vec::new();
    for size in [4, 8, 16] {
        let mut f = Fixture::new(size, size * 2);
        let indices: Vec<_> = (0..size).collect();
        let a = f.active(&indices);
        let destinations: Vec<_> = indices.iter().map(|&i| (i, Some(a))).collect();
        let b = f.queued(&destinations);
        let requests: Vec<_> = indices.iter().map(|&i| request(&f, i, b)).collect();
        let storage = (
            f.journal.read_producer_scratch.as_ptr(),
            f.journal.read_producer_scratch.capacity(),
        );
        let mut outputs = vec![None; size];
        f.journal.inner.reset_access_count_for_test_v1();
        f.journal
            .acquire_mixed_reads_with_queued(
                key(20),
                (&[], &mut []),
                (&[], &mut []),
                (&requests, &mut outputs),
            )
            .unwrap();
        work.push(f.journal.inner.guard_accesses_for_test_v1());
        assert!(f.journal.read_producer_scratch.is_empty());
        assert_eq!(
            storage,
            (
                f.journal.read_producer_scratch.as_ptr(),
                f.journal.read_producer_scratch.capacity()
            )
        );
        let references: Vec<_> = outputs.into_iter().map(Option::unwrap).collect();
        release(&mut f, &references);
        let mut bad = requests;
        bad[size - 1].byte_len = 0;
        let before = snapshot(&f.journal);
        assert_eq!(
            f.journal.acquire_mixed_reads_with_queued(
                key(21),
                (&[], &mut []),
                (&[], &mut []),
                (&bad, &mut vec![None; size])
            ),
            Err(Error::InvalidExtent)
        );
        assert!(f.journal.read_producer_scratch.is_empty());
        assert_eq!(snapshot(&f.journal), before);
    }
    assert!(work[1] > work[0]);
    assert_eq!(work[2] - work[1], 2 * (work[1] - work[0]));
}

#[test]
fn queued_reads_of_third_writer_allow_only_admitted_ancestors_to_progress() {
    let mut f = Fixture::new(1, 8);
    let a = f.active(&[0]);
    let b = f.queued(&[(0, Some(a))]);
    let c = f.queued(&[(0, Some(b))]);
    let req = request(&f, 0, c);
    let read = acquire(&mut f, 20, req);
    let d = f.writer();
    let writes = [ContextQueuedWriteV1 {
        destination: f.writes[0],
        predecessor: Some(c),
    }];
    for predecessor in [a, b, c] {
        let before = snapshot(&f.journal);
        assert_eq!(
            f.journal.begin_queued_write(d, &writes),
            Err(Error::AllocationBusy)
        );
        assert_eq!(snapshot(&f.journal), before);
        assert_eq!(
            f.journal.queued_producer_read_status(read),
            Ok(ContextProducerReadStatusV1::Pending)
        );
        f.success(predecessor);
    }
    assert_eq!(
        f.journal.queued_producer_read_status(read),
        Ok(ContextProducerReadStatusV1::Success)
    );
    let state = f.journal.lookup_allocation(f.writes[0].allocation).unwrap();
    assert_eq!((state.attempt_epoch, state.content_lineage), (3, 3));
    release(&mut f, &[read]);
}

#[test]
fn queued_read_blocked_or_unknown_exact_producer_rejects_atomically() {
    for unknown in [false, true] {
        let mut f = Fixture::new(2, 8);
        let a = f.active(&[0, 1]);
        let b = f.queued(&[(0, Some(a)), (1, Some(a))]);
        let requests = [request(&f, 0, b), request(&f, 1, b)];
        if unknown {
            f.journal.mark_unknown(b).unwrap();
        } else {
            f.cancel(a);
        }
        let before = snapshot(&f.journal);
        let mut outputs = [None; 2];
        assert_eq!(
            f.journal.acquire_mixed_reads_with_queued(
                key(20),
                (&[], &mut []),
                (&[], &mut []),
                (&requests, &mut outputs)
            ),
            Err(Error::AllocationBusy)
        );
        assert_eq!(outputs, [None; 2]);
        assert_eq!(snapshot(&f.journal), before);
    }
}

#[test]
fn queued_read_attachment_corruption_rejects_every_settlement_before_effect() {
    for corruption in 0..3 {
        for outcome in 0..3 {
            let mut f = Fixture::new(2, 8);
            let a = f.active(&[0, 1]);
            let b = f.queued(&[(0, Some(a)), (1, Some(a))]);
            let requests = [request(&f, 0, b), request(&f, 1, b)];
            f.journal
                .acquire_mixed_reads_with_queued(
                    key(20),
                    (&[], &mut []),
                    (&[], &mut []),
                    (&requests, &mut [None; 2]),
                )
                .unwrap();
            f.success(a);
            let root = f.journal.roots[b.slot].as_mut().unwrap();
            match corruption {
                0 => root.read_head = None,
                1 => root.read_count = 17,
                2 => root.read_count = 1,
                _ => unreachable!(),
            }
            let before = snapshot(&f.journal);
            let result = match outcome {
                0 => f
                    .journal
                    .settle_success(b, &ContextWriterSuccessEvidenceV1 { writer: b }),
                1 => f
                    .journal
                    .settle_no_effect(b, &ContextWriterNoEffectEvidenceV1 { writer: b }),
                2 => f.journal.mark_unknown(b),
                _ => unreachable!(),
            };
            assert_eq!(result, Err(Error::InvalidState));
            assert_eq!(snapshot(&f.journal), before);
        }
    }
}

fn request(f: &Fixture, index: usize, producer: Writer) -> ContextQueuedProducerReadV1 {
    ContextQueuedProducerReadV1 {
        allocation: f.writes[index],
        byte_offset: 0,
        byte_len: 16,
        producer,
    }
}

fn acquire(
    f: &mut Fixture,
    consumer: u64,
    request: ContextQueuedProducerReadV1,
) -> ContextQueuedProducerReadReferenceV1 {
    let mut output = [None];
    f.journal
        .acquire_mixed_reads_with_queued(
            key(consumer),
            (&[], &mut []),
            (&[], &mut []),
            (&[request], &mut output),
        )
        .unwrap();
    output[0].unwrap()
}

fn release(f: &mut Fixture, references: &[ContextQueuedProducerReadReferenceV1]) {
    let consumer = references[0].consumer;
    f.journal
        .release_queued_producer_reads(
            consumer,
            references,
            &ContextReadQuiescenceEvidenceV1 { consumer },
        )
        .unwrap();
}

#[test]
fn queued_read_resolves_actual_tail_version_and_survives_writer_slot_reuse() {
    let mut f = Fixture::new(2, 8);
    let a = f.active(&[0]);
    let b = f.queued(&[(0, Some(a))]);
    let req = request(&f, 0, b);
    let reference = acquire(&mut f, 3, req);
    let storage = (
        f.journal.queued_reads.as_ptr(),
        f.journal.free_reads.as_ptr(),
    );
    assert_eq!(
        f.journal.queued_producer_read_status(reference),
        Ok(ContextProducerReadStatusV1::Pending)
    );
    assert_eq!(f.journal.reader_count(f.writes[0].allocation), Ok(1));
    f.success(a);
    assert_eq!(
        f.journal.queued_producer_read_status(reference),
        Ok(ContextProducerReadStatusV1::Pending)
    );
    // Already admitted predecessors can activate despite the deferred reader.
    f.journal.activate_queued_writer(b).unwrap();
    f.success(b);
    let state = f.journal.lookup_allocation(f.writes[0].allocation).unwrap();
    assert_eq!((state.attempt_epoch, state.content_lineage), (2, 2));
    let reused = f.active(&[1]);
    assert_eq!(reused.slot, b.slot);
    assert_eq!(f.journal.lookup_queued_producer_read(reference), Ok(req));
    assert_eq!(
        f.journal.queued_producer_read_status(reference),
        Ok(ContextProducerReadStatusV1::Success)
    );
    let blocked = f.writer();
    assert_eq!(
        f.journal.begin_write(blocked, &[f.writes[0]]),
        Err(Error::AllocationBusy)
    );
    assert_eq!(
        f.journal.retire_allocations(&[f.writes[0].allocation]),
        Err(Error::AllocationBusy)
    );
    release(&mut f, &[reference]);
    assert_eq!(
        f.journal.lookup_queued_producer_read(reference),
        Err(Error::InvalidReference)
    );
    f.journal.begin_write(blocked, &[f.writes[0]]).unwrap();
    f.success(blocked);
    f.success(reused);
    assert_eq!(
        storage,
        (
            f.journal.queued_reads.as_ptr(),
            f.journal.free_reads.as_ptr()
        )
    );
}

#[test]
fn cancelled_tail_reads_never_inherit_ancestor_success() {
    let mut f = Fixture::new(1, 8);
    let a = f.active(&[0]);
    let b = f.queued(&[(0, Some(a))]);
    let req = request(&f, 0, b);
    let read = acquire(&mut f, 3, req);
    f.cancel(b);
    assert_eq!(
        f.journal.queued_producer_read_status(read),
        Ok(ContextProducerReadStatusV1::NoEffect)
    );
    f.success(a);
    assert_eq!(
        f.journal.queued_producer_read_status(read),
        Ok(ContextProducerReadStatusV1::NoEffect)
    );
    release(&mut f, &[read]);
    let c = f.active(&[0]);
    let d = f.queued(&[(0, Some(c))]);
    let req = request(&f, 0, d);
    let replacement = acquire(&mut f, 20, req);
    assert_eq!(replacement.slot, read.slot);
    assert_ne!(replacement.incarnation, read.incarnation);
    assert_eq!(
        f.journal.lookup_queued_producer_read(read),
        Err(Error::InvalidReference)
    );
    release(&mut f, &[replacement]);
    f.success(c);
    f.success(d);
}

#[test]
fn unknown_producer_and_selected_consumer_reads_block_group_disposal() {
    let mut f = Fixture::new(2, 8);
    let a = f.active(&[0]);
    let b = f.queued(&[(0, Some(a))]);
    let c = f.active(&[1]);
    let req = request(&f, 0, b);
    let read = acquire(&mut f, c.key.local, req);
    f.journal.mark_unknown(a).unwrap();
    assert_eq!(
        f.journal.queued_producer_read_status(read),
        Ok(ContextProducerReadStatusV1::Pending)
    );
    f.journal.mark_unknown(b).unwrap();
    f.journal.mark_unknown(c).unwrap();
    assert_eq!(
        f.journal.queued_producer_read_status(read),
        Ok(ContextProducerReadStatusV1::Unknown)
    );
    let x = [f.writes[0]];
    let y = [f.writes[1]];
    let producers = [
        ContextWriterDisposalEvidenceV1 {
            writer: a,
            allocations: &x,
        },
        ContextWriterDisposalEvidenceV1 {
            writer: b,
            allocations: &x,
        },
    ];
    let consumer = [ContextWriterDisposalEvidenceV1 {
        writer: c,
        allocations: &y,
    }];
    assert_eq!(
        f.journal.validate_unknown_group_disposal(&producers, &x),
        Err(Error::AllocationBusy)
    );
    assert_eq!(
        f.journal.validate_unknown_group_disposal(&consumer, &y),
        Err(Error::AllocationBusy)
    );
    assert_eq!(
        f.journal.validate_unknown_disposal(c, &y),
        Err(Error::AllocationBusy)
    );
    release(&mut f, &[read]);
    f.journal
        .validate_unknown_group_disposal(&producers, &x)
        .unwrap();
    f.journal
        .validate_unknown_group_disposal(&consumer, &y)
        .unwrap();
}

#[test]
fn mixed_acquisition_is_atomic_and_all_families_share_capacity() {
    let mut f = Fixture::new(3, 8);
    let a = f.active(&[0]);
    let b = f.queued(&[(0, Some(a))]);
    let p = f.active(&[2]);
    let stable = f.read(1);
    let active = ContextProducerReadV1 {
        read: f.read(2),
        producer: p,
    };
    let queued = request(&f, 0, b);
    let before = snapshot(&f.journal);
    for case in 0..5 {
        let mut bad = queued;
        match case {
            0 => bad.byte_len = 0,
            1 => bad.producer = a,
            2 => bad.byte_offset = u64::MAX,
            3 => bad.allocation.byte_extent += 1,
            _ => bad.producer.key.local += 1,
        }
        let (mut s, mut p, mut q) = ([None], [None], [None]);
        assert!(
            f.journal
                .acquire_mixed_reads_with_queued(
                    key(20),
                    (&[stable], &mut s),
                    (&[active], &mut p),
                    (&[bad], &mut q)
                )
                .is_err()
        );
        assert_eq!((s, p, q), ([None], [None], [None]));
        assert_eq!(snapshot(&f.journal), before);
    }
    let (mut s, mut p, mut q) = ([None], [None], [None]);
    f.journal
        .acquire_mixed_reads_with_queued(
            key(20),
            (&[stable], &mut s),
            (&[active], &mut p),
            (&[queued], &mut q),
        )
        .unwrap();
    let mut fanout = Vec::new();
    for consumer in 21..34 {
        fanout.push(acquire(&mut f, consumer, queued));
    }
    assert_eq!(f.journal.retained_read_count(), 16);
    assert_eq!(f.journal.retained_producer_read_count(), 15);
    assert_eq!(f.journal.remaining_read_slots(), 0);
    for capacity in [
        f.journal.validate_read_capacity(1),
        f.journal.validate_producer_read_capacity(1),
        f.journal.validate_queued_producer_read_capacity(1),
    ] {
        assert_eq!(capacity, Err(Error::MemberCapacity));
    }
    let mut output = [None];
    assert_eq!(
        f.journal.acquire_reads(key(40), &[stable], &mut output),
        Err(Error::MemberCapacity)
    );
    let mut output = [None];
    assert_eq!(
        f.journal
            .acquire_producer_reads(key(40), &[active], &mut output),
        Err(Error::MemberCapacity)
    );
    let (mut s2, mut p2) = ([None], [None]);
    assert_eq!(
        f.journal
            .acquire_mixed_reads(key(40), &[stable], &mut s2, &[active], &mut p2),
        Err(Error::MemberCapacity)
    );
    assert_eq!((s2, p2), ([None], [None]));
    for read in fanout {
        release(&mut f, &[read]);
    }
    release(&mut f, &[q[0].unwrap()]);
    let evidence = ContextReadQuiescenceEvidenceV1 { consumer: key(20) };
    f.journal
        .release_reads(key(20), &[s[0].unwrap()], &evidence)
        .unwrap();
    f.journal
        .release_producer_reads(key(20), &[p[0].unwrap()], &evidence)
        .unwrap();
    assert_eq!(f.journal.retained_read_count(), 0);
    f.success(a);
    f.success(b);
}

#[test]
fn pending_batch_release_uses_current_links_and_rejects_duplicates_atomically() {
    let mut f = Fixture::new(2, 8);
    let a = f.active(&[0, 1]);
    let b = f.queued(&[(0, Some(a)), (1, Some(a))]);
    let requests = [
        request(&f, 0, b),
        ContextQueuedProducerReadV1 {
            byte_offset: 16,
            ..request(&f, 0, b)
        },
        request(&f, 1, b),
    ];
    let mut output = [None; 3];
    f.journal
        .acquire_mixed_reads_with_queued(
            key(3),
            (&[], &mut []),
            (&[], &mut []),
            (&requests, &mut output),
        )
        .unwrap();
    let refs = output.map(Option::unwrap);
    let before = snapshot(&f.journal);
    let evidence = ContextReadQuiescenceEvidenceV1 { consumer: key(3) };
    assert!(
        f.journal
            .release_queued_producer_reads(key(3), &[refs[0], refs[0]], &evidence)
            .is_err()
    );
    assert_eq!(snapshot(&f.journal), before);
    release(&mut f, &refs);
    assert_eq!(f.journal.root(b).unwrap().read_count, 0);
    f.success(a);
    f.success(b);
}
