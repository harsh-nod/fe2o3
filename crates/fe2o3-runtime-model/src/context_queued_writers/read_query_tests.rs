use super::*;
use alloc::{format, string::String, vec};

fn key(local: u64) -> ContextWriterKeyV1 {
    ContextWriterKeyV1 {
        context_generation: 1,
        local,
        kind: ContextWriterKindV1::Submission,
    }
}

struct Fixture {
    owner: ContextQueuedWriterJournalV1,
    write: ContextAllocationWriteV1,
    active: ContextWriterReferenceV1,
    queued: ContextWriterReferenceV1,
    reads: Vec<ContextQueuedProducerReadReferenceV1>,
}

impl Fixture {
    fn pending(count: usize) -> Self {
        let mut owner = ContextQueuedWriterJournalV1::new(1, 2, 8, 8, 16).unwrap();
        let device = ContextJournalDeviceKeyV1 {
            context_generation: 1,
            local: 1,
        };
        let allocation = owner
            .enroll_allocation(
                ContextAllocationKeyV1 {
                    context_generation: 1,
                    local: 1,
                },
                device,
                64,
            )
            .unwrap();
        owner
            .enroll_allocation(
                ContextAllocationKeyV1 {
                    context_generation: 1,
                    local: 2,
                },
                device,
                64,
            )
            .unwrap();
        let write = ContextAllocationWriteV1 {
            allocation,
            device,
            byte_extent: 64,
        };
        let active = owner.register_writer(key(1)).unwrap();
        owner.begin_write(active, &[write]).unwrap();
        let queued = owner.register_writer(key(2)).unwrap();
        owner
            .begin_queued_write(
                queued,
                &[ContextQueuedWriteV1 {
                    destination: write,
                    predecessor: Some(active),
                }],
            )
            .unwrap();
        let requests: Vec<_> = (0..count)
            .map(|index| ContextQueuedProducerReadV1 {
                allocation: write,
                byte_offset: index as u64 * 4,
                byte_len: 4,
                producer: queued,
            })
            .collect();
        let mut output = vec![None; count];
        owner
            .acquire_mixed_reads_with_queued(
                key(10),
                (&[], &mut []),
                (&[], &mut []),
                (&requests, &mut output),
            )
            .unwrap();
        Self {
            owner,
            write,
            active,
            queued,
            reads: output.into_iter().map(Option::unwrap).collect(),
        }
    }

    fn resolve(&mut self, status: ContextProducerReadStatusV1) {
        match status {
            ContextProducerReadStatusV1::Pending => {}
            ContextProducerReadStatusV1::Success => {
                self.owner
                    .settle_success(
                        self.active,
                        &ContextWriterSuccessEvidenceV1 {
                            writer: self.active,
                        },
                    )
                    .unwrap();
                self.owner
                    .settle_success(
                        self.queued,
                        &ContextWriterSuccessEvidenceV1 {
                            writer: self.queued,
                        },
                    )
                    .unwrap();
            }
            ContextProducerReadStatusV1::NoEffect => {
                self.owner
                    .settle_no_effect(
                        self.queued,
                        &ContextWriterNoEffectEvidenceV1 {
                            writer: self.queued,
                        },
                    )
                    .unwrap();
            }
            ContextProducerReadStatusV1::Unknown => self.owner.mark_unknown(self.queued).unwrap(),
        }
    }

    fn entry(&mut self, index: usize) -> &mut Reservation {
        self.owner.queued_reads[self.reads[index].slot]
            .as_mut()
            .unwrap()
    }
}

fn frame(owner: &ContextQueuedWriterJournalV1) -> (String, [(usize, usize); 11]) {
    // The existing cfg(test) access counter is instrumentation, not owner custody.
    owner.inner.reset_access_count_for_test_v1();
    (
        format!("{owner:?}"),
        [
            (owner.roots.as_ptr() as usize, owner.roots.capacity()),
            (owner.members.as_ptr() as usize, owner.members.capacity()),
            (owner.free.as_ptr() as usize, owner.free.capacity()),
            (owner.heads.as_ptr() as usize, owner.heads.capacity()),
            (owner.tails.as_ptr() as usize, owner.tails.capacity()),
            (
                owner.queued_counts.as_ptr() as usize,
                owner.queued_counts.capacity(),
            ),
            (owner.scratch.as_ptr() as usize, owner.scratch.capacity()),
            (
                owner.queued_reads.as_ptr() as usize,
                owner.queued_reads.capacity(),
            ),
            (
                owner.free_reads.as_ptr() as usize,
                owner.free_reads.capacity(),
            ),
            (
                owner.read_counts.as_ptr() as usize,
                owner.read_counts.capacity(),
            ),
            (
                owner.read_producer_scratch.as_ptr() as usize,
                owner.read_producer_scratch.capacity(),
            ),
        ],
    )
}

fn check(
    owner: &ContextQueuedWriterJournalV1,
    reference: ContextQueuedProducerReadReferenceV1,
    expected: Result<ContextProducerReadStatusV1, Error>,
    compare_baseline: bool,
) {
    let before = frame(owner);
    assert_eq!(owner.queued_producer_read_status(reference), expected);
    match expected {
        Ok(_) => assert_eq!(
            owner.lookup_queued_producer_read(reference),
            Ok(owner.queued_reads[reference.slot].unwrap().request)
        ),
        Err(error) => assert_eq!(owner.lookup_queued_producer_read(reference), Err(error)),
    }
    if compare_baseline {
        let view = |entry: Reservation| {
            (
                entry.reference,
                entry.request,
                entry.status,
                entry.version,
                entry.previous,
                entry.next,
            )
        };
        assert_eq!(
            owner.inspect_queued_read(reference).map(view),
            owner.baseline_inspect_queued_read(reference).map(view)
        );
    }
    assert_eq!(frame(owner), before);
}

#[test]
fn queued_query_full_identity_helpers_match_literal_derived_equality() {
    let mut keys = Vec::new();
    for context_generation in [0, 1, u64::MAX] {
        for local in [0, 1, u64::MAX] {
            for kind in [
                ContextWriterKindV1::Synchronous,
                ContextWriterKindV1::Submission,
            ] {
                keys.push(ContextWriterKeyV1 {
                    context_generation,
                    local,
                    kind,
                });
            }
        }
    }
    for &left in &keys {
        for &right in &keys {
            assert_eq!(queued_writer_key_same_v1(left, right), left == right);
            for slot in [0, usize::MAX] {
                let a = ContextWriterReferenceV1 { slot: 0, key: left };
                let b = ContextWriterReferenceV1 { slot, key: right };
                assert_eq!(queued_writer_same_v1(a, b), a == b);
                for incarnation in [0, 1, u64::MAX] {
                    let a = ContextQueuedProducerReadReferenceV1 {
                        slot: 0,
                        incarnation: 1,
                        consumer: left,
                    };
                    let b = ContextQueuedProducerReadReferenceV1 {
                        slot,
                        incarnation,
                        consumer: right,
                    };
                    assert_eq!(queued_read_reference_same_v1(a, b), a == b);
                }
            }
        }
    }
}

#[test]
fn queued_query_active_lookup_preserves_terminal_status_asymmetry() {
    let mut f = Fixture::pending(0);
    f.owner
        .settle_no_effect(
            f.queued,
            &ContextWriterNoEffectEvidenceV1 { writer: f.queued },
        )
        .unwrap();
    let state = f.owner.lookup_allocation(f.write.allocation).unwrap();
    let request = ContextProducerReadV1 {
        producer: f.active,
        read: ContextAllocationReadV1 {
            allocation: f.write.allocation,
            device: f.write.device,
            byte_extent: 64,
            byte_offset: 0,
            byte_len: 4,
            attempt_epoch: state.attempt_epoch,
            content_lineage: state.content_lineage,
        },
    };
    let mut output = [None];
    f.owner
        .acquire_producer_reads(key(10), &[request], &mut output)
        .unwrap();
    let reference = output[0].unwrap();
    for terminal in [false, true] {
        f.owner.disposal_terminal = terminal;
        let before = frame(&f.owner);
        assert_eq!(f.owner.lookup_producer_read(reference), Ok(request));
        assert_eq!(
            f.owner.producer_read_status(reference),
            if terminal {
                Err(Error::InvalidState)
            } else {
                Ok(ContextProducerReadStatusV1::Pending)
            }
        );
        let bad = ContextProducerReadReferenceV1 {
            incarnation: reference.incarnation + 1,
            ..reference
        };
        assert_eq!(
            f.owner.lookup_producer_read(bad),
            Err(Error::InvalidReference)
        );
        assert_eq!(
            f.owner.producer_read_status(bad),
            Err(if terminal {
                Error::InvalidState
            } else {
                Error::InvalidReference
            })
        );
        assert_eq!(frame(&f.owner), before);
    }
}

#[test]
fn queued_query_reference_and_first_error_order_are_exact() {
    for fault in 0..8 {
        let mut f = Fixture::pending(1);
        let mut reference = f.reads[0];
        match fault {
            0 => reference.slot = usize::MAX,
            1 => reference.incarnation += 1,
            2 => reference.consumer.context_generation += 1,
            3 => reference.consumer.local += 1,
            4 => reference.consumer.kind = ContextWriterKindV1::Synchronous,
            5 => f.owner.queued_reads[reference.slot] = None,
            6 => {
                f.entry(0).request.allocation.device.local += 1;
                f.entry(0).request.allocation.byte_extent += 1;
            }
            7 => f.entry(0).request.allocation.byte_extent += 1,
            _ => unreachable!(),
        }
        let expected = match fault {
            6 => Error::AllocationDeviceMismatch,
            7 => Error::AllocationExtentMismatch,
            _ => Error::InvalidReference,
        };
        check(&f.owner, reference, Err(expected), true);
        f.owner.disposal_terminal = true;
        check(&f.owner, reference, Err(Error::InvalidState), true);
    }
    let mut f = Fixture::pending(1);
    f.entry(0).request.allocation.device.local += 1;
    f.owner.read_counts.clear();
    check(
        &f.owner,
        f.reads[0],
        Err(Error::AllocationDeviceMismatch),
        true,
    );
    f.entry(0).request.allocation.device.local -= 1;
    f.entry(0).request.allocation.byte_extent += 1;
    check(
        &f.owner,
        f.reads[0],
        Err(Error::AllocationExtentMismatch),
        true,
    );
}

#[test]
fn queued_query_missing_read_count_refuses_without_panicking() {
    let mut f = Fixture::pending(1);
    f.owner.read_counts.clear();
    f.entry(0).previous = Some(usize::MAX);
    check(&f.owner, f.reads[0], Err(Error::InvalidReference), false);
    f.owner.read_counts.push(0);
    check(&f.owner, f.reads[0], Err(Error::InvalidState), true);
    f.owner.read_counts[0] = 1;
    check(&f.owner, f.reads[0], Err(Error::InvalidReference), true);
}

#[test]
fn queued_query_pending_root_and_local_neighbors_match_frozen_queries() {
    for fault in 0..16 {
        let mut f = Fixture::pending(3);
        let reference = f.reads[1];
        let expected = match fault {
            0 => {
                f.owner.roots[f.queued.slot] = None;
                Error::InvalidReference
            }
            1 => {
                f.owner.roots[f.queued.slot]
                    .as_mut()
                    .unwrap()
                    .writer
                    .key
                    .local += 1;
                Error::InvalidReference
            }
            2 => {
                f.entry(1).request.producer.key.context_generation += 1;
                Error::InvalidReference
            }
            3 => {
                f.owner.roots[f.queued.slot].as_mut().unwrap().read_count = 0;
                Error::InvalidState
            }
            4 => {
                f.owner.roots[f.queued.slot].as_mut().unwrap().read_count = 9;
                Error::InvalidState
            }
            5 => {
                f.entry(1).previous = None;
                Error::InvalidState
            }
            6 => {
                f.entry(1).previous = Some(usize::MAX);
                Error::InvalidReference
            }
            7 => {
                let previous = f.entry(1).previous.unwrap();
                f.owner.queued_reads[previous] = None;
                Error::InvalidReference
            }
            8 => {
                let previous = f.entry(1).previous.unwrap();
                f.owner.queued_reads[previous]
                    .as_mut()
                    .unwrap()
                    .request
                    .producer
                    .key
                    .local += 1;
                Error::InvalidState
            }
            9 => {
                let previous = f.entry(1).previous.unwrap();
                f.owner.queued_reads[previous].as_mut().unwrap().status =
                    ContextProducerReadStatusV1::Unknown;
                Error::InvalidState
            }
            10 => {
                let previous = f.entry(1).previous.unwrap();
                f.owner.queued_reads[previous].as_mut().unwrap().next = None;
                f.entry(1).next = Some(usize::MAX);
                Error::InvalidState
            }
            11 => {
                f.entry(1).next = Some(usize::MAX);
                Error::InvalidReference
            }
            12 => {
                let next = f.entry(1).next.unwrap();
                f.owner.queued_reads[next] = None;
                Error::InvalidReference
            }
            13 => {
                let next = f.entry(1).next.unwrap();
                f.owner.queued_reads[next]
                    .as_mut()
                    .unwrap()
                    .request
                    .producer
                    .slot = usize::MAX;
                Error::InvalidState
            }
            14 => {
                let next = f.entry(1).next.unwrap();
                f.owner.queued_reads[next].as_mut().unwrap().previous = None;
                Error::InvalidState
            }
            15 => {
                let next = f.entry(1).next.unwrap();
                f.owner.queued_reads[next].as_mut().unwrap().status =
                    ContextProducerReadStatusV1::Unknown;
                Error::InvalidState
            }
            _ => unreachable!(),
        };
        check(&f.owner, reference, Err(expected), true);
    }
}

#[test]
fn queued_query_resolved_versions_links_and_reused_writer_slots_are_exact() {
    for status in [
        ContextProducerReadStatusV1::Pending,
        ContextProducerReadStatusV1::Success,
        ContextProducerReadStatusV1::NoEffect,
        ContextProducerReadStatusV1::Unknown,
    ] {
        let mut f = Fixture::pending(1);
        f.resolve(status);
        check(&f.owner, f.reads[0], Ok(status), true);
        if matches!(
            status,
            ContextProducerReadStatusV1::Success | ContextProducerReadStatusV1::NoEffect
        ) {
            let replacement = f.owner.register_writer(key(30)).unwrap();
            assert_eq!(replacement.slot, f.queued.slot);
            assert_ne!(replacement.key, f.queued.key);
            check(&f.owner, f.reads[0], Ok(status), true);
        }
        if status != ContextProducerReadStatusV1::Pending {
            f.entry(0).previous = Some(usize::MAX);
            check(&f.owner, f.reads[0], Err(Error::InvalidState), true);
            f.entry(0).previous = None;
            f.entry(0).next = Some(usize::MAX);
            check(&f.owner, f.reads[0], Err(Error::InvalidState), true);
            f.entry(0).next = None;
        }
        f.entry(0).version = if status == ContextProducerReadStatusV1::Success {
            None
        } else {
            Some((0, 0))
        };
        check(&f.owner, f.reads[0], Err(Error::InvalidState), true);
    }
    let mut f = Fixture::pending(1);
    f.resolve(ContextProducerReadStatusV1::Success);
    let state = f.owner.lookup_allocation(f.write.allocation).unwrap();
    for version in [
        (state.attempt_epoch + 1, state.content_lineage),
        (state.attempt_epoch, state.content_lineage + 1),
    ] {
        f.entry(0).version = Some(version);
        check(&f.owner, f.reads[0], Err(Error::InvalidState), true);
    }
    f.owner
        .inner
        .seed_idle_epoch_for_test_v1(f.write.allocation, state.attempt_epoch + 1);
    f.entry(0).version = Some((state.attempt_epoch + 1, state.content_lineage));
    check(&f.owner, f.reads[0], Err(Error::InvalidState), true);

    let mut f = Fixture::pending(1);
    f.resolve(ContextProducerReadStatusV1::Success);
    let replacement = f.owner.inner.register_writer(key(30)).unwrap();
    f.owner.inner.begin_write(replacement, &[f.write]).unwrap();
    let state = f.owner.lookup_allocation(f.write.allocation).unwrap();
    assert!(state.pending_writer.is_some());
    f.entry(0).version = Some((state.attempt_epoch, state.content_lineage));
    check(&f.owner, f.reads[0], Err(Error::InvalidState), true);
}

#[test]
fn queued_query_preserves_local_only_scope_and_unrelated_owner_storage() {
    let mut f = Fixture::pending(1);
    let reference = f.reads[0];
    f.entry(0).previous = Some(reference.slot);
    f.entry(0).next = Some(reference.slot);
    f.owner.roots[f.queued.slot].as_mut().unwrap().read_head = None;
    f.owner.free.clear();
    f.owner.free_reads.clear();
    f.owner.scratch.clear();
    f.owner.read_producer_scratch.push(f.active);
    check(
        &f.owner,
        reference,
        Ok(ContextProducerReadStatusV1::Pending),
        true,
    );
    // This is an arbitrary malformed owner's locally reciprocal cycle, not a valid list.
    let mut f = Fixture::pending(1);
    f.resolve(ContextProducerReadStatusV1::Unknown);
    f.entry(0).request.producer.slot = usize::MAX;
    check(
        &f.owner,
        f.reads[0],
        Ok(ContextProducerReadStatusV1::Unknown),
        true,
    );
}
