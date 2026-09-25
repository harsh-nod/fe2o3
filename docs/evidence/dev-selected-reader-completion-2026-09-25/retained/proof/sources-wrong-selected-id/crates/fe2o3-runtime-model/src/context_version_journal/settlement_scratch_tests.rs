use super::*;
use crate::context_version_journal::settlement_scratch::{
    shared_settlement_scratch_scan_v1 as scan, shared_settlement_scratch_stage_v1 as stage,
};

fn poison() -> BeginMemberPlanV1 {
    BeginMemberPlanV1 {
        member_slot: usize::MAX,
        allocation: ContextAllocationReferenceV1 {
            slot: usize::MAX,
            key: ContextAllocationKeyV1 {
                context_generation: u64::MAX,
                local: u64::MAX,
            },
        },
        prior_lineage: u64::MAX,
        attempt_epoch: 0,
    }
}

fn assert_stage(journal: &mut Journal, initial: Option<usize>, slots: &[usize]) {
    let mut expected = snapshot(journal);
    for (index, &slot) in slots.iter().enumerate() {
        let member = journal.members[slot].unwrap();
        expected.scratch[index] = Some(BeginMemberPlanV1 {
            member_slot: slot,
            allocation: member.allocation,
            prior_lineage: member.prior_lineage,
            attempt_epoch: member.attempt_epoch,
        });
    }
    journal.indexed_accesses.set(0);
    stage(journal, initial, slots.len());
    assert_eq!(journal.indexed_accesses.get(), 2 * slots.len());
    assert_eq!(snapshot(journal), expected);
}

#[test]
fn staging_preserves_nonzero_lineage_in_every_intermediate_plan() {
    for count in [0, 1, 3, 8] {
        let (mut j, mut writer, roster) = pending(count);
        for (local, success) in [(11_000, true), (11_001, false)] {
            settle(&mut j, writer, writer, success).unwrap();
            writer = j.register_writer(key(local)).unwrap();
            j.begin_write(writer, &roster[..count]).unwrap();
        }
        let slots = chain(&j, writer);
        for &slot in &slots {
            let member = j.members[slot].unwrap();
            assert_eq!(member.prior_lineage, 1);
            assert_eq!(member.attempt_epoch, 3);
        }
        assert_stage(&mut j, slots.first().copied(), &slots);
    }
}

#[test]
fn staging_uses_linked_slots_and_complete_member_fields() {
    let (mut j, writer, _) = pending(3);
    let old = chain(&j, writer);
    let slots = [6, 1, 4];
    let members: [MemberEntryV1; 3] = core::array::from_fn(|index| j.members[old[index]].unwrap());
    j.members.fill(None);
    for (index, (&slot, mut member)) in slots.iter().zip(members).enumerate() {
        member.next = slots.get(index + 1).copied();
        member.prior_lineage = 10 + index as u64;
        member.attempt_epoch = 20 + index as u64;
        member.allocation.key.context_generation = 100 + index as u64;
        member.allocation.key.local = 200 + index as u64;
        j.members[slot] = Some(member);
    }
    assert_stage(&mut j, Some(slots[0]), &slots);
}

#[test]
fn staging_preserves_dirty_tails_and_unrelated_malformed_contents() {
    for count in [1, 3] {
        let (mut j, writer, roster) = pending(count);
        let slots = chain(&j, writer);
        j.scratch[count] = Some(poison());
        j.scratch[7] = Some(poison());
        let unused = j.member_free.pop().unwrap();
        j.members[unused] = Some(MemberEntryV1 {
            writer: Reference {
                slot: usize::MAX,
                key: key(u64::MAX),
            },
            allocation: poison().allocation,
            prior_lineage: u64::MAX,
            attempt_epoch: 0,
            next: Some(unused),
        });
        j.allocations[roster[7].allocation.slot]
            .as_mut()
            .unwrap()
            .pending_member = Some(slots[0]);
        j.free = vec![usize::MAX, writer.slot, writer.slot];
        j.allocation_free = vec![usize::MAX];
        j.member_free = vec![slots[0], slots[0], usize::MAX];
        j.context_generation = u64::MAX;
        j.registration_watermark = u64::MAX;
        j.reserved_count = usize::MAX;
        j.allocation_capacity = 0;
        j.writer_capacity = 0;
        assert_stage(&mut j, Some(slots[0]), &slots);
    }
}

#[test]
fn zero_count_staging_ignores_the_head_and_all_unrelated_shapes() {
    for head in [None, Some(usize::MAX)] {
        for dirty_tail in [false, true] {
            let (mut j, _, _) = pending(0);
            j.members.clear();
            j.allocations.clear();
            j.writers.clear();
            j.free = vec![usize::MAX];
            j.member_free = vec![usize::MAX];
            if dirty_tail {
                j.scratch[0] = Some(poison());
            } else {
                j.scratch.clear();
            }
            assert_stage(&mut j, head, &[]);
        }
    }
}

#[test]
fn raw_staging_accepts_bounded_repeated_prefixes_and_nonterminal_links() {
    for slots in [vec![2, 2, 2, 2], vec![2, 5, 2, 5], vec![2, 5]] {
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
        assert_stage(&mut j, Some(2), &slots);
    }
}

#[test]
fn scratch_scan_stops_at_the_first_dirty_prefix_cell_and_ignores_tails() {
    for count in [0, 1, 3, 8] {
        for dirty in core::iter::once(None).chain((0..count).map(Some)) {
            let (mut j, _, _) = pending(count);
            if count < j.scratch.len() {
                j.scratch[count] = Some(poison());
            }
            if let Some(index) = dirty {
                j.scratch[index] = Some(poison());
                j.scratch[count - 1] = Some(poison());
            }
            let before = snapshot(&j);
            j.indexed_accesses.set(0);
            assert_eq!(
                scan(&j, count),
                if dirty.is_some() {
                    Err(Error::InvalidState)
                } else {
                    Ok(())
                }
            );
            assert_eq!(
                j.indexed_accesses.get(),
                dirty.map_or(count, |index| index + 1)
            );
            assert_eq!(snapshot(&j), before);
        }
    }
    let (mut j, writer, _) = pending(3);
    j.scratch.truncate(2);
    let before = snapshot(&j);
    j.indexed_accesses.set(0);
    assert_eq!(
        j.preflight_settlement(writer, writer),
        Err(Error::InvalidState)
    );
    assert_eq!(j.indexed_accesses.get(), 7);
    assert_eq!(snapshot(&j), before);
}

#[test]
fn scratch_wrappers_share_only_bounded_allocation_free_bodies() {
    let adapter = include_str!("settlement_scratch.rs");
    let bodies = include_str!("settlement_scratch_bodies.rs");
    assert!(adapter.contains("include!(\"settlement_scratch_bodies.rs\")"));
    assert_eq!(bodies.matches("while $index < $count").count(), 2);
    assert_eq!(bodies.matches("$index += 1;").count(), 2);
    assert_eq!(bodies.matches("$journal.scratch[$index] =").count(), 1);
    assert!(bodies.contains("prior_lineage: member.prior_lineage"));
    assert!(bodies.contains("attempt_epoch: member.attempt_epoch"));
    for forbidden in [
        ".push(", "reserve", "resize", "Vec::", "Box::", "unsafe", ".iter(", "loop {",
    ] {
        assert!(!bodies.contains(forbidden));
        assert!(!adapter.contains(forbidden));
    }
    let compact = adapter
        .split_whitespace()
        .collect::<alloc::string::String>();
    assert!(compact.contains(
        "settlement_scratch_scan_body!(settlement_scratch_rust_expr,journal,count,index,[])"
    ));
    assert!(compact.contains("settlement_scratch_stage_body!(settlement_scratch_rust_expr,journal,initial,count,head,index,[])"));
}
