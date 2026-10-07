use super::*;

fn dispose(j: &mut Journal, writer: Reference, roster: &[Write]) -> Result<(), Error> {
    j.dispose_unknown(
        writer,
        &ContextWriterDisposalEvidenceV1 {
            writer,
            allocations: roster,
        },
    )
}

fn assert_rejected(j: &mut Journal, writer: Reference, roster: &[Write], error: Error) {
    let before = snapshot(j);
    assert_eq!(j.validate_unknown_disposal(writer, roster), Err(error));
    assert_eq!(dispose(j, writer, roster), Err(error));
    assert_eq!(snapshot(j), before);
}

#[test]
fn complete_unknown_disposal_removes_exact_roster_and_preserves_unrelated_state() {
    for count in [0, 1, 3, 8] {
        for kind in [Kind::Synchronous, Kind::Submission] {
            let (mut j, writer, roster) = pending_kind(count, kind);
            j.mark_unknown(writer).unwrap();
            if count == 3 {
                let neighbor = j
                    .writers
                    .iter()
                    .enumerate()
                    .find_map(|(slot, entry)| {
                        if let Some(WriterEntryV1::Reserved(key)) = entry {
                            Some(Reference { slot, key: *key })
                        } else {
                            None
                        }
                    })
                    .unwrap();
                j.begin_write(neighbor, &roster[3..5]).unwrap();
                if kind == Kind::Submission {
                    j.mark_unknown(neighbor).unwrap();
                }
            }
            let mut expected = snapshot(&j);
            let slots = chain(&j, writer);
            for slot in slots {
                let member = expected.members[slot].take().unwrap();
                expected.allocations[member.allocation.slot] = None;
                expected.allocation_free.push(member.allocation.slot);
                expected.member_free.push(slot);
            }
            expected.writers[writer.slot] = None;
            expected.free.push(writer.slot);
            j.validate_unknown_disposal(writer, &roster[..count])
                .unwrap();
            dispose(&mut j, writer, &roster[..count]).unwrap();
            assert_eq!(snapshot(&j), expected);
            audit(&j);
            for destination in &roster[..count] {
                assert_eq!(
                    j.lookup_allocation(destination.allocation),
                    Err(Error::InvalidAllocationReference)
                );
            }
            assert_rejected(&mut j, writer, &roster[..count], Error::InvalidReference);
        }
    }
}

#[test]
fn disposal_requires_unknown_and_exact_writer_evidence() {
    for kind in [Kind::Synchronous, Kind::Submission] {
        let (mut j, writer, roster) = reserved_kind(8, 4, kind);
        assert_rejected(&mut j, writer, &roster[..3], Error::InvalidReference);
        j.begin_write(writer, &roster[..3]).unwrap();
        assert_rejected(&mut j, writer, &roster[..3], Error::InvalidState);
        j.mark_unknown(writer).unwrap();
        for coordinate in 0..4 {
            let mut other = writer;
            match coordinate {
                0 => other.slot = usize::MAX,
                1 => other.key.local += 1,
                2 => other.key.context_generation += 1,
                _ => {
                    other.key.kind = if kind == Kind::Synchronous {
                        Kind::Submission
                    } else {
                        Kind::Synchronous
                    }
                }
            }
            assert_rejected(&mut j, other, &roster[..3], Error::InvalidReference);
            let before = snapshot(&j);
            assert_eq!(
                j.dispose_unknown(
                    writer,
                    &ContextWriterDisposalEvidenceV1 {
                        writer: other,
                        allocations: &roster[..3],
                    }
                ),
                Err(Error::SettlementEvidenceMismatch)
            );
            assert_eq!(snapshot(&j), before);
        }
    }
}

#[test]
fn disposal_rejects_partial_reordered_and_substituted_rosters_atomically() {
    let (mut j, writer, roster) = pending(3);
    j.mark_unknown(writer).unwrap();
    for count in [0, 1, 2, 4, 8] {
        assert_rejected(
            &mut j,
            writer,
            &roster[..count],
            Error::SettlementEvidenceMismatch,
        );
    }
    for index in 0..3 {
        for coordinate in 0..8 {
            let mut changed = roster[..3].to_vec();
            match coordinate {
                0 => changed[index].allocation.slot = usize::MAX,
                1 => changed[index].allocation.key.local += 1,
                2 => changed[index].allocation.key.context_generation += 1,
                3 => changed[index].device.local += 1,
                4 => changed[index].device.context_generation += 1,
                5 => changed[index].byte_extent += 1,
                6 => changed.swap(index, (index + 1) % 3),
                _ => changed[index] = changed[(index + 1) % 3],
            }
            assert_rejected(&mut j, writer, &changed, Error::SettlementEvidenceMismatch);
        }
    }
    audit(&j);
}

#[test]
fn disposal_checks_every_member_backlink_epoch_and_chain_before_mutation() {
    for index in 0..3 {
        for corruption in 0..8 {
            let (mut j, writer, roster) = pending(3);
            j.mark_unknown(writer).unwrap();
            let slots = chain(&j, writer);
            let slot = slots[index];
            let allocation = j.members[slot].unwrap().allocation.slot;
            match corruption {
                0 => j.members[slot] = None,
                1 => j.members[slot].as_mut().unwrap().next = Some(slot),
                2 => j.members[slot].as_mut().unwrap().allocation.slot = usize::MAX,
                3 => j.members[slot].as_mut().unwrap().writer.key.local += 1,
                4 => j.members[slot].as_mut().unwrap().attempt_epoch += 1,
                5 => j.members[slot].as_mut().unwrap().prior_lineage += 1,
                6 => j.allocations[allocation].as_mut().unwrap().pending_member = None,
                _ => j.allocations[allocation] = None,
            }
            assert_rejected(&mut j, writer, &roster[..3], Error::InvalidState);
        }
    }
}

#[test]
fn disposal_preflights_all_return_stacks_and_scratch_without_allocating() {
    for corruption in 0..8 {
        let (mut j, writer, roster) = pending(3);
        j.mark_unknown(writer).unwrap();
        match corruption {
            0 => j.free = core::mem::take(&mut j.free).into_boxed_slice().into_vec(),
            1 => {
                j.member_free = core::mem::take(&mut j.member_free)
                    .into_boxed_slice()
                    .into_vec()
            }
            2 => {
                j.allocation_free = core::mem::take(&mut j.allocation_free)
                    .into_boxed_slice()
                    .into_vec()
            }
            3 => {
                j.free.reserve_exact(j.writer_capacity + 1);
                j.free.resize(j.writer_capacity, usize::MAX);
            }
            4 => {
                j.member_free.reserve_exact(j.allocation_capacity + 3);
                j.member_free.resize(j.allocation_capacity, usize::MAX);
            }
            5 => {
                j.allocation_free.reserve_exact(j.allocation_capacity + 3);
                j.allocation_free.resize(j.allocation_capacity, usize::MAX);
            }
            6 => j.scratch.truncate(2),
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
        assert_rejected(&mut j, writer, &roster[..3], Error::InvalidState);
    }
}

#[test]
fn disposed_identity_cannot_alias_reused_writer_allocation_or_member_slots() {
    let (mut j, old, roster) = pending(3);
    j.mark_unknown(old).unwrap();
    let old_slots = chain(&j, old);
    dispose(&mut j, old, &roster[..3]).unwrap();
    let new = j.register_writer(key(old.key.local + 1)).unwrap();
    assert_eq!(new.slot, old.slot);
    let mut replacement = Vec::new();
    for (index, prior) in roster[..3].iter().enumerate() {
        let allocation = j
            .enroll_allocation(
                ContextAllocationKeyV1 {
                    context_generation: 7,
                    local: 50_000 + index as u64,
                },
                prior.device,
                prior.byte_extent,
            )
            .unwrap();
        assert_eq!(allocation.slot, roster[2 - index].allocation.slot);
        replacement.push(Write {
            allocation,
            ..*prior
        });
    }
    j.begin_write(new, &replacement).unwrap();
    let mut expected_slots = old_slots;
    expected_slots.reverse();
    assert_eq!(chain(&j, new), expected_slots);
    j.mark_unknown(new).unwrap();
    assert_rejected(&mut j, old, &roster[..3], Error::InvalidReference);
    assert_rejected(&mut j, new, &roster[..3], Error::SettlementEvidenceMismatch);
    dispose(&mut j, new, &replacement).unwrap();
    audit(&j);
}
