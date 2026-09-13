use super::*;

type AllocationKey = ContextAllocationKeyV1;
type AllocationReference = ContextAllocationReferenceV1;
type DeviceKey = ContextJournalDeviceKeyV1;
type Write = ContextAllocationWriteV1;
type WriterState = ContextWriterStateV1;

fn allocation_key(local: u64) -> AllocationKey {
    AllocationKey {
        context_generation: 7,
        local,
    }
}

fn device() -> DeviceKey {
    DeviceKey {
        context_generation: 7,
        local: 11,
    }
}

fn enroll(journal: &mut Journal, locals: &[u64]) -> Vec<Write> {
    locals
        .iter()
        .map(|local| {
            let byte_extent = local + 64;
            Write {
                allocation: journal
                    .enroll_allocation(allocation_key(*local), device(), byte_extent)
                    .unwrap(),
                device: device(),
                byte_extent,
            }
        })
        .collect()
}

fn fixture() -> (Journal, Reference, Vec<Write>) {
    let mut journal = Journal::new(7, 4, 3).unwrap();
    let mut roster = enroll(&mut journal, &[100, 200, 300, 900]);
    let writer = journal.register_writer(key(41)).unwrap();
    let neighbor = journal.register_writer(key(44)).unwrap();
    journal
        .begin_write(neighbor, &[roster.pop().unwrap()])
        .unwrap();
    audit(&journal);
    (journal, writer, roster)
}

fn rejected_begin(journal: &mut Journal, writer: Reference, roster: &[Write], error: Error) {
    let before = snapshot(journal);
    assert_eq!(journal.begin_write(writer, roster), Err(error));
    assert_eq!(
        snapshot(journal),
        before,
        "whole journal and scratch remain unchanged"
    );
}

fn rejected_enrollment(
    journal: &mut Journal,
    key: AllocationKey,
    device: DeviceKey,
    extent: u64,
    error: Error,
) {
    let before = snapshot(journal);
    assert_eq!(journal.enroll_allocation(key, device, extent), Err(error));
    assert_eq!(snapshot(journal), before);
    audit(journal);
}

#[test]
fn enrollment_uses_complete_coordinates_without_an_allocation_watermark() {
    let mut journal = Journal::new(7, 3, 1).unwrap();
    let roster = enroll(&mut journal, &[300, 100, 200]);
    for (slot, destination) in roster.iter().enumerate() {
        assert_eq!(destination.allocation.slot, slot);
        assert_eq!(
            journal.lookup_allocation(destination.allocation),
            Ok(ContextAllocationStateV1 {
                device: device(),
                byte_extent: destination.byte_extent,
                attempt_epoch: 0,
                content_lineage: 0,
                pending_writer: None,
            })
        );
    }
    assert_eq!(journal.registration_watermark(), 0);
    for local in [0, u64::MAX] {
        rejected_enrollment(
            &mut journal,
            allocation_key(local),
            device(),
            64,
            Error::InvalidAllocationId,
        );
        rejected_enrollment(
            &mut journal,
            allocation_key(100),
            DeviceKey { local, ..device() },
            64,
            Error::InvalidDeviceId,
        );
    }
    rejected_enrollment(
        &mut journal,
        AllocationKey {
            context_generation: 8,
            local: 0,
        },
        device(),
        0,
        Error::ForeignContext,
    );
    rejected_enrollment(
        &mut journal,
        allocation_key(0),
        DeviceKey {
            context_generation: 8,
            ..device()
        },
        0,
        Error::InvalidAllocationId,
    );
    rejected_enrollment(
        &mut journal,
        allocation_key(100),
        DeviceKey {
            context_generation: 8,
            local: 0,
        },
        0,
        Error::ForeignContext,
    );
    rejected_enrollment(
        &mut journal,
        allocation_key(100),
        device(),
        0,
        Error::InvalidExtent,
    );
    rejected_enrollment(
        &mut journal,
        allocation_key(100),
        DeviceKey {
            local: 12,
            ..device()
        },
        999,
        Error::AllocationReplay,
    );
    rejected_enrollment(
        &mut journal,
        allocation_key(400),
        device(),
        64,
        Error::AllocationCapacity,
    );
    audit(&journal);
    for context_generation in [1, u64::MAX - 1] {
        for local in [1, u64::MAX - 1] {
            for device_local in [1, u64::MAX - 1] {
                let mut journal = Journal::new(context_generation, 1, 1).unwrap();
                let key = AllocationKey {
                    context_generation,
                    local,
                };
                let device = DeviceKey {
                    context_generation,
                    local: device_local,
                };
                let allocation = journal.enroll_allocation(key, device, 64).unwrap();
                assert_eq!(allocation.key, key);
                let writer = journal
                    .register_writer(Key {
                        context_generation,
                        local: 2,
                        kind: Kind::Synchronous,
                    })
                    .unwrap();
                journal
                    .begin_write(
                        writer,
                        &[Write {
                            allocation,
                            device,
                            byte_extent: 64,
                        }],
                    )
                    .unwrap();
                let state = journal.lookup_allocation(allocation).unwrap();
                assert_eq!(
                    (
                        state.device,
                        state.byte_extent,
                        state.attempt_epoch,
                        state.content_lineage,
                        state.pending_writer
                    ),
                    (device, 64, 1, 0, Some(writer))
                );
                audit(&journal);
            }
        }
    }
}

#[test]
fn empty_begin_retains_writer_capacity_without_members_or_epochs() {
    let mut journal = Journal::new(7, 3, 1).unwrap();
    let _roster = enroll(&mut journal, &[100, 200, 300]);
    let writer = journal.register_writer(key(41)).unwrap();
    let before = snapshot(&journal);
    journal.begin_write(writer, &[]).unwrap();
    assert_eq!(
        journal.lookup_writer(writer),
        Ok(WriterState::Pending { member_count: 0 })
    );
    assert_eq!(
        journal.lookup_reserved(writer),
        Err(Error::InvalidReference)
    );
    assert_eq!(journal.reserved_writer_count(), 0);
    assert_eq!(journal.remaining_writer_slots(), 0);
    assert_eq!(journal.allocations, before.allocations);
    assert_eq!(journal.allocation_free, before.allocation_free);
    assert_eq!(journal.members, before.members);
    assert_eq!(journal.member_free, before.member_free);
    assert_eq!(journal.scratch, before.scratch);
    assert_eq!(storage(&journal), before.storage);
    {
        let _discarded_copy = writer;
    }
    rejected_reference(&mut journal, writer);
    rejected_begin(&mut journal, writer, &[], Error::InvalidReference);
    rejected_registration(&mut journal, key(42), Error::WriterCapacity);
    audit(&journal);
}

#[test]
fn canonical_keys_not_slot_order_bind_complete_chains_for_older_reservations() {
    let mut journal = Journal::new(7, 4, 3).unwrap();
    let mut roster = enroll(&mut journal, &[300, 100, 200, 900]);
    let older = journal.register_writer(key(41)).unwrap();
    let newer = journal.register_writer(key(44)).unwrap();
    journal
        .begin_write(newer, &[roster.pop().unwrap()])
        .unwrap();
    roster.sort_by_key(|destination| destination.allocation.key);
    assert_eq!(
        roster.iter().map(|d| d.allocation.slot).collect::<Vec<_>>(),
        [1, 2, 0]
    );
    let original_roster = roster.clone();
    let original_storage = storage(&journal);
    journal.begin_write(older, &roster).unwrap();
    assert_eq!(roster, original_roster);
    assert_eq!(storage(&journal), original_storage);
    assert_eq!(journal.registration_watermark(), 44);
    assert_eq!(journal.reserved_writer_count(), 0);
    assert_eq!(
        journal.lookup_writer(older),
        Ok(WriterState::Pending { member_count: 3 })
    );
    let Some(WriterEntryV1::Pending {
        mut head, count: 3, ..
    }) = journal.writers[older.slot]
    else {
        panic!("complete pending writer")
    };
    for destination in roster {
        let member = journal.members[head.unwrap()].unwrap();
        assert_eq!(member.writer, older);
        assert_eq!(member.allocation, destination.allocation);
        assert_eq!((member.prior_lineage, member.attempt_epoch), (0, 1));
        let state = journal.lookup_allocation(destination.allocation).unwrap();
        assert_eq!((state.attempt_epoch, state.content_lineage), (1, 0));
        assert_eq!(state.pending_writer, Some(older));
        head = member.next;
    }
    assert_eq!(head, None);
    audit(&journal);
}

#[test]
fn first_middle_last_bad_destinations_leave_every_owner_and_scratch_unchanged() {
    for position in 0..3 {
        for coordinate in 0..6 {
            let (mut journal, writer, roster) = fixture();
            let mut bad = roster.clone();
            let destination = &mut bad[position];
            let error = match coordinate {
                0 => {
                    destination.allocation.slot = usize::MAX;
                    Error::InvalidAllocationReference
                }
                1 => {
                    destination.allocation.key.local += 1;
                    Error::InvalidAllocationReference
                }
                2 => {
                    destination.device.context_generation += 1;
                    Error::AllocationDeviceMismatch
                }
                3 => {
                    destination.device.local += 1;
                    Error::AllocationDeviceMismatch
                }
                4 => {
                    destination.byte_extent += 1;
                    Error::AllocationExtentMismatch
                }
                _ => {
                    destination.byte_extent = 0;
                    Error::AllocationExtentMismatch
                }
            };
            rejected_begin(&mut journal, writer, &bad, error);
            audit(&journal);
            journal.begin_write(writer, &roster).unwrap();
            audit(&journal);
        }
    }
    for position in 0..3 {
        let mut journal = Journal::new(7, 3, 2).unwrap();
        let roster = enroll(&mut journal, &[100, 200, 300]);
        let owner = journal.register_writer(key(41)).unwrap();
        let conflicting = journal.register_writer(key(44)).unwrap();
        journal.begin_write(owner, &[roster[position]]).unwrap();
        rejected_begin(
            &mut journal,
            conflicting,
            &[roster[position]],
            Error::AllocationBusy,
        );
        rejected_begin(&mut journal, conflicting, &roster, Error::AllocationBusy);
        audit(&journal);
    }
}

#[test]
fn whole_roster_order_and_capacity_precede_destination_validation() {
    let (mut journal, writer, roster) = fixture();
    let mut unsorted = roster.clone();
    unsorted.swap(0, 1);
    rejected_begin(&mut journal, writer, &unsorted, Error::NonCanonicalRoster);
    for (context_generation, local, error) in [
        (8, 100, Error::NonCanonicalRoster),
        (6, 400, Error::InvalidAllocationReference),
    ] {
        let mut cross_context = vec![roster[0], roster[1]];
        cross_context[0].allocation.key = AllocationKey {
            context_generation,
            local,
        };
        rejected_begin(&mut journal, writer, &cross_context, error);
        audit(&journal);
    }
    for alternate_fields in [false, true] {
        let mut duplicate = vec![roster[0], roster[0]];
        if alternate_fields {
            duplicate[1].device.local += 1;
            duplicate[1].byte_extent += 1;
        }
        rejected_begin(&mut journal, writer, &duplicate, Error::NonCanonicalRoster);
    }
    let mut long = vec![roster[0]; 5];
    long[0].allocation.slot = usize::MAX;
    rejected_begin(&mut journal, writer, &long, Error::RosterCapacity);
    rejected_begin(
        &mut journal,
        Reference {
            slot: usize::MAX,
            ..writer
        },
        &long,
        Error::InvalidReference,
    );
    let mut bad_order_and_reference = unsorted;
    bad_order_and_reference[0].allocation.slot = usize::MAX;
    rejected_begin(
        &mut journal,
        writer,
        &bad_order_and_reference,
        Error::NonCanonicalRoster,
    );
    journal.begin_write(writer, &roster).unwrap();
    audit(&journal);
    for earlier_error in [
        Error::InvalidAllocationReference,
        Error::AllocationDeviceMismatch,
        Error::AllocationExtentMismatch,
        Error::EpochExhausted,
    ] {
        let (mut journal, writer, roster) = fixture();
        let mut bad = roster.clone();
        bad[0].device.local += 1;
        bad[0].byte_extent = 0;
        let error = match earlier_error {
            Error::InvalidAllocationReference => {
                bad[0].allocation.slot = usize::MAX;
                earlier_error
            }
            Error::AllocationDeviceMismatch => earlier_error,
            Error::AllocationExtentMismatch => {
                bad[0].device = device();
                earlier_error
            }
            _ => {
                bad[0] = roster[0];
                journal.allocations[roster[0].allocation.slot]
                    .as_mut()
                    .unwrap()
                    .attempt_epoch = u64::MAX;
                earlier_error
            }
        };
        bad[2].allocation.slot = usize::MAX;
        rejected_begin(&mut journal, writer, &bad, error);
        audit(&journal);
        // Per-destination errors precede defensive member-capacity rejection.
        let retained_free = journal.member_free.pop().unwrap();
        rejected_begin(&mut journal, writer, &bad, error);
        journal.member_free.push(retained_free);
        audit(&journal);
    }
    let mut journal = Journal::new(7, 1, 2).unwrap();
    let roster = enroll(&mut journal, &[100]);
    journal.allocations[roster[0].allocation.slot]
        .as_mut()
        .unwrap()
        .attempt_epoch = u64::MAX - 1;
    let owner = journal.register_writer(key(41)).unwrap();
    journal.begin_write(owner, &roster).unwrap();
    let other = journal.register_writer(key(44)).unwrap();
    let mut bad = roster[0];
    bad.device.local += 1;
    bad.byte_extent = 0;
    rejected_begin(&mut journal, other, &[bad], Error::AllocationDeviceMismatch);
    bad.device = device();
    rejected_begin(&mut journal, other, &[bad], Error::AllocationExtentMismatch);
    rejected_begin(&mut journal, other, &roster, Error::AllocationBusy);
    audit(&journal);
}

#[test]
fn allocation_writer_and_member_limits_have_independent_effects() {
    let mut journal = Journal::new(7, 1, 3).unwrap();
    let roster = enroll(&mut journal, &[100]);
    let first = journal.register_writer(key(41)).unwrap();
    let empty = journal.register_writer(key(42)).unwrap();
    let still_reserved = journal.register_writer(key(43)).unwrap();
    journal.begin_write(first, &roster).unwrap();
    assert!(journal.member_free.is_empty());
    rejected_enrollment(
        &mut journal,
        allocation_key(200),
        device(),
        64,
        Error::AllocationCapacity,
    );
    rejected_begin(&mut journal, still_reserved, &roster, Error::AllocationBusy);
    journal.begin_write(empty, &[]).unwrap();
    assert_eq!(journal.reserved_writer_count(), 1);
    journal.abort_reserved(still_reserved).unwrap();
    assert_eq!(journal.remaining_writer_slots(), 1);
    assert_eq!(
        journal.lookup_writer(first),
        Ok(WriterState::Pending { member_count: 1 })
    );
    assert_eq!(
        journal.lookup_writer(empty),
        Ok(WriterState::Pending { member_count: 0 })
    );
    audit(&journal);
    let mut journal = Journal::new(7, 3, 1).unwrap();
    let roster = enroll(&mut journal, &[100, 200, 300]);
    let writer = journal.register_writer(key(41)).unwrap();
    journal.begin_write(writer, &roster).unwrap();
    rejected_registration(&mut journal, key(42), Error::WriterCapacity);
    assert_eq!(journal.members.iter().flatten().count(), 3);
    audit(&journal);
}

#[test]
fn final_epoch_is_admitted_but_exhaustion_is_whole_roster_atomic() {
    for position in 0..3 {
        let (mut journal, writer, roster) = fixture();
        // Seed valid boundary states, not a claim of executing 2^64 writes.
        for destination in &roster {
            let entry = journal.allocations[destination.allocation.slot]
                .as_mut()
                .unwrap();
            entry.attempt_epoch = u64::MAX - 1;
            entry.content_lineage = 42;
        }
        journal.allocations[roster[position].allocation.slot]
            .as_mut()
            .unwrap()
            .attempt_epoch = u64::MAX;
        audit(&journal);
        rejected_begin(&mut journal, writer, &roster, Error::EpochExhausted);
        audit(&journal);
        journal.allocations[roster[position].allocation.slot]
            .as_mut()
            .unwrap()
            .attempt_epoch = u64::MAX - 1;
        journal.begin_write(writer, &roster).unwrap();
        for destination in &roster {
            let state = journal.lookup_allocation(destination.allocation).unwrap();
            assert_eq!((state.attempt_epoch, state.content_lineage), (u64::MAX, 42));
            assert_eq!(state.pending_writer, Some(writer));
            let index = journal.allocations[destination.allocation.slot]
                .unwrap()
                .pending_member
                .unwrap();
            assert_eq!(
                (
                    journal.members[index].unwrap().attempt_epoch,
                    journal.members[index].unwrap().prior_lineage
                ),
                (u64::MAX, 42)
            );
        }
        audit(&journal);
    }
}

#[test]
fn stale_copied_and_foreign_references_never_alias_reused_slots() {
    let mut journal = Journal::new(7, 1, 1).unwrap();
    let roster = enroll(&mut journal, &[100]);
    let old = journal.register_writer(key(41)).unwrap();
    journal.abort_reserved(old).unwrap();
    let writer = journal
        .register_writer(Key {
            kind: Kind::Submission,
            ..key(44)
        })
        .unwrap();
    assert_eq!(old.slot, writer.slot);
    for invalid in [
        old,
        Reference {
            slot: usize::MAX,
            ..writer
        },
        Reference {
            key: Key {
                context_generation: 8,
                ..writer.key
            },
            ..writer
        },
        Reference {
            key: Key {
                local: 45,
                ..writer.key
            },
            ..writer
        },
        Reference {
            key: Key {
                kind: Kind::Synchronous,
                ..writer.key
            },
            ..writer
        },
    ] {
        let before = snapshot(&journal);
        assert_eq!(journal.lookup_writer(invalid), Err(Error::InvalidReference));
        rejected_begin(&mut journal, invalid, &roster, Error::InvalidReference);
        rejected_reference(&mut journal, invalid);
        assert_eq!(snapshot(&journal), before);
    }
    let reference = roster[0].allocation;
    for invalid in [
        AllocationReference {
            slot: usize::MAX,
            ..reference
        },
        AllocationReference {
            key: allocation_key(101),
            ..reference
        },
        AllocationReference {
            key: AllocationKey {
                context_generation: 8,
                ..reference.key
            },
            ..reference
        },
    ] {
        let before = snapshot(&journal);
        assert_eq!(
            journal.lookup_allocation(invalid),
            Err(Error::InvalidAllocationReference)
        );
        rejected_begin(
            &mut journal,
            writer,
            &[Write {
                allocation: invalid,
                ..roster[0]
            }],
            Error::InvalidAllocationReference,
        );
        assert_eq!(snapshot(&journal), before);
        audit(&journal);
    }
    {
        let _copy = (writer, reference);
    }
    assert_eq!(journal.lookup_writer(writer), Ok(WriterState::Reserved));
    journal.begin_write(writer, &roster).unwrap();
    assert_eq!(
        journal.lookup_allocation(reference).unwrap().pending_writer,
        Some(writer)
    );
    rejected_begin(&mut journal, writer, &roster, Error::InvalidReference);
    audit(&journal);
}

#[test]
fn selected_private_slot_corruption_rejects_before_any_planning_write() {
    for position in 0..3 {
        for arena in 0..3 {
            let mut journal = Journal::new(7, 3, 1).unwrap();
            let roster = enroll(&mut journal, &[100, 200, 300]);
            let writer = journal.register_writer(key(41)).unwrap();
            let free_index = journal.member_free.len() - 1 - position;
            let member_slot = journal.member_free[free_index];
            let plan = BeginMemberPlanV1 {
                member_slot,
                allocation: roster[position].allocation,
                prior_lineage: 0,
                attempt_epoch: 1,
            };
            match arena {
                0 => journal.member_free[free_index] = usize::MAX,
                1 => {
                    journal.members[member_slot] = Some(MemberEntryV1 {
                        writer,
                        allocation: plan.allocation,
                        prior_lineage: 0,
                        attempt_epoch: 1,
                        next: None,
                    })
                }
                _ => journal.scratch[position] = Some(plan),
            }
            // Deliberately malformed private state is not a legal public trace.
            rejected_begin(&mut journal, writer, &roster, Error::InvalidState);
            match arena {
                0 => journal.member_free[free_index] = member_slot,
                1 => journal.members[member_slot] = None,
                _ => journal.scratch[position] = None,
            }
            audit(&journal);
            journal.begin_write(writer, &roster).unwrap();
            audit(&journal);
        }
    }
    let mut journal = Journal::new(7, 3, 1).unwrap();
    let roster = enroll(&mut journal, &[100, 200, 300]);
    let writer = journal.register_writer(key(41)).unwrap();
    let original_storage = storage(&journal);
    let last_free = journal.member_free.pop().unwrap();
    rejected_begin(&mut journal, writer, &roster, Error::MemberCapacity);
    journal.member_free.push(last_free);
    journal.scratch.truncate(2);
    rejected_begin(&mut journal, writer, &roster, Error::InvalidState);
    journal.scratch.resize_with(3, || None);
    journal.reserved_count = 0;
    rejected_begin(&mut journal, writer, &roster, Error::InvalidState);
    journal.reserved_count = 1;
    assert_eq!(storage(&journal), original_storage);
    audit(&journal);
    journal.begin_write(writer, &roster).unwrap();
    audit(&journal);
    for invalid in [0, 2, usize::MAX] {
        let mut journal = Journal::new(7, 2, 1).unwrap();
        let _first = enroll(&mut journal, &[100]);
        let slot = *journal.allocation_free.last().unwrap();
        *journal.allocation_free.last_mut().unwrap() = invalid;
        let before = snapshot(&journal);
        assert_eq!(
            journal.enroll_allocation(allocation_key(200), device(), 64),
            Err(Error::InvalidState)
        );
        assert_eq!(snapshot(&journal), before);
        *journal.allocation_free.last_mut().unwrap() = slot;
        audit(&journal);
        assert_eq!(
            journal
                .enroll_allocation(allocation_key(200), device(), 64)
                .unwrap()
                .slot,
            slot
        );
        audit(&journal);
    }
}

#[test]
fn fixed_roster_work_and_storage_are_independent_of_unrelated_capacities() {
    for a in [3, 64, 4096, 65_537] {
        for w in [2, 17, 257] {
            for k in [0, 1, 3] {
                let mut journal = Journal::new(7, a, w).unwrap();
                let roster = enroll(&mut journal, &[100, 200, 300]);
                let writer = journal.register_writer(key(41)).unwrap();
                let other = journal.register_writer(key(44)).unwrap();
                journal.begin_write(other, &[]).unwrap();
                let original_storage = storage(&journal);
                journal.indexed_accesses.set(0);
                journal.begin_write(writer, &roster[..k]).unwrap();
                assert_eq!(
                    journal.indexed_accesses.get(),
                    12 * k + 2,
                    "a={a} w={w} k={k}"
                );
                assert_eq!(storage(&journal), original_storage);
                for (index, destination) in roster.iter().enumerate() {
                    journal.indexed_accesses.set(0);
                    let state = journal.lookup_allocation(destination.allocation).unwrap();
                    assert_eq!(
                        journal.indexed_accesses.get(),
                        if index < k { 2 } else { 1 }
                    );
                    assert_eq!(state.pending_writer, (index < k).then_some(writer));
                }
                audit(&journal);
            }
        }
        let mut journal = Journal::new(7, a, 1).unwrap();
        journal.indexed_accesses.set(0);
        let _reference = journal
            .enroll_allocation(allocation_key(100), device(), 64)
            .unwrap();
        assert_eq!(
            journal.indexed_accesses.get(),
            a + 4,
            "enrollment, not Begin, scans A"
        );
        audit(&journal);
    }
}

struct ExpectedAllocation {
    descriptor: Write,
    epoch: u64,
    pending: Option<Reference>,
}

struct ReferenceModel {
    watermark: u64,
    writers: BTreeMap<u64, (Reference, Option<Vec<u64>>)>,
    allocations: BTreeMap<u64, ExpectedAllocation>,
}

impl ReferenceModel {
    fn reserved(&self, reference: Reference) -> bool {
        self.writers
            .get(&reference.key.local)
            .is_some_and(|(actual, members)| *actual == reference && members.is_none())
    }

    fn begin(&mut self, writer: Reference, roster: &[Write]) -> Result<(), Error> {
        if !self.reserved(writer) {
            return Err(Error::InvalidReference);
        }
        if roster.len() > 3 {
            return Err(Error::RosterCapacity);
        }
        let mut previous = None;
        for destination in roster {
            let key = destination.allocation.key;
            if previous.is_some_and(|prior| prior >= key) {
                return Err(Error::NonCanonicalRoster);
            }
            previous = Some(key);
        }
        let mut destinations = BTreeSet::new();
        for destination in roster {
            let allocation = self
                .allocations
                .get(&destination.allocation.key.local)
                .filter(|a| a.descriptor.allocation == destination.allocation)
                .ok_or(Error::InvalidAllocationReference)?;
            if destination.device != allocation.descriptor.device {
                return Err(Error::AllocationDeviceMismatch);
            }
            if destination.byte_extent != allocation.descriptor.byte_extent {
                return Err(Error::AllocationExtentMismatch);
            }
            if allocation.pending.is_some() {
                return Err(Error::AllocationBusy);
            }
            if allocation.epoch == u64::MAX {
                return Err(Error::EpochExhausted);
            }
            assert!(destinations.insert(destination.allocation.key.local));
        }
        for local in &destinations {
            let allocation = self.allocations.get_mut(local).unwrap();
            allocation.epoch += 1;
            allocation.pending = Some(writer);
        }
        self.writers.get_mut(&writer.key.local).unwrap().1 =
            Some(destinations.into_iter().collect());
        Ok(())
    }

    fn check(&self, journal: &Journal, history: &[Reference]) {
        assert_eq!(journal.registration_watermark(), self.watermark);
        assert_eq!(
            journal.reserved_writer_count(),
            self.writers
                .values()
                .filter(|(_, members)| members.is_none())
                .count()
        );
        assert_eq!(
            journal.remaining_writer_slots(),
            journal.writer_capacity() - self.writers.len()
        );
        let occupied: BTreeSet<_> = self
            .writers
            .values()
            .map(|(reference, _)| reference.slot)
            .collect();
        assert_eq!(occupied.len(), self.writers.len());
        for (slot, entry) in journal.writers.iter().enumerate() {
            assert_eq!(entry.is_some(), occupied.contains(&slot));
        }
        for (writer, expected_members) in self.writers.values() {
            if let Some(expected_members) = expected_members {
                let Some(WriterEntryV1::Pending { mut head, .. }) = journal.writers[writer.slot]
                else {
                    panic!("expected Pending")
                };
                for local in expected_members {
                    let expected = &self.allocations[local];
                    let member = journal.members[head.expect("complete expected chain")].unwrap();
                    assert_eq!(member.allocation, expected.descriptor.allocation);
                    assert_eq!(member.writer, *writer);
                    assert_eq!(
                        (member.attempt_epoch, member.prior_lineage),
                        (expected.epoch, 0)
                    );
                    head = member.next;
                }
                assert_eq!(head, None, "exact independent membership order and length");
            }
        }
        for reference in history {
            let expected = self
                .writers
                .get(&reference.key.local)
                .filter(|(actual, _)| actual == reference)
                .map(|(_, members)| {
                    members
                        .as_ref()
                        .map_or(WriterState::Reserved, |members| WriterState::Pending {
                            member_count: members.len(),
                        })
                })
                .ok_or(Error::InvalidReference);
            assert_eq!(journal.lookup_writer(*reference), expected);
            assert_eq!(
                journal.lookup_reserved(*reference),
                if self.reserved(*reference) {
                    Ok(reference.key)
                } else {
                    Err(Error::InvalidReference)
                }
            );
        }
        for allocation in self.allocations.values() {
            assert_eq!(
                journal.lookup_allocation(allocation.descriptor.allocation),
                Ok(ContextAllocationStateV1 {
                    device: allocation.descriptor.device,
                    byte_extent: allocation.descriptor.byte_extent,
                    attempt_epoch: allocation.epoch,
                    content_lineage: 0,
                    pending_writer: allocation.pending,
                })
            );
        }
        audit(journal);
    }
}

#[test]
fn short_traces_match_independent_writer_and_allocation_maps() {
    const ACTIONS: usize = 11;
    const DEPTH: u32 = 4;
    for capacity in [1, 3] {
        for trace in 0..ACTIONS.pow(DEPTH) {
            let mut journal = Journal::new(7, 3, capacity).unwrap();
            let roster = enroll(&mut journal, &[100, 200, 300]);
            let mut model = ReferenceModel {
                watermark: 0,
                writers: BTreeMap::new(),
                allocations: BTreeMap::new(),
            };
            for destination in &roster {
                model.allocations.insert(
                    destination.allocation.key.local,
                    ExpectedAllocation {
                        descriptor: *destination,
                        epoch: 0,
                        pending: None,
                    },
                );
            }
            let mut history = Vec::new();
            let mut steps = trace;
            for _ in 0..DEPTH {
                let action = steps % ACTIONS;
                steps /= ACTIONS;
                let before = snapshot(&journal);
                if action < 3 {
                    let writer = Key {
                        kind: if action == 1 {
                            Kind::Submission
                        } else {
                            Kind::Synchronous
                        },
                        ..key([1, 4, 2][action])
                    };
                    let error = if writer.local <= model.watermark {
                        Some(Error::WriterReplay)
                    } else if model.writers.len() == capacity {
                        Some(Error::WriterCapacity)
                    } else {
                        None
                    };
                    let actual = journal.register_writer(writer);
                    if let Some(error) = error {
                        assert_eq!(actual, Err(error));
                        assert_eq!(snapshot(&journal), before);
                    } else {
                        let reference = actual.unwrap();
                        assert_eq!(reference.key, writer);
                        assert!(reference.slot < capacity);
                        assert!(
                            model
                                .writers
                                .values()
                                .all(|(other, _)| other.slot != reference.slot)
                        );
                        model.writers.insert(writer.local, (reference, None));
                        model.watermark = writer.local;
                        history.push(reference);
                    }
                } else {
                    let reference = if matches!(action, 3 | 4 | 8) {
                        history.first()
                    } else {
                        history.last()
                    }
                    .copied()
                    .unwrap_or(Reference {
                        slot: usize::MAX,
                        key: key(42),
                    });
                    if action == 3 {
                        let valid = model.reserved(reference);
                        assert_eq!(
                            journal.abort_reserved(reference),
                            if valid {
                                Ok(())
                            } else {
                                Err(Error::InvalidReference)
                            }
                        );
                        if valid {
                            model.writers.remove(&reference.key.local);
                        } else {
                            assert_eq!(snapshot(&journal), before);
                        }
                    } else {
                        let destinations = match action {
                            4 => vec![roster[0]],
                            5 => vec![roster[0], roster[1]],
                            6 => Vec::new(),
                            7 => vec![Write {
                                byte_extent: roster[2].byte_extent + 1,
                                ..roster[2]
                            }],
                            8 => vec![roster[1], roster[0]],
                            9 => vec![roster[2]],
                            _ => roster.clone(),
                        };
                        let expected = model.begin(reference, &destinations);
                        assert_eq!(journal.begin_write(reference, &destinations), expected);
                        if expected.is_err() {
                            assert_eq!(snapshot(&journal), before);
                        }
                    }
                }
                assert_eq!(storage(&journal), before.storage);
                model.check(&journal, &history);
            }
        }
    }
}

#[test]
fn begin_routes_full_preflight_before_bounded_planning_and_commit() {
    let source = include_str!("../context_version_journal.rs");
    let begin = source
        .split("pub fn begin_write(")
        .nth(1)
        .unwrap()
        .split("fn read_allocation(")
        .next()
        .unwrap();
    let helpers = source
        .split("fn read_allocation(")
        .nth(1)
        .unwrap()
        .split("fn count_indexed_access(")
        .next()
        .unwrap();
    for forbidden in [
        "try_reserve",
        ".reserve(",
        "vec!",
        "Vec::",
        "Box::",
        "to_vec",
        "to_owned",
        ".push(",
        ".resize",
        ".collect(",
        ".sort",
        ".clone(",
        ".extend(",
    ] {
        assert!(
            !begin.contains(forbidden) && !helpers.contains(forbidden),
            "allocation/growth form: {forbidden}"
        );
    }
    for forbidden in [
        "for ",
        "while ",
        "loop {",
        ".iter(",
        ".iter_mut(",
        ".into_iter(",
        ".for_each(",
    ] {
        assert!(!helpers.contains(forbidden), "helper scan: {forbidden}");
    }
    for forbidden in [
        "try_reserve",
        ".push(",
        ".resize",
        ".collect(",
        ".sort",
        ".clone(",
        "while ",
        "loop {",
        "self.allocations.iter",
        "self.writers.iter",
        "self.members.iter",
        "0..self.allocation_capacity",
        "0..self.writer_capacity",
    ] {
        assert!(!begin.contains(forbidden), "Begin contains {forbidden}");
    }
    for traversal in [
        "canonical.windows(2)",
        "for destination in canonical",
        "canonical.iter().enumerate()",
        "for index in 0..count",
    ] {
        assert!(begin.contains(traversal));
    }
    let first_plan = begin.find("self.store_plan(index, plan)").unwrap();
    for validation in [
        "self.lookup_reserved(writer)?",
        "NonCanonicalRoster",
        "self.exact_allocation(destination.allocation)?",
        "AllocationDeviceMismatch",
        "AllocationExtentMismatch",
        "AllocationBusy",
        "EpochExhausted",
        "MemberCapacity",
        "self.scratch[index].is_some()",
    ] {
        assert!(
            begin.find(validation).unwrap() < first_plan,
            "late preflight: {validation}"
        );
    }
    assert!(first_plan < begin.find("self.scratch[index].take()").unwrap());
    assert!(
        begin.find("self.scratch[index].take()").unwrap()
            < begin.find("self.reserved_count = reserved_count").unwrap()
    );
    let plan_helper = source
        .split("fn store_plan(")
        .nth(1)
        .unwrap()
        .split("fn count_indexed_access(")
        .next()
        .unwrap();
    assert!(plan_helper.contains("self.scratch[index] = Some(plan)"));
}
