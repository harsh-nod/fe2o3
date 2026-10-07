use super::*;

#[test]
fn unknown_changes_only_its_writer_with_unrelated_malformed_live_writers() {
    let (mut j, writer, _) = pending(2);
    assert!(writer.slot >= 3);
    j.writers[0] = Some(WriterEntryV1::Reserved(Key {
        context_generation: 0,
        local: 0,
        kind: Kind::Submission,
    }));
    j.writers[1] = Some(WriterEntryV1::Pending {
        key: key(u64::MAX),
        head: Some(usize::MAX),
        count: usize::MAX,
    });
    j.writers[2] = Some(WriterEntryV1::Unknown {
        key: key(0),
        head: None,
        count: usize::MAX,
    });
    let mut expected = snapshot(&j);
    let (key, head, count) = match expected.writers[writer.slot].unwrap() {
        WriterEntryV1::Pending { key, head, count } => (key, head, count),
        _ => unreachable!(),
    };
    expected.writers[writer.slot] = Some(WriterEntryV1::Unknown { key, head, count });
    j.indexed_accesses.set(0);
    assert_eq!(j.mark_unknown(writer), Ok(()));
    assert_eq!(j.indexed_accesses.get(), 6);
    assert_eq!(snapshot(&j), expected);
}

#[test]
fn raw_unknown_accepts_matching_unissuable_ids_and_boundary_epochs() {
    for value in [0, u64::MAX] {
        for kind in [Kind::Synchronous, Kind::Submission] {
            let (mut j, mut writer, _) = pending(1);
            let slots = chain(&j, writer);
            j.context_generation = value;
            writer.key = Key {
                context_generation: value,
                local: value,
                kind,
            };
            let head = Some(slots[0]);
            j.writers[writer.slot] = Some(WriterEntryV1::Pending {
                key: writer.key,
                head,
                count: 1,
            });
            let member = j.members[slots[0]].as_mut().unwrap();
            member.writer = writer;
            member.allocation.key = ContextAllocationKeyV1 {
                context_generation: value,
                local: value,
            };
            member.prior_lineage = u64::MAX - 1;
            member.attempt_epoch = u64::MAX;
            let allocation = j.allocations[member.allocation.slot].as_mut().unwrap();
            allocation.key = member.allocation.key;
            allocation.device = ContextJournalDeviceKeyV1 {
                context_generation: 0,
                local: 0,
            };
            allocation.byte_extent = 0;
            allocation.attempt_epoch = member.attempt_epoch;
            allocation.content_lineage = member.prior_lineage;
            let mut expected = snapshot(&j);
            expected.writers[writer.slot] = Some(WriterEntryV1::Unknown {
                key: writer.key,
                head,
                count: 1,
            });
            for repeated in [false, true] {
                j.indexed_accesses.set(0);
                assert_eq!(j.mark_unknown(writer), Ok(()));
                assert_eq!(j.indexed_accesses.get(), if repeated { 3 } else { 4 });
                assert_eq!(snapshot(&j), expected);
            }
        }
    }
}

#[test]
fn repeated_unknown_revalidates_malformed_header_cardinality_before_member_access() {
    for (head, count) in [(None, 1), (Some(usize::MAX), 0), (Some(0), usize::MAX)] {
        let (mut j, writer, _) = pending(1);
        j.writers[writer.slot] = Some(WriterEntryV1::Unknown {
            key: writer.key,
            head,
            count,
        });
        let before = snapshot(&j);
        j.indexed_accesses.set(0);
        assert_eq!(j.mark_unknown(writer), Err(Error::InvalidState));
        assert_eq!(j.indexed_accesses.get(), 1);
        assert_eq!(snapshot(&j), before);
    }
}

#[test]
fn unknown_proof_compiles_production_declarations_and_the_shared_mutation() {
    let root = include_str!("../../verus/context_journal_unknown_execution_v1.rs");
    let proof = include_str!("../../verus/context_journal_unknown_body_v1.rs");
    assert!(root.contains("include!(\"../src/context_version_journal/declarations.rs\")"));
    assert!(root.contains("include!(\"context_journal_retained_bodies_v1.rs\")"));
    assert!(root.contains("include!(\"context_journal_unknown_decisions_v1.rs\")"));
    assert!(root.contains("include!(\"context_journal_unknown_body_v1.rs\")"));
    assert_eq!(
        proof
            .matches("retained_unknown_body!(journal, writer)")
            .count(),
        1
    );
    assert!(!proof.contains("assume("));
    assert!(!proof.contains("external_body"));
}
