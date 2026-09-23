use super::*;
use alloc::collections::{BTreeMap, BTreeSet};
use alloc::vec;

type Journal = ContextVersionJournalV1;
type Key = ContextWriterKeyV1;
type Reference = ContextWriterReferenceV1;
type Error = ContextVersionJournalErrorV1;
type Kind = ContextWriterKindV1;

#[path = "writer_lifecycle_tests.rs"]
mod writer_lifecycle;

#[path = "scalar_enrollment_tests.rs"]
mod scalar_enrollment;

fn key(local: u64) -> Key {
    Key {
        context_generation: 7,
        local,
        kind: Kind::Synchronous,
    }
}

#[derive(Debug, Eq, PartialEq)]
struct Snapshot {
    context: u64,
    capacities: (usize, usize),
    watermark: u64,
    reserved_count: usize,
    writers: Vec<Option<WriterEntryV1>>,
    free: Vec<usize>,
    allocations: Vec<Option<AllocationEntryV1>>,
    allocation_free: Vec<usize>,
    members: Vec<Option<MemberEntryV1>>,
    member_free: Vec<usize>,
    scratch: Vec<Option<BeginMemberPlanV1>>,
    storage: [(usize, usize); 7],
}

fn storage(journal: &Journal) -> [(usize, usize); 7] {
    [
        (
            journal.writers.as_ptr() as usize,
            journal.writers.capacity(),
        ),
        (journal.free.as_ptr() as usize, journal.free.capacity()),
        (
            journal.allocations.as_ptr() as usize,
            journal.allocations.capacity(),
        ),
        (
            journal.allocation_free.as_ptr() as usize,
            journal.allocation_free.capacity(),
        ),
        (
            journal.members.as_ptr() as usize,
            journal.members.capacity(),
        ),
        (
            journal.member_free.as_ptr() as usize,
            journal.member_free.capacity(),
        ),
        (
            journal.scratch.as_ptr() as usize,
            journal.scratch.capacity(),
        ),
    ]
}

fn snapshot(journal: &Journal) -> Snapshot {
    Snapshot {
        context: journal.context_generation,
        capacities: (journal.allocation_capacity, journal.writer_capacity),
        watermark: journal.registration_watermark,
        reserved_count: journal.reserved_count,
        writers: journal.writers.clone(),
        free: journal.free.clone(),
        allocations: journal.allocations.clone(),
        allocation_free: journal.allocation_free.clone(),
        members: journal.members.clone(),
        member_free: journal.member_free.clone(),
        scratch: journal.scratch.clone(),
        storage: storage(journal),
    }
}

fn partition<T>(slots: &[Option<T>], free: &[usize]) {
    let mut vacant = vec![false; slots.len()];
    for slot in free {
        assert!(*slot < slots.len());
        assert!(!vacant[*slot], "duplicate free slot");
        vacant[*slot] = true;
    }
    for (slot, value) in slots.iter().enumerate() {
        assert_eq!(
            value.is_none(),
            vacant[slot],
            "exact free/occupied partition"
        );
    }
}

// Full arena scans are test-only, never part of issuance or touched Begin work.
fn audit(journal: &Journal) {
    assert!(issuable_context_id(journal.context_generation));
    assert!((1..=CONTEXT_VERSION_JOURNAL_MAX_ENTRIES_V1).contains(&journal.allocation_capacity));
    assert!((1..=CONTEXT_VERSION_JOURNAL_MAX_ENTRIES_V1).contains(&journal.writer_capacity));
    assert!(journal.registration_watermark < u64::MAX);
    assert_eq!(journal.writers.len(), journal.writer_capacity);
    assert_eq!(journal.allocations.len(), journal.allocation_capacity);
    assert_eq!(journal.members.len(), journal.allocation_capacity);
    assert_eq!(journal.scratch.len(), journal.allocation_capacity);
    for (index, (_, capacity)) in storage(journal).into_iter().enumerate() {
        assert!(
            capacity
                >= if index < 2 {
                    journal.writer_capacity
                } else {
                    journal.allocation_capacity
                }
        );
    }
    partition(&journal.writers, &journal.free);
    partition(&journal.allocations, &journal.allocation_free);
    partition(&journal.members, &journal.member_free);
    assert!(journal.scratch.iter().all(Option::is_none));
    let mut locals = BTreeSet::new();
    let mut reserved_count = 0;
    let mut seen = vec![false; journal.allocation_capacity];
    for (slot, writer) in journal.writers.iter().enumerate() {
        let (key, mut head, count) = match writer {
            None => continue,
            Some(WriterEntryV1::Reserved(key)) => {
                reserved_count += 1;
                (*key, None, 0)
            }
            Some(WriterEntryV1::Pending { key, head, count })
            | Some(WriterEntryV1::Unknown { key, head, count }) => (*key, *head, *count),
        };
        assert_eq!(key.context_generation, journal.context_generation);
        assert!(issuable_context_id(key.local));
        assert!(key.local <= journal.registration_watermark);
        assert!(locals.insert(key.local), "reused local writer identity");
        assert!(count <= journal.allocation_capacity);
        let writer_reference = Reference { slot, key };
        let mut previous_key = None;
        for _ in 0..count {
            let index = head.expect("complete retained membership");
            assert!(index < journal.members.len());
            assert!(!seen[index], "cycle or shared member");
            seen[index] = true;
            let member = journal.members[index].expect("live chain member");
            assert_eq!(member.writer, writer_reference);
            let allocation = journal.allocations[member.allocation.slot].unwrap();
            assert_eq!(allocation.key, member.allocation.key);
            assert_eq!(allocation.pending_member, Some(index));
            assert_eq!(allocation.attempt_epoch, member.attempt_epoch);
            assert_eq!(allocation.content_lineage, member.prior_lineage);
            assert!(member.prior_lineage < member.attempt_epoch);
            assert!(previous_key.is_none_or(|key| key < member.allocation.key));
            previous_key = Some(member.allocation.key);
            head = member.next;
        }
        assert_eq!(head, None, "exact chain cardinality");
    }
    assert_eq!(reserved_count, journal.reserved_writer_count());
    assert_eq!(locals.len() + journal.free.len(), journal.writer_capacity);
    let mut allocation_keys = BTreeSet::new();
    for (slot, allocation) in journal.allocations.iter().enumerate() {
        let Some(allocation) = allocation else {
            continue;
        };
        assert_eq!(
            allocation.key.context_generation,
            journal.context_generation
        );
        assert!(issuable_context_id(allocation.key.local));
        assert!(allocation_keys.insert(allocation.key));
        assert_eq!(
            allocation.device.context_generation,
            journal.context_generation
        );
        assert!(issuable_context_id(allocation.device.local));
        assert!(allocation.byte_extent > 0);
        assert!(allocation.content_lineage <= allocation.attempt_epoch);
        if let Some(index) = allocation.pending_member {
            assert!(seen[index], "orphaned allocation backlink");
            assert_eq!(
                journal.members[index].unwrap().allocation,
                ContextAllocationReferenceV1 {
                    slot,
                    key: allocation.key
                }
            );
        }
    }
    for (index, member) in journal.members.iter().enumerate() {
        assert_eq!(member.is_some(), seen[index], "orphaned membership node");
    }
}

fn rejected_registration(journal: &mut Journal, writer: Key, expected: Error) {
    let before = snapshot(journal);
    assert_eq!(journal.register_writer(writer), Err(expected));
    assert_eq!(snapshot(journal), before);
    audit(journal);
}

fn rejected_reference(journal: &mut Journal, reference: Reference) {
    let before = snapshot(journal);
    assert_eq!(
        journal.lookup_reserved(reference),
        Err(Error::InvalidReference)
    );
    assert_eq!(snapshot(journal), before);
    assert_eq!(
        journal.abort_reserved(reference),
        Err(Error::InvalidReference)
    );
    assert_eq!(snapshot(journal), before);
    audit(journal);
}

#[test]
fn constructor_has_explicit_independent_bounds_and_zero_watermark() {
    let maximum = CONTEXT_VERSION_JOURNAL_MAX_ENTRIES_V1;
    for (a, w) in [(1, 1), (maximum, 1), (1, maximum), (65_537, 3)] {
        let journal = Journal::new(7, a, w).unwrap();
        assert_eq!(journal.context_generation(), 7);
        assert_eq!(journal.allocation_capacity(), a);
        assert_eq!(journal.writer_capacity(), w);
        assert_eq!(journal.remaining_writer_slots(), w);
        assert_eq!(journal.reserved_writer_count(), 0);
        assert_eq!(journal.registration_watermark(), 0);
        audit(&journal);
    }
    for invalid in [0, maximum + 1, usize::MAX] {
        assert!(matches!(
            Journal::new(7, invalid, 1),
            Err(Error::InvalidCapacity)
        ));
        assert!(matches!(
            Journal::new(7, 1, invalid),
            Err(Error::InvalidCapacity)
        ));
    }
    for invalid in [0, u64::MAX] {
        assert!(matches!(
            Journal::new(invalid, 1, 1),
            Err(Error::InvalidContextGeneration)
        ));
    }
    for generation in [1, u64::MAX - 1] {
        let journal = Journal::new(generation, 1, 1).unwrap();
        assert_eq!(journal.context_generation(), generation);
        audit(&journal);
    }
}

#[test]
fn existing_ids_preserve_gaps_and_older_reserved_lookup_and_abort() {
    let mut journal = Journal::new(7, 1, 3).unwrap();
    let first = journal.register_writer(key(41)).unwrap();
    let later_key = Key {
        kind: Kind::Submission,
        ..key(44)
    };
    let later = journal.register_writer(later_key).unwrap();
    assert_eq!(first.key, key(41));
    assert_eq!(later.key, later_key);
    assert_ne!(first.slot, later.slot);
    assert_eq!(journal.registration_watermark(), 44);
    assert_eq!(journal.lookup_reserved(first), Ok(key(41)));
    assert_eq!(journal.lookup_reserved(later), Ok(later_key));
    rejected_registration(&mut journal, key(42), Error::WriterReplay);
    rejected_registration(&mut journal, key(44), Error::WriterReplay);
    rejected_reference(
        &mut journal,
        Reference {
            slot: 2,
            key: key(42),
        },
    );
    assert_eq!(journal.abort_reserved(first), Ok(()));
    assert_eq!(journal.registration_watermark(), 44);
    assert_eq!(journal.lookup_reserved(later), Ok(later_key));
    audit(&journal);
}

#[test]
fn abort_never_rolls_back_watermark_or_revives_old_keys() {
    let mut journal = Journal::new(7, 2, 3).unwrap();
    let first = journal.register_writer(key(41)).unwrap();
    let last = journal.register_writer(key(44)).unwrap();
    journal.abort_reserved(last).unwrap();
    assert_eq!(journal.registration_watermark(), 44);
    assert_eq!(journal.lookup_reserved(first), Ok(key(41)));
    rejected_registration(&mut journal, key(42), Error::WriterReplay);
    rejected_registration(&mut journal, key(44), Error::WriterReplay);
    rejected_registration(
        &mut journal,
        Key {
            kind: Kind::Submission,
            ..key(44)
        },
        Error::WriterReplay,
    );
    rejected_reference(&mut journal, last);
    let next = journal.register_writer(key(45)).unwrap();
    assert_eq!(next.slot, last.slot);
    assert_eq!(next.key, key(45));
    assert_eq!(journal.registration_watermark(), 45);
    audit(&journal);
}

#[test]
fn dropped_reference_retains_capacity_and_full_rejection_keeps_watermark() {
    let mut journal = Journal::new(7, 17, 1).unwrap();
    {
        let _reference = journal.register_writer(key(41)).unwrap();
    }
    audit(&journal);
    assert_eq!(journal.reserved_writer_count(), 1);
    assert_eq!(journal.remaining_writer_slots(), 0);
    rejected_registration(&mut journal, key(44), Error::WriterCapacity);
    assert_eq!(journal.registration_watermark(), 41);
    journal
        .abort_reserved(Reference {
            slot: 0,
            key: key(41),
        })
        .unwrap();
    assert_eq!(journal.remaining_writer_slots(), 1);
    let next = journal.register_writer(key(47)).unwrap();
    assert_eq!(next.key, key(47));
    assert_eq!(journal.remaining_writer_slots(), 0);
    audit(&journal);
}

#[test]
fn full_key_and_slot_checks_reject_replay_and_every_substitution() {
    let mut journal = Journal::new(7, 2, 3).unwrap();
    let old = journal.register_writer(key(41)).unwrap();
    let unrelated = journal.register_writer(key(44)).unwrap();
    journal.abort_reserved(old).unwrap();
    let current = journal.register_writer(key(47)).unwrap();
    assert_eq!(current.slot, old.slot);
    rejected_reference(&mut journal, old);
    for wrong_key in [
        Key {
            context_generation: 8,
            ..current.key
        },
        Key {
            local: 46,
            ..current.key
        },
        Key {
            local: 0,
            ..current.key
        },
        Key {
            local: u64::MAX,
            ..current.key
        },
        Key {
            kind: Kind::Submission,
            ..current.key
        },
    ] {
        rejected_reference(
            &mut journal,
            Reference {
                key: wrong_key,
                ..current
            },
        );
    }
    for slot in [unrelated.slot, 2, 3, usize::MAX] {
        rejected_reference(&mut journal, Reference { slot, ..current });
    }
    rejected_registration(
        &mut journal,
        Key {
            context_generation: 8,
            ..key(49)
        },
        Error::ForeignContext,
    );
    assert_eq!(journal.lookup_reserved(current), Ok(key(47)));
    assert_eq!(journal.lookup_reserved(unrelated), Ok(key(44)));
    audit(&journal);
}

#[test]
fn checked_add_allocator_edges_accept_max_minus_one_and_reject_max() {
    let mut journal = Journal::new(u64::MAX - 1, 1, 3).unwrap();
    let last_key = Key {
        context_generation: u64::MAX - 1,
        local: u64::MAX - 1,
        kind: Kind::Submission,
    };
    for local in [0, u64::MAX] {
        rejected_registration(
            &mut journal,
            Key { local, ..last_key },
            Error::InvalidWriterId,
        );
    }
    let last = journal.register_writer(last_key).unwrap();
    assert_eq!(last.key, last_key);
    assert_eq!(journal.registration_watermark(), u64::MAX - 1);
    assert_eq!(journal.lookup_reserved(last), Ok(last_key));
    journal.abort_reserved(last).unwrap();
    assert_eq!(journal.registration_watermark(), u64::MAX - 1);
    for local in [1, u64::MAX - 2, u64::MAX - 1] {
        rejected_registration(&mut journal, Key { local, ..last_key }, Error::WriterReplay);
    }
    for local in [0, u64::MAX] {
        rejected_registration(
            &mut journal,
            Key { local, ..last_key },
            Error::InvalidWriterId,
        );
    }
}

#[test]
fn indexed_work_is_constant_and_storage_does_not_grow() {
    for (a, w) in [(1, 1), (65_537, 17), (1_048_576, 65_537)] {
        for occupied in [0, w / 2, w - 1] {
            let mut journal = Journal::new(7, a, w).unwrap();
            let original_storage = storage(&journal);
            for local in 1..=occupied {
                let _ = journal.register_writer(key(local as u64)).unwrap();
            }
            for step in 1..=32 {
                let local = occupied as u64 + step;
                journal.indexed_accesses.set(0);
                let reference = journal.register_writer(key(local)).unwrap();
                assert_eq!(journal.indexed_accesses.get(), 4);
                journal.indexed_accesses.set(0);
                assert_eq!(journal.lookup_reserved(reference), Ok(key(local)));
                assert_eq!(journal.indexed_accesses.get(), 1);
                journal.indexed_accesses.set(0);
                journal.abort_reserved(reference).unwrap();
                assert_eq!(journal.indexed_accesses.get(), 3);
                assert_eq!(storage(&journal), original_storage);
                audit(&journal);
            }
        }
    }
}

#[test]
fn rejection_work_is_bounded_in_full_journals() {
    for w in [1, 17, 65_537] {
        let mut journal = Journal::new(7, 1, w).unwrap();
        for local in 1..=w {
            let _ = journal.register_writer(key(local as u64)).unwrap();
        }
        for (writer, error, accesses) in [
            (key(0), Error::InvalidWriterId, 0),
            (key(u64::MAX), Error::InvalidWriterId, 0),
            (key(w as u64), Error::WriterReplay, 0),
            (key(w as u64 + 1), Error::WriterCapacity, 1),
            (
                Key {
                    context_generation: 8,
                    ..key(w as u64 + 1)
                },
                Error::ForeignContext,
                0,
            ),
        ] {
            journal.indexed_accesses.set(0);
            rejected_registration(&mut journal, writer, error);
            assert_eq!(journal.indexed_accesses.get(), accesses);
        }
        let invalid = Reference {
            slot: 0,
            key: key(w as u64 + 1),
        };
        let before = snapshot(&journal);
        journal.indexed_accesses.set(0);
        assert_eq!(
            journal.lookup_reserved(invalid),
            Err(Error::InvalidReference)
        );
        assert_eq!(journal.indexed_accesses.get(), 1);
        assert_eq!(snapshot(&journal), before);
        journal.indexed_accesses.set(0);
        assert_eq!(
            journal.abort_reserved(invalid),
            Err(Error::InvalidReference)
        );
        assert_eq!(journal.indexed_accesses.get(), 1);
        assert_eq!(snapshot(&journal), before);
        audit(&journal);
    }
}

#[test]
fn internal_slot_and_capacity_invariants_reject_before_mutation() {
    for invalid in [0, 2, usize::MAX] {
        let mut journal = Journal::new(7, 1, 2).unwrap();
        let first = journal.register_writer(key(41)).unwrap();
        assert_eq!(first.slot, 0);
        *journal.free.last_mut().unwrap() = invalid;
        let before = snapshot(&journal);
        journal.indexed_accesses.set(0);
        assert_eq!(journal.register_writer(key(44)), Err(Error::InvalidState));
        assert_eq!(journal.indexed_accesses.get(), 2);
        assert_eq!(snapshot(&journal), before);
        assert_eq!(journal.lookup_reserved(first), Ok(key(41)));
    }
    // These malformed private states must reject without attempting a growing push.
    let mut full_free = Vec::with_capacity(3);
    full_free.extend([1, 1]);
    assert!(full_free.capacity() > full_free.len());
    for (w, free) in [(1, Vec::new()), (2, full_free)] {
        let mut journal = Journal::new(7, 1, w).unwrap();
        let first = journal.register_writer(key(41)).unwrap();
        journal.free = free;
        let before = snapshot(&journal);
        journal.indexed_accesses.set(0);
        assert_eq!(journal.abort_reserved(first), Err(Error::InvalidState));
        assert_eq!(journal.indexed_accesses.get(), 1);
        assert_eq!(snapshot(&journal), before);
        assert_eq!(journal.lookup_reserved(first), Ok(key(41)));
    }
}

#[test]
fn constructor_initializes_all_arena_contents_and_free_stack_order() {
    for (a, w) in [(1, 3), (7, 2), (2, 7)] {
        let journal = Journal::new(7, a, w).unwrap();
        assert_eq!(journal.writers, vec![None; w]);
        assert_eq!(journal.free, (0..w).rev().collect::<Vec<_>>());
        assert_eq!(journal.allocations, vec![None; a]);
        assert_eq!(journal.allocation_free, (0..a).rev().collect::<Vec<_>>());
        assert_eq!(journal.members, vec![None; a]);
        assert_eq!(journal.member_free, (0..a).rev().collect::<Vec<_>>());
        assert_eq!(journal.scratch, vec![None; a]);
        audit(&journal);
    }
}

#[test]
fn registration_count_guards_preserve_fresh_slot_and_storage() {
    for capacity in [1, 3, 4097] {
        for invalid_count in [capacity, usize::MAX] {
            let mut journal = Journal::new(7, 2, capacity).unwrap();
            journal.reserved_count = invalid_count;
            let before = snapshot(&journal);
            journal.indexed_accesses.set(0);
            assert_eq!(journal.register_writer(key(41)), Err(Error::InvalidState));
            assert_eq!(journal.indexed_accesses.get(), 2);
            assert_eq!(snapshot(&journal), before);

            // The failed preflight consumes neither a slot nor registration history.
            journal.reserved_count = 0;
            let reference = journal.register_writer(key(41)).unwrap();
            assert_eq!(
                reference,
                Reference {
                    slot: 0,
                    key: key(41)
                }
            );
            assert_eq!(storage(&journal), before.storage);
            audit(&journal);
        }
    }
}

#[test]
fn abort_count_underflow_preserves_genuine_reference_and_storage() {
    let mut journal = Journal::new(7, 2, 3).unwrap();
    let first = journal.register_writer(key(41)).unwrap();
    let second = journal.register_writer(key(44)).unwrap();
    assert!(journal.free.len() < journal.writer_capacity);
    assert!(journal.free.len() < journal.free.capacity());
    journal.reserved_count = 0;
    let before = snapshot(&journal);
    journal.indexed_accesses.set(0);
    assert_eq!(journal.abort_reserved(first), Err(Error::InvalidState));
    assert_eq!(journal.indexed_accesses.get(), 1);
    assert_eq!(snapshot(&journal), before);
    assert_eq!(journal.lookup_reserved(first), Ok(key(41)));
    assert_eq!(journal.lookup_reserved(second), Ok(key(44)));

    journal.reserved_count = 2;
    journal.abort_reserved(first).unwrap();
    assert_eq!(journal.reserved_writer_count(), 1);
    assert_eq!(storage(&journal), before.storage);
    audit(&journal);
}

#[test]
fn short_traces_match_independent_active_writer_map() {
    const ACTIONS: usize = 9;
    const DEPTH: u32 = 4;
    for capacity in [1, 3] {
        for trace in 0..ACTIONS.pow(DEPTH) {
            let mut journal = Journal::new(7, 2, capacity).unwrap();
            let mut active = BTreeMap::<u64, Reference>::new();
            let mut references = Vec::new();
            let mut watermark = 0;
            let mut steps = trace;
            for _ in 0..DEPTH {
                let action = steps % ACTIONS;
                steps /= ACTIONS;
                let before = snapshot(&journal);
                if action < 5 {
                    let writer = Key {
                        kind: if action == 1 {
                            Kind::Submission
                        } else {
                            Kind::Synchronous
                        },
                        ..key([1, 4, 2, 0, u64::MAX][action])
                    };
                    let expected_error = if writer.local == 0 || writer.local == u64::MAX {
                        Some(Error::InvalidWriterId)
                    } else if writer.local <= watermark {
                        Some(Error::WriterReplay)
                    } else if active.len() == capacity {
                        Some(Error::WriterCapacity)
                    } else {
                        None
                    };
                    let actual = journal.register_writer(writer);
                    if let Some(error) = expected_error {
                        assert_eq!(actual, Err(error));
                        assert_eq!(snapshot(&journal), before);
                    } else {
                        let reference = actual.unwrap();
                        assert_eq!(reference.key, writer);
                        assert!(reference.slot < capacity);
                        assert!(active.values().all(|live| live.slot != reference.slot));
                        active.insert(writer.local, reference);
                        references.push(reference);
                        watermark = writer.local;
                    }
                } else {
                    let mut reference = references
                        .get(usize::from(action == 6))
                        .copied()
                        .unwrap_or(Reference {
                            slot: usize::MAX,
                            key: key(42),
                        });
                    if action == 8 {
                        reference.key.context_generation += 1;
                    }
                    let valid = active.get(&reference.key.local) == Some(&reference);
                    if action <= 6 {
                        assert_eq!(
                            journal.abort_reserved(reference),
                            if valid {
                                Ok(())
                            } else {
                                Err(Error::InvalidReference)
                            }
                        );
                        if valid {
                            active.remove(&reference.key.local);
                        } else {
                            assert_eq!(snapshot(&journal), before);
                        }
                    } else {
                        assert_eq!(
                            journal.lookup_reserved(reference),
                            if valid {
                                Ok(reference.key)
                            } else {
                                Err(Error::InvalidReference)
                            }
                        );
                        assert_eq!(snapshot(&journal), before);
                    }
                }
                assert_eq!(journal.registration_watermark(), watermark);
                assert_eq!(journal.reserved_writer_count(), active.len());
                assert_eq!(storage(&journal), before.storage);
                for (slot, actual) in journal.writers.iter().enumerate() {
                    let expected = active
                        .values()
                        .find(|reference| reference.slot == slot)
                        .map(|reference| reference.key);
                    assert_eq!(*actual, expected.map(WriterEntryV1::Reserved));
                }
                audit(&journal);
            }
        }
    }
}

#[test]
fn runtime_identity_and_operation_routing_contract_is_explicit() {
    let context = include_str!("../../../fe2o3-runtime/src/context.rs");
    assert!(context.contains("generation.checked_add(1)"));
    let allocator = context
        .split("fn next_id(&mut self)")
        .nth(1)
        .unwrap()
        .split("fn mark_stream_quiescent")
        .next()
        .unwrap();
    assert!(allocator.find(".checked_add(1)").unwrap() < allocator.find("Ok(identity)").unwrap());
    let source = include_str!("../context_version_journal.rs");
    let operations = source
        .split("pub fn register_writer(")
        .nth(1)
        .unwrap()
        .split("pub fn enroll_allocation(")
        .next()
        .unwrap();
    let helpers = source.split("fn count_indexed_access(").nth(1).unwrap();
    let shared = include_str!("writer_lifecycle_bodies.rs");
    let reserved = include_str!("begin_bodies.rs")
        .split("macro_rules! begin_reserved_body")
        .nth(1)
        .unwrap()
        .split("macro_rules! begin_canonical_body")
        .next()
        .unwrap();
    let reserved_adapter = include_str!("begin.rs")
        .split("pub(super) fn begin_reserved_exec_v1(")
        .nth(1)
        .unwrap()
        .split("#[inline]")
        .next()
        .unwrap();
    for forbidden in [
        "try_reserve",
        ".resize(",
        ".extend(",
        ".iter(",
        ".iter_mut(",
        ".into_iter(",
        ".for_each(",
        ".clone(",
        "for ",
        "while ",
        "loop",
    ] {
        assert!(
            [operations, helpers, shared, reserved, reserved_adapter]
                .iter()
                .all(|part| !part.contains(forbidden)),
            "operation contains {forbidden}"
        );
    }
    let routed_operations = operations;
    for direct_access in [
        "self.writers",
        "self.free[",
        "self.free.get",
        "self.free.last",
        "self.free.pop",
        "self.free.push",
    ] {
        assert!(
            !routed_operations.contains(direct_access),
            "operation bypasses indexed helper with {direct_access}"
        );
    }
    assert!(operations.contains("writer_register_body!"));
    assert!(operations.contains("writer_reserved_lookup_body!"));
    assert!(operations.contains("writer_abort_body!"));
    assert!(shared.contains("$count($journal);"));
    assert!(reserved.contains("begin_indexed_access_v1($journal);"));
    assert!(reserved_adapter.contains("begin_reserved_body!(journal, writer)"));
}

#[path = "membership_tests.rs"]
mod membership;

#[path = "settlement_tests.rs"]
mod settlement;

#[path = "allocation_lifecycle_tests.rs"]
mod allocation_lifecycle;
