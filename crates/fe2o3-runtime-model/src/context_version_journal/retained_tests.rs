use super::*;
use crate::context_version_journal::retained as runtime_retained;

fn retained_fixture(count: usize, unknown: bool) -> (Journal, Reference, Vec<usize>) {
    let (mut journal, writer, _) = pending(count);
    let slots = chain(&journal, writer);
    if unknown {
        journal.mark_unknown(writer).unwrap();
    }
    (journal, writer, slots)
}

fn rejects_unchanged(journal: &mut Journal, writer: Reference, error: Error, accesses: usize) {
    let before = snapshot(journal);
    journal.indexed_accesses.set(0);
    assert_eq!(journal.mark_unknown(writer), Err(error));
    assert_eq!(journal.indexed_accesses.get(), accesses);
    assert_eq!(snapshot(journal), before);
}

#[test]
fn matching_foreign_keys_still_reject_the_journal_context() {
    for unknown in [false, true] {
        let (mut j, mut writer, _) = retained_fixture(1, unknown);
        writer.key.context_generation += 1;
        match j.writers[writer.slot].as_mut().unwrap() {
            WriterEntryV1::Pending { key, .. } | WriterEntryV1::Unknown { key, .. } => {
                *key = writer.key;
            }
            _ => unreachable!(),
        }
        rejects_unchanged(&mut j, writer, Error::InvalidReference, 1);

        let (mut j, writer, slots) = retained_fixture(1, unknown);
        let member = j.members[slots[0]].as_mut().unwrap();
        member.allocation.key.context_generation += 1;
        j.allocations[member.allocation.slot].as_mut().unwrap().key = member.allocation.key;
        rejects_unchanged(&mut j, writer, Error::InvalidState, 3);
    }
}

#[test]
fn equal_keys_with_distinct_valid_backlinks_reject_before_allocation_read() {
    for unknown in [false, true] {
        let (mut j, writer, slots) = retained_fixture(2, unknown);
        let first = j.members[slots[0]].unwrap();
        let second = j.members[slots[1]].as_mut().unwrap();
        assert_ne!(first.allocation.slot, second.allocation.slot);
        second.allocation.key = first.allocation.key;
        j.allocations[second.allocation.slot].as_mut().unwrap().key = first.allocation.key;
        assert_eq!(
            j.allocations[second.allocation.slot]
                .unwrap()
                .pending_member,
            Some(slots[1])
        );
        rejects_unchanged(&mut j, writer, Error::InvalidState, 4);
    }
}

#[test]
fn every_writer_coordinate_precedes_the_member_allocation_lookup() {
    for unknown in [false, true] {
        for position in 0..4 {
            for coordinate in 0..4 {
                let (mut j, writer, slots) = retained_fixture(4, unknown);
                let member = j.members[slots[position]].as_mut().unwrap();
                match coordinate {
                    0 => member.writer.slot = usize::MAX,
                    1 => member.writer.key.context_generation += 1,
                    2 => member.writer.key.local += 1,
                    3 => member.writer.key.kind = Kind::Submission,
                    _ => unreachable!(),
                }
                member.allocation.slot = usize::MAX;
                rejects_unchanged(&mut j, writer, Error::InvalidState, 2 * position + 2);
            }
        }
    }
}

#[test]
fn live_arena_boundaries_and_truncation_reject_without_indexing_past_the_end() {
    for unknown in [false, true] {
        for fault in 0..6 {
            let (mut j, mut writer, slots) = retained_fixture(1, unknown);
            let member_slot = slots[0];
            let allocation_slot = j.members[member_slot].unwrap().allocation.slot;
            match fault {
                0 => writer.slot = j.writers.len(),
                1 => match j.writers[writer.slot].as_mut().unwrap() {
                    WriterEntryV1::Pending { head, .. } | WriterEntryV1::Unknown { head, .. } => {
                        *head = Some(j.members.len());
                    }
                    _ => unreachable!(),
                },
                2 => j.members[member_slot].as_mut().unwrap().allocation.slot = j.allocations.len(),
                3 => j.writers.truncate(writer.slot),
                4 => j.members.truncate(member_slot),
                5 => j.allocations.truncate(allocation_slot),
                _ => unreachable!(),
            }
            let (error, accesses) = match fault % 3 {
                0 => (Error::InvalidReference, 1),
                1 => (Error::InvalidState, 2),
                _ => (Error::InvalidState, 3),
            };
            rejects_unchanged(&mut j, writer, error, accesses);
        }
    }
}

#[test]
fn missing_interior_links_reject_without_a_member_array_access() {
    for unknown in [false, true] {
        for missing in 1..4 {
            let (mut j, writer, slots) = retained_fixture(4, unknown);
            j.members[slots[missing - 1]].as_mut().unwrap().next = None;
            rejects_unchanged(&mut j, writer, Error::InvalidState, 1 + 2 * missing);
        }
    }
}

#[test]
fn unknown_ignores_unrelated_corruption_and_repetition_preserves_all_storage() {
    for count in [0, 1, 4] {
        let (mut j, writer, roster) = pending(count);
        let slots = chain(&j, writer);
        let unused = roster[count].allocation.slot;
        j.allocations[unused].as_mut().unwrap().pending_member =
            Some(slots.first().copied().unwrap_or(usize::MAX));
        j.free = vec![usize::MAX, usize::MAX];
        j.allocation_free = vec![usize::MAX];
        j.member_free.clear();
        j.scratch.clear();
        j.writer_capacity = 0;
        j.registration_watermark = u64::MAX;
        j.reserved_count = usize::MAX;
        if count == 0 {
            j.members.clear();
            j.allocations.clear();
        }
        let mut expected = snapshot(&j);
        let (key, head, member_count) = match expected.writers[writer.slot].unwrap() {
            WriterEntryV1::Pending { key, head, count } => (key, head, count),
            _ => unreachable!(),
        };
        expected.writers[writer.slot] = Some(WriterEntryV1::Unknown {
            key,
            head,
            count: member_count,
        });
        for repeated in [false, true] {
            j.indexed_accesses.set(0);
            j.mark_unknown(writer).unwrap();
            assert_eq!(
                j.indexed_accesses.get(),
                2 * count + if repeated { 1 } else { 2 }
            );
            assert_eq!(snapshot(&j), expected);
        }
    }
}

#[test]
fn retained_wrappers_share_bounded_guards_and_the_single_unknown_store() {
    let adapter = include_str!("retained.rs");
    let bodies = include_str!("retained_bodies.rs");
    assert!(adapter.contains("include!(\"retained_bodies.rs\")"));
    assert_eq!(bodies.matches("while ").count(), 1);
    assert!(bodies.contains("while $index < $count"));
    assert!(bodies.contains("$index += 1"));
    assert!(bodies.contains("$reference.slot >= $journal.allocations.len()"));
    assert!(bodies.contains("$writer.slot >= $journal.writers.len()"));
    assert!(bodies.contains("slot >= $journal.members.len()"));
    for forbidden in [
        ".push(", "reserve", "resize", "Vec::", "Box::", "unsafe", ".iter(", "loop {",
    ] {
        assert!(!bodies.contains(forbidden));
        assert!(!adapter.contains(forbidden));
    }
    let unknown = bodies
        .split("macro_rules! retained_unknown_body")
        .nth(1)
        .unwrap();
    let header = unknown
        .find("shared_retained_header_v1($journal, $writer, true)")
        .unwrap();
    let chain = unknown
        .find("shared_retained_chain_v1($journal, $writer, head, count)")
        .unwrap();
    let store = unknown
        .find("$journal.writers[$writer.slot] = Some(WriterEntryV1::Unknown")
        .unwrap();
    assert!(header < chain && chain < store);
    assert_eq!(unknown.matches("$journal.writers[").count(), 1);
    assert!(unknown.contains("if !unknown"));
    for forbidden in [
        "scratch",
        "member_free",
        "allocation_free",
        ".free",
        "store_plan",
    ] {
        assert!(!unknown.contains(forbidden));
    }
    let compact = adapter
        .split_whitespace()
        .collect::<alloc::string::String>();
    assert!(compact.contains("retained_chain_body!(retained_rust_expr,journal,writer,initial,count,head,previous,index,[])"));
    for expansion in [
        "retained_writer_key_body!(left,right)",
        "retained_allocation_less_body!(left,right)",
        "retained_allocation_body!(journal,reference)",
        "retained_header_body!(journal,writer,allow_unknown)",
        "retained_member_body!(journal,writer,head,previous)",
        "retained_unknown_body!(journal,writer)",
    ] {
        assert_eq!(compact.matches(expansion).count(), 1);
    }
}

#[test]
fn raw_empty_chain_does_not_authenticate_a_writer_header() {
    let (mut j, mut writer, _) = pending(0);
    j.writers.clear();
    j.members.clear();
    j.allocations.clear();
    j.allocation_capacity = 0;
    writer.slot = usize::MAX;
    writer.key.context_generation = u64::MAX;
    writer.key.local = 0;
    let before = snapshot(&j);
    j.indexed_accesses.set(0);
    assert_eq!(
        runtime_retained::shared_retained_chain_v1(&j, writer, None, 0),
        Ok(())
    );
    assert_eq!(j.indexed_accesses.get(), 0);
    assert_eq!(snapshot(&j), before);
    assert_eq!(
        runtime_retained::shared_retained_header_v1(&j, writer, true),
        Err(Error::InvalidReference)
    );
}

#[test]
fn raw_header_returns_malformed_payload_before_chain_validation() {
    for unknown in [false, true] {
        for (head, count) in [(None, usize::MAX), (Some(usize::MAX), 0)] {
            let (mut j, writer, _) = pending(0);
            j.writers[writer.slot] = Some(if unknown {
                WriterEntryV1::Unknown {
                    key: writer.key,
                    head,
                    count,
                }
            } else {
                WriterEntryV1::Pending {
                    key: writer.key,
                    head,
                    count,
                }
            });
            let before = snapshot(&j);
            for allow_unknown in [false, true] {
                assert_eq!(
                    runtime_retained::shared_retained_header_v1(&j, writer, allow_unknown),
                    if unknown && !allow_unknown {
                        Err(Error::InvalidReference)
                    } else {
                        Ok((head, count, unknown))
                    }
                );
            }
            assert_eq!(
                runtime_retained::shared_retained_chain_v1(&j, writer, head, count),
                Err(Error::InvalidState)
            );
            assert_eq!(snapshot(&j), before);
        }
    }
}

#[test]
fn raw_nonempty_chain_preserves_unrelated_malformed_contents() {
    let (mut j, mut writer, slots) = retained_fixture(1, false);
    writer.slot = usize::MAX;
    writer.key.context_generation = u64::MAX;
    writer.key.local = 0;
    j.members[slots[0]].as_mut().unwrap().writer = writer;
    let allocation_slot = j.members[slots[0]].unwrap().allocation.slot;
    let allocation = j.allocations[allocation_slot].as_mut().unwrap();
    allocation.device.context_generation = 0;
    allocation.device.local = 0;
    allocation.byte_extent = 0;
    j.writers.clear();
    j.free = vec![usize::MAX, usize::MAX];
    j.member_free.clear();
    j.allocation_free = vec![allocation_slot, allocation_slot];
    j.scratch.clear();
    j.writer_capacity = 0;
    j.reserved_count = usize::MAX;
    let before = snapshot(&j);
    j.indexed_accesses.set(0);
    assert_eq!(
        runtime_retained::shared_retained_chain_v1(&j, writer, Some(slots[0]), 1),
        Ok(())
    );
    assert_eq!(j.indexed_accesses.get(), 2);
    assert_eq!(snapshot(&j), before);
}

#[test]
fn raw_chain_rejects_capacity_overflow_and_nonterminal_tail_without_mutation() {
    for tail in [false, true] {
        let (mut j, writer, slots) = retained_fixture(1, false);
        if tail {
            j.members[slots[0]].as_mut().unwrap().next = Some(usize::MAX);
        } else {
            j.allocation_capacity = 0;
        }
        let before = snapshot(&j);
        j.indexed_accesses.set(0);
        assert_eq!(
            runtime_retained::shared_retained_chain_v1(&j, writer, Some(slots[0]), 1),
            Err(Error::InvalidState)
        );
        assert_eq!(j.indexed_accesses.get(), if tail { 2 } else { 0 });
        assert_eq!(snapshot(&j), before);
    }
}

#[test]
fn production_retained_proof_uses_actual_declarations_and_shared_bodies() {
    let root = include_str!("../../verus/context_journal_retained_execution_v1.rs");
    let proof = include_str!("../../verus/context_journal_retained_bodies_v1.rs");
    assert!(root.contains("include!(\"../src/context_version_journal/declarations.rs\")"));
    assert!(root.contains("include!(\"context_journal_content_views_v1.rs\")"));
    assert!(root.contains("include!(\"context_journal_retained_decisions_v1.rs\")"));
    assert!(root.contains("include!(\"context_journal_retained_bodies_v1.rs\")"));
    for expansion in [
        "retained_allocation_body!(journal, reference)",
        "retained_header_body!(journal, writer, allow_unknown)",
        "retained_member_body!(journal, writer, head, previous)",
        "retained_chain_body!(verus_exec_expr, journal, writer, initial, count, head, previous, index, [",
    ] {
        assert_eq!(proof.matches(expansion).count(), 1);
    }
    assert!(!proof.contains("assume("));
    assert!(!proof.contains("external_body"));
}
