use super::*;
use crate::context_version_journal::settlement_commit::shared_settlement_commit_v1 as commit;
use crate::context_version_journal::settlement_scratch::shared_settlement_scratch_stage_v1 as stage;

fn assert_commit(
    j: &mut Journal,
    writer: Reference,
    head: Option<usize>,
    slots: &[usize],
    success: bool,
) {
    // Pointer stability is tested with spare capacity, not assumed by the raw logical contract.
    j.free.reserve(1);
    j.member_free.reserve(slots.len());
    let mut expected = snapshot(j);
    let plans: Vec<_> = slots
        .iter()
        .map(|&slot| (slot, j.members[slot].unwrap()))
        .collect();
    for (slot, member) in plans {
        let allocation = expected.allocations[member.allocation.slot]
            .as_mut()
            .unwrap();
        if success {
            allocation.content_lineage = member.attempt_epoch;
        }
        allocation.pending_member = None;
        expected.members[slot] = None;
        expected.member_free.push(slot);
    }
    expected.writers[writer.slot] = None;
    expected.free.push(writer.slot);
    stage(j, head, slots.len());
    j.indexed_accesses.set(0);
    commit(j, writer, slots.len(), success);
    assert_eq!(j.indexed_accesses.get(), 4 * slots.len() + 2);
    assert_eq!(snapshot(j), expected);
}

#[test]
fn raw_commit_preserves_exact_normal_settlement_and_storage() {
    for count in [0, 1, 3, 8] {
        for success in [false, true] {
            let (mut j, writer, _) = pending(count);
            let slots = chain(&j, writer);
            assert_commit(&mut j, writer, slots.first().copied(), &slots, success);
            audit(&j);
        }
    }
}

#[test]
fn aliased_allocations_are_last_write_wins_only_on_success() {
    for success in [false, true] {
        let (mut j, writer, _) = pending(2);
        let slots = chain(&j, writer);
        let target = j.members[slots[0]].unwrap().allocation;
        for (index, &slot) in slots.iter().enumerate() {
            let member = j.members[slot].as_mut().unwrap();
            member.allocation = target;
            member.prior_lineage = 701 + index as u64;
            member.attempt_epoch = [19, 31][index];
        }
        let allocation = j.allocations[target.slot].as_mut().unwrap();
        allocation.content_lineage = 41;
        allocation.attempt_epoch = 99;
        assert_commit(&mut j, writer, Some(slots[0]), &slots, success);
        let allocation = j.allocations[target.slot].unwrap();
        assert_eq!(allocation.content_lineage, if success { 31 } else { 41 });
        assert_eq!(allocation.attempt_epoch, 99);
    }
}

#[test]
fn raw_commit_returns_each_repeated_member_occurrence_in_order() {
    for slots in [vec![2, 2, 2, 2], vec![2, 5, 2, 5], vec![2, 5]] {
        for success in [false, true] {
            let (mut j, writer, _) = pending(2);
            let old = chain(&j, writer);
            let first = j.members[old[0]].unwrap();
            let second = j.members[old[1]].unwrap();
            j.members.fill(None);
            j.members[2] = Some(MemberEntryV1 {
                next: Some(slots[1]),
                ..first
            });
            j.members[5] = Some(MemberEntryV1 {
                next: Some(2),
                ..second
            });
            assert_commit(&mut j, writer, Some(2), &slots, success);
        }
    }
}

#[test]
fn raw_commit_frames_malformed_metadata_free_prefixes_and_dirty_tails() {
    for success in [false, true] {
        let (mut j, writer, roster) = pending(3);
        let slots = chain(&j, writer);
        let poison = Some(BeginMemberPlanV1 {
            member_slot: usize::MAX,
            allocation: roster[7].allocation,
            prior_lineage: u64::MAX,
            attempt_epoch: 0,
        });
        j.scratch[3] = poison;
        j.scratch[7] = poison;
        let unused = j.member_free.pop().unwrap();
        j.members[unused] = Some(MemberEntryV1 {
            writer: Reference {
                slot: usize::MAX,
                key: key(u64::MAX),
            },
            allocation: roster[7].allocation,
            prior_lineage: u64::MAX,
            attempt_epoch: 0,
            next: Some(unused),
        });
        j.allocations[roster[7].allocation.slot]
            .as_mut()
            .unwrap()
            .pending_member = Some(slots[0]);
        j.free = vec![usize::MAX, writer.slot, writer.slot];
        j.member_free = vec![slots[0], slots[0], usize::MAX];
        j.allocation_free = vec![usize::MAX];
        j.context_generation = u64::MAX;
        j.registration_watermark = u64::MAX;
        j.reserved_count = usize::MAX;
        j.allocation_capacity = 0;
        j.writer_capacity = 0;
        assert_commit(&mut j, writer, Some(slots[0]), &slots, success);
    }
}

#[test]
fn raw_commit_needs_only_an_in_bounds_writer_slot() {
    for occupied in [false, true] {
        for success in [false, true] {
            let (mut j, writer, _) = pending(1);
            let slots = chain(&j, writer);
            j.writers[writer.slot] = if occupied {
                Some(WriterEntryV1::Reserved(key(u64::MAX)))
            } else {
                None
            };
            assert_commit(&mut j, writer, Some(slots[0]), &slots, success);
        }
    }
}

#[test]
fn zero_count_commit_ignores_the_head_and_unrelated_arenas() {
    for head in [None, Some(usize::MAX)] {
        for success in [false, true] {
            for dirty_tail in [false, true] {
                let (mut j, writer, roster) = pending(0);
                j.members.clear();
                j.allocations.clear();
                j.member_free = vec![usize::MAX];
                if dirty_tail {
                    j.scratch[0] = Some(BeginMemberPlanV1 {
                        member_slot: usize::MAX,
                        allocation: roster[0].allocation,
                        prior_lineage: u64::MAX,
                        attempt_epoch: 0,
                    });
                } else {
                    j.scratch.clear();
                }
                assert_commit(&mut j, writer, head, &[], success);
            }
        }
    }
}

#[test]
fn commit_wrapper_shares_actual_mutations_and_empty_annotations() {
    let adapter = include_str!("settlement_commit.rs");
    let body = include_str!("settlement_commit_body.rs");
    assert!(adapter.contains("include!(\"settlement_commit_body.rs\")"));
    let compact = adapter
        .split_whitespace()
        .collect::<alloc::string::String>();
    assert!(compact.contains(
        "settlement_commit_body!(settlement_commit_rust_expr,journal,writer,count,success,index,[])"
    ));
    assert_eq!(body.matches("while $index < $count").count(), 1);
    assert_eq!(body.matches("$index += 1;").count(), 1);
    assert_eq!(
        body.matches("settlement_commit_access_v1($journal)")
            .count(),
        6
    );
    assert_eq!(body.matches(".push(").count(), 2);
    let mut previous = 0;
    for operation in [
        "$journal.scratch[$index]",
        ".take()",
        ".expect(\"complete settlement plan\")",
        "$journal.allocations[plan.allocation.slot]",
        ".as_mut()",
        ".expect(\"validated exact retained allocation\")",
        "if $success",
        "allocation.content_lineage = plan.attempt_epoch",
        "allocation.pending_member = None",
        "$journal.members[plan.member_slot] = None",
        "$journal.member_free.push(plan.member_slot)",
        "$journal.writers[$writer.slot] = None",
        "$journal.free.push($writer.slot)",
    ] {
        let at = body.find(operation).unwrap();
        assert!(at >= previous, "out-of-order commit: {operation}");
        previous = at;
    }
    for forbidden in [
        "reserve",
        "resize",
        "Vec::",
        "Box::",
        "unsafe",
        ".iter(",
        "loop {",
        "AllocationEntryV1 {",
    ] {
        assert!(!body.contains(forbidden));
        assert!(!adapter.contains(forbidden));
    }
}
