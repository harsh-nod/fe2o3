use super::*;
use alloc::{format, string::String, vec};

mod group_disposal;
mod reads;

type Journal = ContextQueuedWriterJournalV1;
type Writer = ContextWriterReferenceV1;
type Write = ContextAllocationWriteV1;
type Status = ContextQueuedWriterStatusV1;

fn key(local: u64) -> ContextWriterKeyV1 {
    ContextWriterKeyV1 {
        context_generation: 7,
        local,
        kind: ContextWriterKindV1::Submission,
    }
}

struct Fixture {
    journal: Journal,
    writes: Vec<Write>,
}

impl Fixture {
    fn new(allocations: usize, members: usize) -> Self {
        let mut journal = Journal::new(7, allocations, 16, 16, members).unwrap();
        let device = ContextJournalDeviceKeyV1 {
            context_generation: 7,
            local: 1,
        };
        let writes = (1..=allocations)
            .map(|local| {
                let allocation = journal
                    .enroll_allocation(
                        ContextAllocationKeyV1 {
                            context_generation: 7,
                            local: local as u64,
                        },
                        device,
                        64,
                    )
                    .unwrap();
                Write {
                    allocation,
                    device,
                    byte_extent: 64,
                }
            })
            .collect();
        Self { journal, writes }
    }

    fn writer(&mut self) -> Writer {
        self.journal
            .register_writer(key(self.journal.registration_watermark() + 1))
            .unwrap()
    }

    fn active(&mut self, slots: &[usize]) -> Writer {
        let writer = self.writer();
        let members: Vec<_> = slots.iter().map(|&i| self.writes[i]).collect();
        self.journal.begin_write(writer, &members).unwrap();
        audit(&self.journal);
        writer
    }

    fn queued(&mut self, bindings: &[(usize, Option<Writer>)]) -> Writer {
        let writer = self.writer();
        let requests: Vec<_> = bindings
            .iter()
            .map(|&(i, predecessor)| ContextQueuedWriteV1 {
                destination: self.writes[i],
                predecessor,
            })
            .collect();
        self.journal.begin_queued_write(writer, &requests).unwrap();
        self.journal
            .validate_queued_writer(writer, &requests)
            .unwrap();
        audit(&self.journal);
        writer
    }

    fn read(&self, index: usize) -> ContextAllocationReadV1 {
        let write = self.writes[index];
        let state = self.journal.lookup_allocation(write.allocation).unwrap();
        ContextAllocationReadV1 {
            allocation: write.allocation,
            device: write.device,
            byte_extent: write.byte_extent,
            byte_offset: 0,
            byte_len: 16,
            attempt_epoch: state.attempt_epoch,
            content_lineage: state.content_lineage,
        }
    }

    fn success(&mut self, writer: Writer) {
        self.journal
            .settle_success(writer, &ContextWriterSuccessEvidenceV1 { writer })
            .unwrap();
        audit(&self.journal);
    }

    fn cancel(&mut self, writer: Writer) {
        self.journal
            .settle_no_effect(writer, &ContextWriterNoEffectEvidenceV1 { writer })
            .unwrap();
        audit(&self.journal);
    }
}

fn snapshot(j: &Journal) -> String {
    j.inner.reset_access_count_for_test_v1();
    format!("{j:?}")
}

// Independently traverse both indexes, accounting for every arena cell exactly once.
fn audit(j: &Journal) {
    let mut by_writer = vec![0; j.members.len()];
    let mut by_allocation = vec![0; j.members.len()];
    let mut free = vec![0; j.members.len()];
    for root in j.roots.iter().flatten() {
        j.validate_root(root.writer).unwrap();
        let mut next = root.head;
        for _ in 0..root.count {
            let index = next.unwrap();
            by_writer[index] += 1;
            next = j.member(index).unwrap().next_writer;
        }
        assert!(next.is_none());
    }
    for slot in 0..j.heads.len() {
        let mut next = j.heads[slot];
        let mut previous = None;
        let mut queued = 0;
        while let Some(index) = next {
            by_allocation[index] += 1;
            assert_eq!(by_allocation[index], 1, "allocation cycle");
            let member = j.member(index).unwrap();
            assert_eq!(member.request.destination.allocation.slot, slot);
            assert_eq!(member.previous_allocation, previous);
            if j.root(member.writer).unwrap().phase != Phase::Active {
                queued += 1;
            }
            previous = Some(index);
            next = member.next_allocation;
        }
        assert_eq!(j.tails[slot], previous);
        assert_eq!(j.queued_counts[slot], queued);
    }
    for &index in &j.free {
        free[index] += 1;
    }
    for index in 0..j.members.len() {
        let live = usize::from(j.members[index].is_some());
        assert_eq!(by_writer[index], live);
        assert_eq!(by_allocation[index], live);
        assert_eq!(free[index] + live, 1);
    }
}

#[test]
fn three_deep_success_uses_actual_epochs_and_lineages() {
    let mut f = Fixture::new(2, 8);
    let a = f.active(&[0]);
    let b = f.queued(&[(0, Some(a))]);
    let c = f.queued(&[(0, Some(b))]);
    assert_eq!(f.journal.queued_writer_status(b), Ok(Some(Status::Waiting)));
    assert_eq!(
        f.journal.activate_queued_writer(b),
        Err(Error::AllocationBusy)
    );
    for (ordinal, writer) in [a, b, c].into_iter().enumerate() {
        f.success(writer);
        let state = f.journal.lookup_allocation(f.writes[0].allocation).unwrap();
        assert_eq!(state.attempt_epoch, ordinal as u64 + 1);
        assert_eq!(state.content_lineage, ordinal as u64 + 1);
        assert_eq!(state.pending_writer, None);
        if let Some(next) = [b, c].get(ordinal) {
            assert_eq!(
                f.journal.lookup_writer(*next),
                Ok(ContextWriterStateV1::Reserved)
            );
            assert_eq!(
                f.journal.queued_writer_status(*next),
                Ok(Some(Status::Ready))
            );
        }
    }
    assert_eq!(
        f.journal.remaining_member_slots(),
        f.journal.member_capacity()
    );
    assert_eq!(f.journal.latest_writer(f.writes[0].allocation), Ok(None));
}

#[test]
fn multi_parent_join_reserves_idle_members_and_activates_whole_roster() {
    let mut f = Fixture::new(3, 12);
    let a = f.active(&[0]);
    let b = f.active(&[1]);
    let c = f.queued(&[(0, Some(a)), (1, Some(b)), (2, None)]);
    assert_eq!(
        f.journal.validate_read(&f.read(2)),
        Err(Error::AllocationBusy)
    );
    f.success(b);
    assert_eq!(f.journal.queued_writer_status(c), Ok(Some(Status::Waiting)));
    f.success(a);
    assert_eq!(f.journal.queued_writer_status(c), Ok(Some(Status::Ready)));
    f.journal.activate_queued_writer(c).unwrap();
    audit(&f.journal);
    assert_eq!(
        f.journal.lookup_writer(c),
        Ok(ContextWriterStateV1::Pending { member_count: 3 })
    );
    for write in &f.writes {
        assert_eq!(
            f.journal
                .lookup_allocation(write.allocation)
                .unwrap()
                .pending_writer,
            Some(c)
        );
    }
    f.success(c);
    for (index, write) in f.writes.iter().enumerate() {
        let state = f.journal.lookup_allocation(write.allocation).unwrap();
        assert_eq!(state.attempt_epoch, if index == 2 { 1 } else { 2 });
        assert_eq!(state.content_lineage, state.attempt_epoch);
    }
}

#[test]
fn ready_but_reserved_writer_blocks_every_external_admission() {
    let mut f = Fixture::new(2, 8);
    let a = f.active(&[0]);
    let b = f.queued(&[(0, Some(a))]);
    let older_read = ContextProducerReadV1 {
        read: f.read(0),
        producer: a,
    };
    assert_eq!(
        f.journal.validate_producer_read(&older_read),
        Err(Error::AllocationBusy)
    );
    f.success(a);
    let c = f.writer();
    let read = f.read(0);
    let stable = f.read(1);
    let before = snapshot(&f.journal);
    let mut output = [None];
    let mut producer_output = [None];
    assert_eq!(
        f.journal.validate_no_queued_writer(read.allocation),
        Err(Error::AllocationBusy)
    );
    assert_eq!(f.journal.validate_read(&read), Err(Error::AllocationBusy));
    assert_eq!(
        f.journal.begin_write(c, &[f.writes[0]]),
        Err(Error::AllocationBusy)
    );
    assert_eq!(
        f.journal.acquire_reads(key(12), &[read], &mut output),
        Err(Error::AllocationBusy)
    );
    assert_eq!(
        f.journal
            .acquire_producer_reads(key(12), &[older_read], &mut producer_output),
        Err(Error::AllocationBusy)
    );
    assert_eq!(
        f.journal.acquire_mixed_reads(
            key(12),
            &[stable],
            &mut output,
            &[older_read],
            &mut producer_output
        ),
        Err(Error::AllocationBusy)
    );
    assert_eq!(
        f.journal
            .acquire_mixed_reads(key(12), &[read], &mut output, &[], &mut []),
        Err(Error::AllocationBusy)
    );
    assert_eq!(
        f.journal.retire_allocations(&[read.allocation]),
        Err(Error::AllocationBusy)
    );
    assert_eq!(f.journal.abort_reserved(b), Err(Error::AllocationBusy));
    assert_eq!(output, [None]);
    assert_eq!(producer_output, [None]);
    assert_eq!(snapshot(&f.journal), before);
    f.cancel(b);
    f.journal.validate_read(&read).unwrap();
    f.journal.abort_reserved(c).unwrap();
}

#[test]
fn middle_cancellation_never_reparents_descendants() {
    for cancel_first in [true, false] {
        let mut f = Fixture::new(2, 10);
        let a = f.active(&[0]);
        let b = f.queued(&[(0, Some(a))]);
        let c = f.queued(&[(0, Some(b))]);
        let d = f.queued(&[(0, Some(c))]);
        if cancel_first {
            f.cancel(c);
        }
        f.success(a);
        f.success(b);
        if !cancel_first {
            f.cancel(c);
        }
        assert_eq!(f.journal.queued_writer_status(d), Ok(Some(Status::Blocked)));
        assert_eq!(
            f.journal.activate_queued_writer(d),
            Err(Error::AllocationBusy)
        );
        assert_eq!(f.journal.latest_writer(f.writes[0].allocation), Ok(Some(d)));
        f.cancel(d);
        assert_eq!(
            f.journal
                .lookup_allocation(f.writes[0].allocation)
                .unwrap()
                .attempt_epoch,
            2
        );
    }
}

#[test]
fn parent_no_effect_and_unknown_do_not_release_successor() {
    for unknown in [false, true] {
        let mut f = Fixture::new(1, 4);
        let a = f.active(&[0]);
        let b = f.queued(&[(0, Some(a))]);
        if unknown {
            f.journal.mark_unknown(a).unwrap();
            assert_eq!(
                f.journal.validate_unknown_disposal(a, &f.writes),
                Err(Error::AllocationBusy)
            );
            assert_eq!(
                f.journal
                    .settle_success(a, &ContextWriterSuccessEvidenceV1 { writer: a }),
                Err(Error::InvalidReference)
            );
        } else {
            f.cancel(a);
        }
        assert_eq!(
            f.journal.activate_queued_writer(b),
            Err(Error::AllocationBusy)
        );
        assert_eq!(f.journal.abort_reserved(b), Err(Error::AllocationBusy));
        assert_eq!(
            f.journal.validate_read(&f.read(0)),
            Err(Error::AllocationBusy)
        );
        f.cancel(b);
        if unknown {
            f.journal
                .dispose_unknown(
                    a,
                    &ContextWriterDisposalEvidenceV1 {
                        writer: a,
                        allocations: &f.writes,
                    },
                )
                .unwrap();
            audit(&f.journal);
        }
    }
}

#[test]
fn queued_unknown_is_fail_stop_even_with_later_no_effect() {
    let mut f = Fixture::new(1, 4);
    let a = f.active(&[0]);
    let b = f.queued(&[(0, Some(a))]);
    f.journal.mark_unknown(b).unwrap();
    f.success(a);
    assert_eq!(
        f.journal.lookup_writer(b),
        Ok(ContextWriterStateV1::Reserved)
    );
    assert_eq!(f.journal.queued_writer_status(b), Ok(Some(Status::Unknown)));
    assert_eq!(
        f.journal
            .settle_success(b, &ContextWriterSuccessEvidenceV1 { writer: b }),
        Err(Error::AllocationBusy)
    );
    assert_eq!(
        f.journal.validate_unknown_disposal(b, &f.writes),
        Err(Error::InvalidReference)
    );
    let before = snapshot(&f.journal);
    assert_eq!(
        f.journal
            .settle_no_effect(b, &ContextWriterNoEffectEvidenceV1 { writer: b }),
        Err(Error::InvalidState)
    );
    assert_eq!(snapshot(&f.journal), before);
    assert_eq!(
        f.journal.lookup_writer(b),
        Ok(ContextWriterStateV1::Reserved)
    );
    assert_eq!(
        f.journal
            .lookup_allocation(f.writes[0].allocation)
            .unwrap()
            .attempt_epoch,
        1
    );
}

#[test]
fn zero_queue_count_cannot_hide_reserved_custody() {
    for ready in [false, true] {
        let mut f = Fixture::new(1, 4);
        let a = f.active(&[0]);
        let _b = f.queued(&[(0, Some(a))]);
        if ready {
            f.success(a);
        }
        f.journal.queued_counts[0] = 0;
        let before = snapshot(&f.journal);
        assert_eq!(
            f.journal.validate_no_queued_writer(f.writes[0].allocation),
            Err(Error::InvalidState)
        );
        assert_eq!(
            f.journal.retire_allocations(&[f.writes[0].allocation]),
            Err(if ready {
                Error::InvalidState
            } else {
                Error::AllocationBusy
            })
        );
        assert_eq!(snapshot(&f.journal), before);
    }
}

#[test]
fn queued_admission_is_atomic_for_complete_roster_errors() {
    for fault in 0..8 {
        let mut f = Fixture::new(3, if fault == 7 { 3 } else { 12 });
        let a = f.active(&[0, 1]);
        let b = f.writer();
        let mut requests = [
            ContextQueuedWriteV1 {
                destination: f.writes[0],
                predecessor: Some(a),
            },
            ContextQueuedWriteV1 {
                destination: f.writes[1],
                predecessor: Some(a),
            },
        ];
        match fault {
            0 => requests[1].predecessor = None,
            1 => requests[1].predecessor.as_mut().unwrap().key.local += 1,
            2 => requests[1].destination.device.local += 1,
            3 => requests[1].destination.byte_extent -= 1,
            4 => requests[1] = requests[0],
            5 => requests.swap(0, 1),
            6 => requests[1].destination.allocation.key.local += 10,
            _ => {}
        }
        let before = snapshot(&f.journal);
        assert!(
            f.journal.begin_queued_write(b, &requests).is_err(),
            "fault {fault}"
        );
        assert_eq!(snapshot(&f.journal), before, "fault {fault}");
        audit(&f.journal);
    }
}

#[test]
fn preexisting_stable_or_producer_read_prevents_entire_queue_admission() {
    for producer in [false, true] {
        let mut f = Fixture::new(2, 8);
        let a = f.active(&[0]);
        let mut stable_refs = [None];
        let mut producer_refs = [None];
        let consumer = key(14);
        if producer {
            let request = ContextProducerReadV1 {
                read: f.read(0),
                producer: a,
            };
            f.journal
                .acquire_producer_reads(consumer, &[request], &mut producer_refs)
                .unwrap();
        } else {
            f.journal
                .acquire_reads(consumer, &[f.read(1)], &mut stable_refs)
                .unwrap();
        }
        let b = f.writer();
        let requests = [
            ContextQueuedWriteV1 {
                destination: f.writes[0],
                predecessor: Some(a),
            },
            ContextQueuedWriteV1 {
                destination: f.writes[1],
                predecessor: None,
            },
        ];
        let before = snapshot(&f.journal);
        assert_eq!(
            f.journal.begin_queued_write(b, &requests),
            Err(Error::AllocationBusy)
        );
        assert_eq!(snapshot(&f.journal), before);
        let evidence = ContextReadQuiescenceEvidenceV1 { consumer };
        if producer {
            f.journal
                .release_producer_reads(consumer, &[producer_refs[0].unwrap()], &evidence)
                .unwrap();
        } else {
            f.journal
                .release_reads(consumer, &[stable_refs[0].unwrap()], &evidence)
                .unwrap();
        }
        f.journal.begin_queued_write(b, &requests).unwrap();
        f.success(a);
        f.success(b);
    }
}

#[test]
fn stale_tail_and_recycled_writer_or_allocation_references_cannot_reenter() {
    let mut f = Fixture::new(1, 4);
    let a = f.active(&[0]);
    let b = f.queued(&[(0, Some(a))]);
    let c = f.writer();
    let request = ContextQueuedWriteV1 {
        destination: f.writes[0],
        predecessor: Some(a),
    };
    assert_eq!(
        f.journal.begin_queued_write(c, &[request]),
        Err(Error::AllocationBusy)
    );
    f.cancel(b);
    f.success(a);
    let d = f.active(&[0]);
    assert_eq!(d.slot, a.slot);
    assert_ne!(d.key, a.key);
    assert_eq!(
        f.journal.begin_queued_write(c, &[request]),
        Err(Error::AllocationBusy)
    );
    f.success(d);
    let old = f.writes[0];
    f.journal.retire_allocations(&[old.allocation]).unwrap();
    let new = f
        .journal
        .enroll_allocation(
            ContextAllocationKeyV1 {
                context_generation: 7,
                local: 2,
            },
            old.device,
            64,
        )
        .unwrap();
    assert_eq!(new.slot, old.allocation.slot);
    assert_eq!(
        f.journal.begin_queued_write(
            c,
            &[ContextQueuedWriteV1 {
                predecessor: None,
                ..request
            }]
        ),
        Err(Error::InvalidAllocationReference)
    );
    assert_eq!(f.journal.latest_writer(new), Ok(None));
    audit(&f.journal);
}

#[test]
fn zero_member_direct_writer_and_unknown_disposal_refund_all_indexes() {
    let mut f = Fixture::new(2, 4);
    let empty = f.active(&[]);
    f.success(empty);
    let a = f.active(&[0, 1]);
    f.journal.mark_unknown(a).unwrap();
    f.journal
        .dispose_unknown(
            a,
            &ContextWriterDisposalEvidenceV1 {
                writer: a,
                allocations: &f.writes,
            },
        )
        .unwrap();
    audit(&f.journal);
    assert_eq!(f.journal.remaining_allocation_slots(), 2);
    assert_eq!(f.journal.remaining_member_slots(), 4);
}

#[test]
fn whole_roster_link_fault_prevents_prefix_refunds() {
    for queued in [false, true] {
        let mut f = Fixture::new(2, 8);
        let a = f.active(&[0, 1]);
        let writer = if queued {
            f.queued(&[(0, Some(a)), (1, Some(a))])
        } else {
            a
        };
        let root = f.journal.root(writer).unwrap();
        let second = f
            .journal
            .member(root.head.unwrap())
            .unwrap()
            .next_writer
            .unwrap();
        f.journal.members[second].as_mut().unwrap().next_allocation = Some(usize::MAX);
        let before = snapshot(&f.journal);
        assert_eq!(
            f.journal
                .settle_no_effect(writer, &ContextWriterNoEffectEvidenceV1 { writer }),
            Err(Error::InvalidState)
        );
        assert_eq!(snapshot(&f.journal), before);
    }
}

#[test]
fn epoch_headroom_is_reserved_for_each_successor_before_activation() {
    let mut f = Fixture::new(1, 8);
    f.journal
        .inner
        .seed_idle_epoch_for_test_v1(f.writes[0].allocation, u64::MAX - 2);
    let a = f.active(&[0]);
    let b = f.queued(&[(0, Some(a))]);
    let c = f.writer();
    let request = ContextQueuedWriteV1 {
        destination: f.writes[0],
        predecessor: Some(b),
    };
    let before = snapshot(&f.journal);
    assert_eq!(
        f.journal.begin_queued_write(c, &[request]),
        Err(Error::EpochExhausted)
    );
    assert_eq!(snapshot(&f.journal), before);
    f.success(a);
    f.success(b);
    let state = f.journal.lookup_allocation(f.writes[0].allocation).unwrap();
    assert_eq!(state.attempt_epoch, u64::MAX);
    assert_eq!(state.content_lineage, u64::MAX);
    assert_eq!(
        f.journal.begin_write(c, &[f.writes[0]]),
        Err(Error::EpochExhausted)
    );
    audit(&f.journal);
}

#[test]
fn borrowed_admission_consumes_neither_identity_nor_storage_and_begin_rechecks() {
    let mut f = Fixture::new(1, 4);
    let a = f.active(&[0]);
    let requests = [ContextQueuedWriteV1 {
        destination: f.writes[0],
        predecessor: Some(a),
    }];
    let proposed = key(2);
    let before = snapshot(&f.journal);
    f.journal
        .validate_queued_write_admission(proposed, &requests)
        .unwrap();
    assert_eq!(snapshot(&f.journal), before);
    let b = f.queued(&[(0, Some(a))]);
    let c = f.writer();
    assert_eq!(
        f.journal.begin_queued_write(c, &requests),
        Err(Error::AllocationBusy)
    );
    f.cancel(b);
    f.journal.begin_queued_write(c, &requests).unwrap();
    audit(&f.journal);
}

#[test]
fn borrowed_admission_public_issuance_checks_match_registration() {
    for fault in 0..6 {
        let mut f = Fixture::new(1, 4);
        let a = f.active(&[0]);
        let mut proposed = key(10);
        match fault {
            0 => proposed.context_generation += 1,
            1 => proposed.local = 0,
            2 => proposed.local = u64::MAX,
            3 => proposed.local = a.key.local,
            4 => {
                while f.journal.remaining_writer_slots() != 0 {
                    let _ = f.writer();
                }
                proposed.local = f.journal.registration_watermark() + 1;
            }
            _ => {}
        }
        let requests = [ContextQueuedWriteV1 {
            destination: f.writes[0],
            predecessor: Some(a),
        }];
        let before = snapshot(&f.journal);
        let admission = f
            .journal
            .validate_queued_write_admission(proposed, &requests);
        assert_eq!(snapshot(&f.journal), before);
        assert_eq!(admission, f.journal.register_writer(proposed).map(|_| ()));
        if fault != 5 {
            assert_eq!(snapshot(&f.journal), before);
        }
    }
}

#[test]
fn empty_queue_acquisition_preserves_inner_error_precedence() {
    for mode in 0..3 {
        for fault in 0..8 {
            let mut f = Fixture::new(2, 8);
            let a = f.active(&[0]);
            let producer = ContextProducerReadV1 {
                producer: a,
                read: f.read(0),
            };
            let stable = f.read(1);
            let mut consumer = key(10);
            let mut stable_requests = [stable, stable];
            let mut producer_requests = [producer, producer];
            let mut stable_output = vec![None; 2];
            let mut producer_output = vec![None; 2];
            match fault {
                0 => consumer.context_generation += 1,
                1 => consumer.local = 0,
                2 => {
                    stable_output.pop();
                    producer_output.pop();
                }
                3 => consumer.kind = ContextWriterKindV1::Synchronous,
                4 => stable_requests[0].attempt_epoch += 1,
                5 => producer_requests[0].producer.key.local += 1,
                6 => {
                    stable_requests[0].byte_len = 0;
                    producer_requests[0].read.byte_len = 0;
                }
                _ => {}
            }
            stable_requests[1].allocation.key.local += 10;
            producer_requests[1].read.allocation.key.local += 10;
            let before = snapshot(&f.journal);
            let expected = match mode {
                0 => f
                    .journal
                    .inner
                    .acquire_reads(consumer, &stable_requests, &mut stable_output),
                1 => f.journal.inner.acquire_producer_reads(
                    consumer,
                    &producer_requests,
                    &mut producer_output,
                ),
                _ => f.journal.inner.acquire_mixed_reads(
                    consumer,
                    &stable_requests,
                    &mut stable_output,
                    &producer_requests,
                    &mut producer_output,
                ),
            };
            assert!(expected.is_err());
            assert_eq!(snapshot(&f.journal), before);
            let actual = match mode {
                0 => f
                    .journal
                    .acquire_reads(consumer, &stable_requests, &mut stable_output),
                1 => f.journal.acquire_producer_reads(
                    consumer,
                    &producer_requests,
                    &mut producer_output,
                ),
                _ => f.journal.acquire_mixed_reads(
                    consumer,
                    &stable_requests,
                    &mut stable_output,
                    &producer_requests,
                    &mut producer_output,
                ),
            };
            assert_eq!(actual, expected, "mode {mode}, fault {fault}");
            assert_eq!(snapshot(&f.journal), before);
        }
    }
}

#[test]
fn empty_queue_begin_and_retirement_preserve_inner_error_precedence() {
    for fault in 0..6 {
        let mut f = Fixture::new(2, 8);
        let mut writer = f.writer();
        let mut members = [f.writes[0], f.writes[1]];
        let mut retire = [f.writes[0].allocation, f.writes[1].allocation];
        f.journal
            .acquire_reads(key(10), &[f.read(0)], &mut [None])
            .unwrap();
        match fault {
            0 => writer.slot = usize::MAX,
            1 => members[1].allocation.key.local += 10,
            2 => members[1].device.local += 1,
            3 => members[1].byte_extent -= 1,
            4 => members.swap(0, 1),
            _ => members[1] = members[0],
        }
        retire[1].key.local += 10;
        let before = snapshot(&f.journal);
        let expected = f.journal.inner.begin_write(writer, &members);
        assert!(expected.is_err());
        assert_eq!(f.journal.begin_write(writer, &members), expected);
        assert_eq!(
            f.journal.validate_allocation_retirement(&retire),
            f.journal.inner.validate_allocation_retirement(&retire)
        );
        assert_eq!(snapshot(&f.journal), before);
    }
}

fn storage(j: &Journal) -> Vec<(usize, usize)> {
    let mut result = j.inner.guard_owner_storage_v1();
    result.extend([
        (j.roots.as_ptr() as usize, j.roots.capacity()),
        (j.members.as_ptr() as usize, j.members.capacity()),
        (j.free.as_ptr() as usize, j.free.capacity()),
        (j.heads.as_ptr() as usize, j.heads.capacity()),
        (j.tails.as_ptr() as usize, j.tails.capacity()),
        (
            j.queued_counts.as_ptr() as usize,
            j.queued_counts.capacity(),
        ),
        (j.scratch.as_ptr() as usize, j.scratch.capacity()),
    ]);
    result
}

#[test]
fn all_four_writer_outcome_orders_preserve_indexes_and_storage() {
    for i in 0..4 {
        for j in 0..4 {
            for k in 0..4 {
                for l in 0..4 {
                    let order = [i, j, k, l];
                    if order
                        .iter()
                        .enumerate()
                        .any(|(n, index)| order[..n].contains(index))
                    {
                        continue;
                    }
                    for successes in 0..16 {
                        let mut f = Fixture::new(2, 12);
                        let storage = storage(&f.journal);
                        let a = f.active(&[0, 1]);
                        let b = f.queued(&[(0, Some(a)), (1, Some(a))]);
                        let c = f.queued(&[(0, Some(b)), (1, Some(b))]);
                        let d = f.queued(&[(0, Some(c)), (1, Some(c))]);
                        let writers = [a, b, c, d];
                        let mut outcomes = [None; 4];
                        let mut epoch = 1;
                        let mut lineage = 0;
                        for index in order {
                            let ready = index == 0 || outcomes[index - 1] == Some(true);
                            if index != 0 {
                                let status = match outcomes[index - 1] {
                                    None => Status::Waiting,
                                    Some(true) => Status::Ready,
                                    Some(false) => Status::Blocked,
                                };
                                assert_eq!(
                                    f.journal.queued_writer_status(writers[index]),
                                    Ok(Some(status))
                                );
                            }
                            let success = successes & (1 << index) != 0;
                            if success && !ready {
                                assert_eq!(
                                    f.journal.settle_success(
                                        writers[index],
                                        &ContextWriterSuccessEvidenceV1 {
                                            writer: writers[index]
                                        }
                                    ),
                                    Err(Error::AllocationBusy)
                                );
                            }
                            outcomes[index] = Some(success && ready);
                            if success && ready {
                                f.success(writers[index]);
                                if index != 0 {
                                    epoch += 1;
                                }
                                lineage = epoch;
                            } else {
                                f.cancel(writers[index]);
                            }
                            for write in &f.writes {
                                let state = f.journal.lookup_allocation(write.allocation).unwrap();
                                assert_eq!(
                                    (state.attempt_epoch, state.content_lineage),
                                    (epoch, lineage)
                                );
                            }
                            assert_eq!(storage, self::storage(&f.journal));
                        }
                        assert_eq!(f.journal.remaining_member_slots(), 12);
                    }
                }
            }
        }
    }
}

#[test]
fn admission_and_cancellation_indexed_work_is_independent_of_queue_depth() {
    let mut expected = None;
    for depth in [1, 4, 12] {
        let mut f = Fixture::new(1, 16);
        let mut parent = f.active(&[0]);
        for _ in 1..depth {
            parent = f.queued(&[(0, Some(parent))]);
        }
        let writer = f.writer();
        let requests = [ContextQueuedWriteV1 {
            destination: f.writes[0],
            predecessor: Some(parent),
        }];
        f.journal.inner.reset_access_count_for_test_v1();
        f.journal.begin_queued_write(writer, &requests).unwrap();
        let admission = f.journal.inner.guard_accesses_for_test_v1();
        f.journal.inner.reset_access_count_for_test_v1();
        f.journal
            .settle_no_effect(writer, &ContextWriterNoEffectEvidenceV1 { writer })
            .unwrap();
        let cancellation = f.journal.inner.guard_accesses_for_test_v1();
        assert!(admission > 0 && cancellation > 0);
        assert_eq!(
            *expected.get_or_insert((admission, cancellation)),
            (admission, cancellation)
        );
        audit(&f.journal);
    }
}

#[test]
fn unknown_middle_writer_cannot_refund_or_authorize_a_descendant() {
    let mut f = Fixture::new(2, 8);
    let a = f.active(&[0, 1]);
    let b = f.queued(&[(0, Some(a)), (1, Some(a))]);
    let c = f.queued(&[(0, Some(b)), (1, Some(b))]);
    f.journal.mark_unknown(b).unwrap();
    f.success(a);
    let before = snapshot(&f.journal);
    assert_eq!(
        f.journal
            .settle_no_effect(b, &ContextWriterNoEffectEvidenceV1 { writer: b }),
        Err(Error::InvalidState)
    );
    assert_eq!(
        f.journal.activate_queued_writer(c),
        Err(Error::AllocationBusy)
    );
    assert_eq!(snapshot(&f.journal), before);
    f.cancel(c);
    for write in &f.writes {
        assert_eq!(f.journal.latest_writer(write.allocation), Ok(Some(b)));
        assert_eq!(
            f.journal.validate_no_queued_writer(write.allocation),
            Err(Error::AllocationBusy)
        );
    }
    audit(&f.journal);
}
