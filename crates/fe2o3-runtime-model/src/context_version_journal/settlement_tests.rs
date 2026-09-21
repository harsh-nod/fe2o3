use super::*;

#[path = "disposal_tests.rs"]
mod disposal;

type Write = ContextAllocationWriteV1;
type Success = ContextWriterSuccessEvidenceV1;
type NoEffect = ContextWriterNoEffectEvidenceV1;

fn reserved(allocations: usize, writers: usize) -> (Journal, Reference, Vec<Write>) {
    reserved_kind(allocations, writers, Kind::Synchronous)
}

fn reserved_kind(
    allocations: usize,
    writers: usize,
    kind: Kind,
) -> (Journal, Reference, Vec<Write>) {
    let mut j = Journal::new(7, allocations, writers).unwrap();
    let device = ContextJournalDeviceKeyV1 {
        context_generation: 7,
        local: 1,
    };
    let mut roster = Vec::new();
    for index in 0..allocations {
        let allocation = j
            .enroll_allocation(
                ContextAllocationKeyV1 {
                    context_generation: 7,
                    local: (100 + allocations - index) as u64,
                },
                device,
                64,
            )
            .unwrap();
        roster.push(Write {
            allocation,
            device,
            byte_extent: 64,
        });
    }
    roster.reverse();
    let mut writer = None;
    for index in 0..writers {
        writer = Some(
            j.register_writer(Key {
                kind,
                ..key(10_000 + index as u64)
            })
            .unwrap(),
        );
    }
    audit(&j);
    (j, writer.unwrap(), roster)
}

fn pending(count: usize) -> (Journal, Reference, Vec<Write>) {
    pending_kind(count, Kind::Synchronous)
}

fn pending_kind(count: usize, kind: Kind) -> (Journal, Reference, Vec<Write>) {
    let (mut j, writer, roster) = reserved_kind(8, 4, kind);
    j.begin_write(writer, &roster[..count]).unwrap();
    audit(&j);
    (j, writer, roster)
}

fn chain(j: &Journal, writer: Reference) -> Vec<usize> {
    let (mut head, count) = match j.writers[writer.slot].unwrap() {
        WriterEntryV1::Pending { head, count, .. } | WriterEntryV1::Unknown { head, count, .. } => {
            (head, count)
        }
        _ => panic!("retained writer required"),
    };
    let mut slots = Vec::new();
    for _ in 0..count {
        let slot = head.unwrap();
        slots.push(slot);
        head = j.members[slot].unwrap().next;
    }
    assert_eq!(head, None);
    slots
}

fn settle(
    j: &mut Journal,
    writer: Reference,
    evidence: Reference,
    success: bool,
) -> Result<(), Error> {
    if success {
        j.settle_success(writer, &Success { writer: evidence })
    } else {
        j.settle_no_effect(writer, &NoEffect { writer: evidence })
    }
}

#[test]
fn settlements_return_exact_canonical_members_and_preserve_unrelated_storage() {
    for count in [0, 1, 3, 8] {
        for (success, kind) in [
            (false, Kind::Synchronous),
            (true, Kind::Synchronous),
            (false, Kind::Submission),
            (true, Kind::Submission),
        ] {
            let (mut j, writer, _) = pending_kind(count, kind);
            let mut expected = snapshot(&j);
            let slots = chain(&j, writer);
            expected.writers[writer.slot] = None;
            expected.free.push(writer.slot);
            for slot in slots {
                let member = expected.members[slot].take().unwrap();
                let allocation = expected.allocations[member.allocation.slot]
                    .as_mut()
                    .unwrap();
                if success {
                    allocation.content_lineage = member.attempt_epoch;
                }
                allocation.pending_member = None;
                expected.member_free.push(slot);
            }
            settle(&mut j, writer, writer, success).unwrap();
            assert_eq!(snapshot(&j), expected);
            audit(&j);
        }
    }
}

#[test]
fn settlement_preserves_unreachable_corruption_and_free_prefixes() {
    for faults in 0..16 {
        for success in [false, true] {
            let (mut j, writer, roster) = pending(3);
            let slots = chain(&j, writer);
            if faults & 1 != 0 {
                let slot = j.member_free.pop().unwrap();
                let allocation = roster[3].allocation;
                j.members[slot] = Some(MemberEntryV1 {
                    writer,
                    allocation,
                    prior_lineage: 0,
                    attempt_epoch: 1,
                    next: None,
                });
                let entry = j.allocations[allocation.slot].as_mut().unwrap();
                entry.pending_member = Some(slot);
                entry.attempt_epoch = 1;
            }
            if faults & 2 != 0 {
                j.allocations[roster[4].allocation.slot]
                    .as_mut()
                    .unwrap()
                    .pending_member = Some(slots[0]);
            }
            if faults & 4 != 0 {
                j.free.extend([writer.slot, writer.slot]);
                j.member_free[0] = slots[0];
                j.member_free[1] = slots[0];
            }
            if faults & 8 != 0 {
                let poison = Some(BeginMemberPlanV1 {
                    member_slot: usize::MAX,
                    allocation: roster[7].allocation,
                    prior_lineage: u64::MAX,
                    attempt_epoch: 0,
                });
                j.scratch[3] = poison;
                j.scratch[7] = poison;
            }
            // Raw settlement validates the reachable chain, not unrelated arena state.
            let mut expected = snapshot(&j);
            expected.writers[writer.slot] = None;
            expected.free.push(writer.slot);
            for slot in slots {
                let member = expected.members[slot].take().unwrap();
                let entry = expected.allocations[member.allocation.slot]
                    .as_mut()
                    .unwrap();
                if success {
                    entry.content_lineage = member.attempt_epoch;
                }
                entry.pending_member = None;
                expected.member_free.push(slot);
            }
            j.indexed_accesses.set(0);
            assert_eq!(settle(&mut j, writer, writer, success), Ok(()));
            assert_eq!(j.indexed_accesses.get(), 30);
            assert_eq!(snapshot(&j), expected, "faults={faults}, success={success}");
        }
    }
}

#[test]
fn empty_settlement_does_not_require_unrelated_arena_shapes() {
    for success in [false, true] {
        let (mut j, writer, _) = pending(0);
        j.allocations.clear();
        j.members.clear();
        j.scratch.clear();
        let mut expected = snapshot(&j);
        expected.writers[writer.slot] = None;
        expected.free.push(writer.slot);
        j.indexed_accesses.set(0);
        assert_eq!(settle(&mut j, writer, writer, success), Ok(()));
        assert_eq!(j.indexed_accesses.get(), 3);
        assert_eq!(snapshot(&j), expected);
    }
}

#[test]
fn immutable_settlement_preflight_matches_ranked_fault_combinations() {
    for count in [0, 3] {
        for header_fault in 0..5 {
            for evidence_fault in [false, true] {
                for chain_fault in 0..3 {
                    for capacity_fault in 0..3 {
                        for scratch_fault in 0..3 {
                            for success in [false, true] {
                                let (mut j, writer, _) = pending(count);
                                let slots = chain(&j, writer);
                                let head = slots.first().copied();
                                let mut supplied = writer;
                                match header_fault {
                                    0 => {}
                                    1 => supplied.key.context_generation += 1,
                                    2 => supplied.key.local += 1,
                                    3 => supplied.key.kind = Kind::Submission,
                                    _ => supplied.slot = usize::MAX,
                                }
                                let mut evidence = supplied;
                                if evidence_fault {
                                    evidence.key.local += 1;
                                }
                                if chain_fault != 0 {
                                    if count == 0 {
                                        j.writers[writer.slot] = Some(WriterEntryV1::Pending {
                                            key: writer.key,
                                            head: Some(usize::MAX),
                                            count: 0,
                                        });
                                    } else if chain_fault == 1 {
                                        j.members[slots[0]]
                                            .as_mut()
                                            .unwrap()
                                            .allocation
                                            .key
                                            .local += 1;
                                    } else {
                                        j.members[*slots.last().unwrap()].as_mut().unwrap().next =
                                            Some(usize::MAX);
                                    }
                                }
                                match capacity_fault {
                                    0 => {}
                                    1 => {
                                        j.free = core::mem::take(&mut j.free)
                                            .into_boxed_slice()
                                            .into_vec()
                                    }
                                    _ => {
                                        j.member_free = core::mem::take(&mut j.member_free)
                                            .into_boxed_slice()
                                            .into_vec()
                                    }
                                }
                                match scratch_fault {
                                    0 => {}
                                    1 => {
                                        j.scratch[0] = Some(BeginMemberPlanV1 {
                                            member_slot: usize::MAX,
                                            allocation: ContextAllocationReferenceV1 {
                                                slot: usize::MAX,
                                                key: ContextAllocationKeyV1 {
                                                    context_generation: 7,
                                                    local: 1,
                                                },
                                            },
                                            prior_lineage: 0,
                                            attempt_epoch: 1,
                                        })
                                    }
                                    _ => j.scratch.truncate(count.saturating_sub(1)),
                                }
                                // Fault ranks are fixture inputs, independent of the production decision code.
                                let expected = if header_fault != 0 {
                                    Err(Error::InvalidReference)
                                } else if evidence_fault {
                                    Err(Error::SettlementEvidenceMismatch)
                                } else if chain_fault != 0
                                    || capacity_fault == 1
                                    || (count != 0 && (capacity_fault == 2 || scratch_fault != 0))
                                {
                                    Err(Error::InvalidState)
                                } else {
                                    Ok((head, count))
                                };
                                let preflight_accesses = if header_fault != 0
                                    || evidence_fault
                                    || (count == 0 && chain_fault != 0)
                                {
                                    1
                                } else if chain_fault == 1 {
                                    3
                                } else if chain_fault == 2
                                    || capacity_fault == 1
                                    || (count != 0 && (capacity_fault == 2 || scratch_fault == 2))
                                {
                                    1 + 2 * count
                                } else if count != 0 && scratch_fault == 1 {
                                    2 + 2 * count
                                } else {
                                    1 + 3 * count
                                };
                                let mut after = snapshot(&j);
                                j.indexed_accesses.set(0);
                                assert_eq!(j.preflight_settlement(supplied, evidence), expected);
                                assert_eq!(j.indexed_accesses.get(), preflight_accesses);
                                assert_eq!(
                                    snapshot(&j),
                                    after,
                                    "immutable preflight changed storage"
                                );
                                if expected.is_ok() {
                                    for slot in slots {
                                        let member = after.members[slot].take().unwrap();
                                        let allocation = after.allocations[member.allocation.slot]
                                            .as_mut()
                                            .unwrap();
                                        if success {
                                            allocation.content_lineage = member.attempt_epoch;
                                        }
                                        allocation.pending_member = None;
                                        after.member_free.push(slot);
                                    }
                                    after.writers[writer.slot] = None;
                                    after.free.push(writer.slot);
                                }
                                j.indexed_accesses.set(0);
                                assert_eq!(
                                    settle(&mut j, supplied, evidence, success),
                                    expected.map(|_| ())
                                );
                                assert_eq!(
                                    j.indexed_accesses.get(),
                                    if expected.is_ok() {
                                        9 * count + 3
                                    } else {
                                        preflight_accesses
                                    }
                                );
                                assert_eq!(
                                    snapshot(&j),
                                    after,
                                    "exact settlement and storage frame"
                                );
                            }
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn no_effect_burns_epochs_and_success_advances_lineage_across_gaps() {
    let (mut j, mut writer, roster) = pending(3);
    let original_storage = storage(&j);
    let mut lineage = 0;
    for epoch in 1..=12 {
        let success = epoch % 4 == 0;
        settle(&mut j, writer, writer, success).unwrap();
        if success {
            lineage = epoch;
        }
        for destination in &roster[..3] {
            let state = j.lookup_allocation(destination.allocation).unwrap();
            assert_eq!(
                (
                    state.attempt_epoch,
                    state.content_lineage,
                    state.pending_writer
                ),
                (epoch, lineage, None)
            );
        }
        assert_eq!(storage(&j), original_storage);
        audit(&j);
        if epoch < 12 {
            writer = j.register_writer(key(writer.key.local + 1)).unwrap();
            j.begin_write(writer, &roster[..3]).unwrap();
        }
    }
}

#[test]
fn unknown_is_sticky_revalidated_and_retains_empty_and_nonempty_writers() {
    for count in [0, 1, 3] {
        for kind in [Kind::Synchronous, Kind::Submission] {
            let (mut j, writer, _) = pending_kind(count, kind);
            let mut expected = snapshot(&j);
            let Some(WriterEntryV1::Pending { key, head, count }) = expected.writers[writer.slot]
            else {
                unreachable!()
            };
            expected.writers[writer.slot] = Some(WriterEntryV1::Unknown { key, head, count });
            j.mark_unknown(writer).unwrap();
            assert_eq!(snapshot(&j), expected);
            assert_eq!(
                j.lookup_writer(writer),
                Ok(ContextWriterStateV1::Unknown {
                    member_count: count
                })
            );
            j.mark_unknown(writer).unwrap();
            assert_eq!(snapshot(&j), expected);
            for success in [false, true] {
                assert_eq!(
                    settle(&mut j, writer, writer, success),
                    Err(Error::InvalidReference)
                );
                assert_eq!(snapshot(&j), expected);
            }
            assert_eq!(j.begin_write(writer, &[]), Err(Error::InvalidReference));
            assert_eq!(j.abort_reserved(writer), Err(Error::InvalidReference));
            assert_eq!(snapshot(&j), expected);
            audit(&j);
        }
    }
}

#[test]
fn exact_reserved_writers_reject_settlement_and_unknown_before_evidence() {
    for kind in [Kind::Synchronous, Kind::Submission] {
        let (mut j, writer, _) = reserved_kind(8, 4, kind);
        let before = snapshot(&j);
        for evidence in [
            writer,
            Reference {
                slot: usize::MAX,
                ..writer
            },
        ] {
            for success in [false, true] {
                assert_eq!(
                    settle(&mut j, writer, evidence, success),
                    Err(Error::InvalidReference)
                );
                assert_eq!(snapshot(&j), before);
            }
        }
        assert_eq!(j.mark_unknown(writer), Err(Error::InvalidReference));
        assert_eq!(snapshot(&j), before);
        audit(&j);
    }
}

#[test]
fn full_reference_and_evidence_rejections_precede_retained_corruption() {
    for success in [false, true] {
        for coordinate in 0..5 {
            let (mut j, writer, _) = pending(3);
            let mut altered = writer;
            match coordinate {
                0 => altered.slot = usize::MAX,
                1 => altered.key.context_generation += 1,
                2 => altered.key.local += 1,
                3 => altered.key.kind = Kind::Submission,
                _ => altered.slot = 0,
            }
            let head = chain(&j, writer)[0];
            j.members[head].as_mut().unwrap().next = None;
            let before = snapshot(&j);
            assert_eq!(
                settle(&mut j, altered, altered, success),
                Err(Error::InvalidReference)
            );
            assert_eq!(snapshot(&j), before);
            assert_eq!(
                settle(&mut j, writer, altered, success),
                Err(Error::SettlementEvidenceMismatch)
            );
            assert_eq!(snapshot(&j), before);
            assert_eq!(
                settle(&mut j, writer, writer, success),
                Err(Error::InvalidState)
            );
            assert_eq!(snapshot(&j), before);
        }
    }
}

#[test]
fn every_retained_member_rejects_corruption_before_any_scratch_write() {
    for index in 0..3 {
        for corruption in 0..14 {
            for operation in 0..4 {
                let (mut j, writer, _) = pending(3);
                if operation == 3 {
                    j.mark_unknown(writer).unwrap();
                }
                let slots = chain(&j, writer);
                let slot = slots[index];
                let allocation_slot = j.members[slot].unwrap().allocation.slot;
                match corruption {
                    0 => j.members[slot] = None,
                    1 => j.members[slot].as_mut().unwrap().writer.key.kind = Kind::Submission,
                    2 => j.members[slot].as_mut().unwrap().allocation.slot = usize::MAX,
                    3 => {
                        j.members[slot]
                            .as_mut()
                            .unwrap()
                            .allocation
                            .key
                            .context_generation += 1
                    }
                    4 => {
                        j.allocations[allocation_slot]
                            .as_mut()
                            .unwrap()
                            .pending_member = None
                    }
                    5 => j.members[slot].as_mut().unwrap().attempt_epoch += 1,
                    6 => j.members[slot].as_mut().unwrap().prior_lineage += 1,
                    7 => j.members[slot].as_mut().unwrap().next = Some(slot),
                    8 => j.allocations[allocation_slot] = None,
                    9 => {
                        j.members[slot].as_mut().unwrap().prior_lineage = 1;
                        j.allocations[allocation_slot]
                            .as_mut()
                            .unwrap()
                            .content_lineage = 1;
                    }
                    10 => {
                        let other = j.members[slots[(index + 1) % 3]].unwrap().allocation;
                        j.members[slot].as_mut().unwrap().allocation = other;
                    }
                    11 => j.members[slot].as_mut().unwrap().writer.slot = 0,
                    12 => j.members[slot].as_mut().unwrap().next = Some(usize::MAX),
                    _ => j.members[slot].as_mut().unwrap().allocation.key.local += 100_000,
                }
                let before = snapshot(&j);
                let result = match operation {
                    0 => settle(&mut j, writer, writer, true),
                    1 => settle(&mut j, writer, writer, false),
                    _ => j.mark_unknown(writer),
                };
                assert_eq!(result, Err(Error::InvalidState));
                assert_eq!(snapshot(&j), before);
            }
        }
    }
}

#[test]
fn retained_prior_lineage_mismatch_below_admitted_epoch_rejects_atomically() {
    for index in 0..3 {
        for change_allocation in [false, true] {
            for operation in 0..4 {
                let (mut j, mut writer, roster) = pending(3);
                settle(&mut j, writer, writer, true).unwrap();
                writer = j.register_writer(key(writer.key.local + 1)).unwrap();
                j.begin_write(writer, &roster[..3]).unwrap();
                settle(&mut j, writer, writer, false).unwrap();
                writer = j.register_writer(key(writer.key.local + 1)).unwrap();
                j.begin_write(writer, &roster[..3]).unwrap();
                if operation == 3 {
                    j.mark_unknown(writer).unwrap();
                }
                audit(&j);
                let slot = chain(&j, writer)[index];
                let member = j.members[slot].unwrap();
                assert_eq!((member.prior_lineage, member.attempt_epoch), (1, 3));
                if change_allocation {
                    j.allocations[member.allocation.slot]
                        .as_mut()
                        .unwrap()
                        .content_lineage = 2;
                } else {
                    j.members[slot].as_mut().unwrap().prior_lineage = 2;
                }
                let before = snapshot(&j);
                j.indexed_accesses.set(0);
                let result = if operation < 2 {
                    settle(&mut j, writer, writer, operation == 0)
                } else {
                    j.mark_unknown(writer)
                };
                assert_eq!(result, Err(Error::InvalidState));
                assert_eq!(j.indexed_accesses.get(), 2 * index + 3);
                assert_eq!(snapshot(&j), before);
            }
        }
    }
}

#[test]
fn header_cardinality_and_noncanonical_chains_reject_atomically() {
    for corruption in 0..7 {
        for operation in 0..3 {
            let (mut j, writer, _) = pending(3);
            let slots = chain(&j, writer);
            let Some(WriterEntryV1::Pending { head, count, .. }) = j.writers[writer.slot].as_mut()
            else {
                unreachable!()
            };
            match corruption {
                0 => *head = None,
                1 => *count = 0,
                2 => *count = usize::MAX,
                3 => *count = 2,
                4 => j.members[slots[1]].as_mut().unwrap().next = None,
                5 => {
                    *head = Some(slots[2]);
                    j.members[slots[2]].as_mut().unwrap().next = Some(slots[1]);
                    j.members[slots[1]].as_mut().unwrap().next = Some(slots[0]);
                    j.members[slots[0]].as_mut().unwrap().next = None;
                }
                _ => *head = Some(usize::MAX),
            }
            let before = snapshot(&j);
            let result = if operation == 2 {
                j.mark_unknown(writer)
            } else {
                settle(&mut j, writer, writer, operation == 0)
            };
            assert_eq!(result, Err(Error::InvalidState));
            assert_eq!(snapshot(&j), before);
        }
    }
}

#[test]
fn release_headroom_and_scratch_reject_but_do_not_gate_unknown() {
    for corruption in 0..6 {
        let (mut j, writer, _) = pending(3);
        match corruption {
            0 => j.free = core::mem::take(&mut j.free).into_boxed_slice().into_vec(),
            1 => {
                j.member_free = core::mem::take(&mut j.member_free)
                    .into_boxed_slice()
                    .into_vec()
            }
            2 => {
                j.free.reserve_exact(j.writer_capacity + 1);
                j.free.resize(j.writer_capacity, usize::MAX);
                assert!(j.free.capacity() > j.writer_capacity);
            }
            3 => {
                j.member_free.reserve_exact(j.allocation_capacity + 3);
                j.member_free.resize(j.allocation_capacity, usize::MAX);
                assert!(j.member_free.capacity() >= j.allocation_capacity + 3);
            }
            4 => j.scratch.truncate(2),
            _ => {
                let slot = chain(&j, writer)[2];
                let member = j.members[slot].unwrap();
                j.scratch[2] = Some(BeginMemberPlanV1 {
                    member_slot: slot,
                    allocation: member.allocation,
                    prior_lineage: member.prior_lineage,
                    attempt_epoch: member.attempt_epoch,
                });
            }
        }
        let before = snapshot(&j);
        for success in [false, true] {
            assert_eq!(
                settle(&mut j, writer, writer, success),
                Err(Error::InvalidState)
            );
            assert_eq!(snapshot(&j), before);
        }
        j.mark_unknown(writer).unwrap();
        let after = snapshot(&j);
        let mut expected = before;
        let Some(WriterEntryV1::Pending { key, head, count }) = expected.writers[writer.slot]
        else {
            unreachable!()
        };
        expected.writers[writer.slot] = Some(WriterEntryV1::Unknown { key, head, count });
        assert_eq!(after, expected);
        j.mark_unknown(writer).unwrap();
        assert_eq!(snapshot(&j), after);
    }
}

#[test]
fn settlement_replay_cannot_alias_a_reused_writer_slot() {
    for success in [false, true] {
        let (mut j, old, roster) = pending(3);
        let evidence = Success { writer: old };
        let no_effect = NoEffect { writer: old };
        settle(&mut j, old, old, success).unwrap();
        let new = j.register_writer(key(old.key.local + 1)).unwrap();
        assert_eq!(new.slot, old.slot);
        j.begin_write(new, &roster[..3]).unwrap();
        let before = snapshot(&j);
        assert_eq!(
            j.settle_success(old, &evidence),
            Err(Error::InvalidReference)
        );
        assert_eq!(
            j.settle_no_effect(old, &no_effect),
            Err(Error::InvalidReference)
        );
        assert_eq!(
            j.settle_success(new, &evidence),
            Err(Error::SettlementEvidenceMismatch)
        );
        assert_eq!(
            j.settle_no_effect(new, &no_effect),
            Err(Error::SettlementEvidenceMismatch)
        );
        assert_eq!(evidence.writer, old);
        assert_eq!(no_effect.writer, old);
        assert_eq!(snapshot(&j), before);
        audit(&j);
    }
}

#[test]
fn final_admitted_epoch_settles_without_rolling_back_exhaustion() {
    for success in [false, true] {
        let (mut j, writer, roster) = reserved(8, 4);
        for destination in &roster[..3] {
            let allocation = j.allocations[destination.allocation.slot].as_mut().unwrap();
            allocation.attempt_epoch = u64::MAX - 1;
            allocation.content_lineage = u64::MAX - 2;
        }
        j.begin_write(writer, &roster[..3]).unwrap();
        settle(&mut j, writer, writer, success).unwrap();
        for destination in &roster[..3] {
            let state = j.lookup_allocation(destination.allocation).unwrap();
            assert_eq!(state.attempt_epoch, u64::MAX);
            assert_eq!(
                state.content_lineage,
                if success { u64::MAX } else { u64::MAX - 2 }
            );
            assert_eq!(state.pending_writer, None);
        }
        let next = j.register_writer(key(writer.key.local + 1)).unwrap();
        let before = snapshot(&j);
        assert_eq!(
            j.begin_write(next, &roster[..3]),
            Err(Error::EpochExhausted)
        );
        assert_eq!(snapshot(&j), before);
        audit(&j);
    }
}

#[test]
fn settlement_work_depends_on_touched_members_not_unrelated_populations() {
    for (allocations, writers) in [(8, 4), (64, 32), (512, 128)] {
        for count in [0, 1, 3] {
            for operation in 0..3 {
                let (mut j, writer, roster) = reserved(allocations, writers);
                j.begin_write(writer, &roster[..count]).unwrap();
                let before = storage(&j);
                j.indexed_accesses.set(0);
                if operation < 2 {
                    settle(&mut j, writer, writer, operation == 0).unwrap();
                    assert_eq!(j.indexed_accesses.get(), 9 * count + 3);
                } else {
                    j.mark_unknown(writer).unwrap();
                    assert_eq!(j.indexed_accesses.get(), 2 * count + 2);
                    j.indexed_accesses.set(0);
                    j.mark_unknown(writer).unwrap();
                    assert_eq!(j.indexed_accesses.get(), 2 * count + 1);
                }
                assert_eq!(storage(&j), before);
                audit(&j);
            }
        }
    }
}

#[test]
fn rejected_settlement_work_is_bounded_by_the_examined_prefix() {
    for (allocations, writers) in [(8, 4), (64, 32), (512, 128)] {
        for operation in 0..4 {
            for corruption in 0..9 {
                for index in 0..3 {
                    let (mut j, writer, roster) = reserved(allocations, writers);
                    j.begin_write(writer, &roster[..3]).unwrap();
                    if operation == 3 {
                        j.mark_unknown(writer).unwrap();
                    }
                    let slots = chain(&j, writer);
                    let slot = slots[index];
                    let member = j.members[slot].unwrap();
                    let mut supplied = writer;
                    let mut evidence = writer;
                    let (error, accesses) = match corruption {
                        0 => {
                            supplied.key.context_generation += 1;
                            (Error::InvalidReference, 1)
                        }
                        1 if operation < 2 => {
                            evidence.key.kind = Kind::Submission;
                            j.members[slot] = None;
                            (Error::SettlementEvidenceMismatch, 1)
                        }
                        1 => {
                            supplied.key.local += 1;
                            (Error::InvalidReference, 1)
                        }
                        2 => {
                            j.members[slot] = None;
                            (Error::InvalidState, 2 * index + 2)
                        }
                        3 => {
                            j.members[slot].as_mut().unwrap().writer.slot = usize::MAX;
                            (Error::InvalidState, 2 * index + 2)
                        }
                        4 => {
                            j.members[slot].as_mut().unwrap().allocation.slot = usize::MAX;
                            (Error::InvalidState, 2 * index + 3)
                        }
                        5 => {
                            j.allocations[member.allocation.slot]
                                .as_mut()
                                .unwrap()
                                .pending_member = None;
                            (Error::InvalidState, 2 * index + 3)
                        }
                        6 => {
                            j.members[slot].as_mut().unwrap().prior_lineage += 1;
                            (Error::InvalidState, 2 * index + 3)
                        }
                        7 => {
                            j.members[slots[2]].as_mut().unwrap().next = Some(usize::MAX);
                            (Error::InvalidState, 7)
                        }
                        _ => {
                            let entry = j.writers[writer.slot].as_mut().unwrap();
                            match entry {
                                WriterEntryV1::Pending { count, .. }
                                | WriterEntryV1::Unknown { count, .. } => *count = usize::MAX,
                                _ => unreachable!(),
                            }
                            (Error::InvalidState, 1)
                        }
                    };
                    let before = snapshot(&j);
                    j.indexed_accesses.set(0);
                    let result = if operation < 2 {
                        settle(&mut j, supplied, evidence, operation == 0)
                    } else {
                        j.mark_unknown(supplied)
                    };
                    assert_eq!(result, Err(error));
                    assert_eq!(j.indexed_accesses.get(), accesses);
                    assert_eq!(snapshot(&j), before);
                }
            }
        }
    }
}

#[test]
fn rejected_release_cost_covers_each_scratch_cell_and_return_limit() {
    for (allocations, writers) in [(8, 4), (64, 32), (512, 128)] {
        for success in [false, true] {
            for corruption in 0..8 {
                let (mut j, writer, roster) = reserved(allocations, writers);
                j.begin_write(writer, &roster[..3]).unwrap();
                let accesses = match corruption {
                    0 => {
                        j.free = core::mem::take(&mut j.free).into_boxed_slice().into_vec();
                        7
                    }
                    1 => {
                        j.member_free = core::mem::take(&mut j.member_free)
                            .into_boxed_slice()
                            .into_vec();
                        7
                    }
                    2 => {
                        j.free.reserve_exact(writers + 1);
                        j.free.resize(writers, usize::MAX);
                        assert!(j.free.capacity() > writers);
                        7
                    }
                    3 => {
                        j.member_free.reserve_exact(allocations + 3);
                        j.member_free.resize(allocations, usize::MAX);
                        assert!(j.member_free.capacity() >= allocations + 3);
                        7
                    }
                    4 => {
                        j.scratch.truncate(2);
                        7
                    }
                    _ => {
                        let index = corruption - 5;
                        let slot = chain(&j, writer)[index];
                        let member = j.members[slot].unwrap();
                        j.scratch[index] = Some(BeginMemberPlanV1 {
                            member_slot: slot,
                            allocation: member.allocation,
                            prior_lineage: member.prior_lineage,
                            attempt_epoch: member.attempt_epoch,
                        });
                        8 + index
                    }
                };
                let before = snapshot(&j);
                j.indexed_accesses.set(0);
                assert_eq!(
                    settle(&mut j, writer, writer, success),
                    Err(Error::InvalidState)
                );
                assert_eq!(j.indexed_accesses.get(), accesses);
                assert_eq!(snapshot(&j), before);
            }
        }
    }
}

#[test]
fn settlement_routes_bounded_preflight_before_plan_and_commit() {
    let source = include_str!("settlement.rs");
    for forbidden in [
        "try_reserve",
        ".reserve(",
        ".reserve_exact(",
        ".resize",
        ".extend(",
        ".collect(",
        "Vec::",
        "Box::",
        "vec!",
        "to_vec",
        "to_owned",
        ".clone(",
        "while ",
        "loop {",
        ".iter(",
        ".iter_mut(",
        ".into_iter(",
        "0..self.allocation_capacity",
        "0..self.writer_capacity",
        "self.registration_watermark =",
        "self.reserved_count =",
    ] {
        assert!(
            !source.contains(forbidden),
            "settlement contains {forbidden}"
        );
    }
    assert_eq!(source.matches("for _ in 0..count").count(), 1);
    assert_eq!(source.matches("for index in 0..count").count(), 3);
    assert_eq!(source.matches("for ").count(), 4);
    assert_eq!(source.matches(".push(").count(), 1);
    assert!(source.contains("self.member_free.push(plan.member_slot)"));
    assert!(source.contains("self.settle_retained(writer, evidence.writer, true)"));
    assert!(source.contains("self.settle_retained(writer, evidence.writer, false)"));

    let release = source.split("fn settle_retained(").nth(1).unwrap();
    let plan = release.find("self.store_plan(").unwrap();
    assert!(
        release
            .find("self.preflight_settlement(writer, evidence)?")
            .unwrap()
            < plan
    );
    let preflight = source
        .split("fn preflight_settlement(")
        .nth(1)
        .unwrap()
        .split("fn settle_retained(")
        .next()
        .unwrap();
    assert!(preflight.contains("&self,"));
    assert!(!preflight.contains("&mut self"));
    let mut previous = 0;
    for validation in [
        "self.retained_header(writer, false)?",
        "SettlementEvidenceMismatch",
        "self.validate_retained_chain(writer, head, count)?",
        "writer_returns > self.writer_capacity",
        "writer_returns > self.free.capacity()",
        "member_returns > self.allocation_capacity",
        "member_returns > self.member_free.capacity()",
        "count > self.scratch.len()",
        "self.scratch[index].is_some()",
    ] {
        let position = preflight.find(validation).unwrap();
        assert!(position >= previous, "out-of-order preflight: {validation}");
        previous = position;
    }
    let commit = release.find("allocation.pending_member = None").unwrap();
    assert!(plan < commit);
    assert!(
        commit
            < release
                .find("self.member_free.push(plan.member_slot)")
                .unwrap()
    );
    assert!(
        release
            .find("self.member_free.push(plan.member_slot)")
            .unwrap()
            < release.find("self.store_slot(writer.slot, None)").unwrap()
    );
    assert!(
        release.find("self.store_slot(writer.slot, None)").unwrap()
            < release.find("self.push_free(writer.slot)").unwrap()
    );

    let unknown = source
        .split("pub fn mark_unknown(")
        .nth(1)
        .unwrap()
        .split("fn retained_header(")
        .next()
        .unwrap();
    assert!(
        unknown
            .find("self.validate_retained_chain(writer, head, count)?")
            .unwrap()
            < unknown.find("self.store_slot(").unwrap()
    );
    assert!(unknown.contains("self.retained_header(writer, true)?"));
    for forbidden in [
        "self.scratch",
        "self.free",
        "self.member_free",
        "self.store_plan",
    ] {
        assert!(!unknown.contains(forbidden));
    }
}

#[path = "settlement_reference_tests.rs"]
mod reference_traces;

#[test]
fn settlement_preserves_nonempty_allocation_free_stack() {
    for count in [0_usize, 2] {
        for mode in 0..3 {
            let mut j = Journal::new(7, 4, 2).unwrap();
            let device = ContextJournalDeviceKeyV1 {
                context_generation: 7,
                local: 1,
            };
            let mut roster = Vec::new();
            for index in 0..count {
                let allocation = j
                    .enroll_allocation(
                        ContextAllocationKeyV1 {
                            context_generation: 7,
                            local: 100 + index as u64,
                        },
                        device,
                        64,
                    )
                    .unwrap();
                roster.push(Write {
                    allocation,
                    device,
                    byte_extent: 64,
                });
            }
            let writer = j.register_writer(key(41)).unwrap();
            j.begin_write(writer, &roster).unwrap();
            let before = snapshot(&j);
            assert!(before.allocation_free.len() >= 2);
            let wrong = Reference {
                slot: usize::MAX,
                ..writer
            };
            for success in [false, true] {
                assert_eq!(
                    settle(&mut j, writer, wrong, success),
                    Err(Error::SettlementEvidenceMismatch)
                );
                assert_eq!(snapshot(&j), before, "rejected evidence changed state");
            }
            assert_eq!(j.mark_unknown(wrong), Err(Error::InvalidReference));
            assert_eq!(snapshot(&j), before, "rejected Unknown changed state");
            if mode < 2 {
                settle(&mut j, writer, writer, mode == 1).unwrap();
            } else {
                j.mark_unknown(writer).unwrap();
            }
            assert_eq!(
                j.allocation_free, before.allocation_free,
                "settlement changed allocation-free order: mode={mode} count={count}"
            );
            assert_eq!(storage(&j), before.storage);
            if mode == 2 {
                let retained = snapshot(&j);
                j.mark_unknown(writer).unwrap();
                assert_eq!(snapshot(&j), retained, "repeated Unknown changed state");
            }
            audit(&j);
            let expected_slot = *before.allocation_free.last().unwrap();
            let enrolled = j
                .enroll_allocation(
                    ContextAllocationKeyV1 {
                        context_generation: 7,
                        local: 900,
                    },
                    device,
                    64,
                )
                .unwrap();
            assert_eq!(
                enrolled.slot, expected_slot,
                "settlement changed next enrollment"
            );
            assert_eq!(
                j.allocation_free,
                before.allocation_free[..before.allocation_free.len() - 1]
            );
            assert_eq!(storage(&j), before.storage);
            audit(&j);
        }
    }
}

#[test]
fn unknown_custody_rejects_fresh_reserved_begin_atomically() {
    for position in 0..3 {
        let mut j = Journal::new(7, 4, 2).unwrap();
        let device = ContextJournalDeviceKeyV1 {
            context_generation: 7,
            local: 1,
        };
        let mut roster = Vec::new();
        for local in [100, 200, 300] {
            let allocation = j
                .enroll_allocation(
                    ContextAllocationKeyV1 {
                        context_generation: 7,
                        local,
                    },
                    device,
                    64,
                )
                .unwrap();
            roster.push(Write {
                allocation,
                device,
                byte_extent: 64,
            });
        }
        let owner = j.register_writer(key(41)).unwrap();
        j.begin_write(owner, &[roster[position]]).unwrap();
        j.mark_unknown(owner).unwrap();
        let contender = j.register_writer(key(44)).unwrap();
        assert_eq!(j.member_free.len(), roster.len());
        let before = snapshot(&j);
        assert_eq!(
            j.begin_write(contender, &roster),
            Err(Error::AllocationBusy),
            "Unknown custody must reject fresh Reserved Begin: position={position}"
        );
        assert_eq!(
            snapshot(&j),
            before,
            "Unknown conflict must preserve all state: position={position}"
        );
        j.mark_unknown(owner).unwrap();
        assert_eq!(
            snapshot(&j),
            before,
            "Unknown must remain revalidatable after conflict: position={position}"
        );
        audit(&j);
    }
}
