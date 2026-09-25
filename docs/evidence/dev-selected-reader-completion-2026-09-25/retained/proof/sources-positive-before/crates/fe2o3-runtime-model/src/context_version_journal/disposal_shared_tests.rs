use super::*;

#[allow(unused_macros)]
#[macro_use]
mod templates {
    include!("disposal_bodies.rs");
}

macro_rules! disposal_test_expr {
    ($body:expr) => {
        $body
    };
}

fn compare(
    journal: &mut Journal,
    writer: Reference,
    evidence: Reference,
    roster: &[Write],
    validate: (Result<(), Error>, usize),
    execute: (Result<(), Error>, usize),
) {
    let saved = journal.guard_copy_for_test_v1();
    let before = snapshot(journal);
    for baseline in [true, false] {
        journal.reset_access_count_for_test_v1();
        let result = if baseline {
            journal.baseline_validate_unknown_disposal_v1(writer, roster)
        } else {
            journal.validate_unknown_disposal(writer, roster)
        };
        assert_eq!(result, validate.0);
        assert_eq!(journal.guard_accesses_for_test_v1(), validate.1);
        assert_eq!(snapshot(journal), before);
    }
    let evidence = ContextWriterDisposalEvidenceV1 {
        writer: evidence,
        allocations: roster,
    };
    journal.reset_access_count_for_test_v1();
    assert_eq!(
        journal.baseline_dispose_unknown_v1(writer, &evidence),
        execute.0
    );
    assert_eq!(journal.guard_accesses_for_test_v1(), execute.1);
    let frozen = snapshot(journal);
    assert_eq!(storage(journal), before.storage);
    if execute.0.is_ok() {
        journal.restore_disposal_for_test_v1(&saved, writer);
    } else {
        assert_eq!(frozen, before);
        journal.reset_access_count_for_test_v1();
    }
    assert_eq!(snapshot(journal), before);
    assert_eq!(journal.dispose_unknown(writer, &evidence), execute.0);
    assert_eq!(journal.guard_accesses_for_test_v1(), execute.1);
    assert_eq!(snapshot(journal), frozen);
    assert_eq!(storage(journal), before.storage);
}

#[test]
fn disposal_shared_success_preserves_order_storage_and_empty_chain_behavior() {
    for kind in [Kind::Synchronous, Kind::Submission] {
        for count in [0, 1, 3, 8] {
            let (mut journal, writer, roster) = pending_kind(count, kind);
            journal.mark_unknown(writer).unwrap();
            let slots = chain(&journal, writer);
            let old_members = journal.member_free.clone();
            let old_allocations = journal.allocation_free.clone();
            compare(
                &mut journal,
                writer,
                writer,
                &roster[..count],
                (Ok(()), 1 + 3 * count),
                (Ok(()), 4 + 4 * count),
            );
            assert_eq!(&journal.member_free[old_members.len()..], slots);
            assert_eq!(
                &journal.allocation_free[old_allocations.len()..],
                roster[..count]
                    .iter()
                    .map(|entry| entry.allocation.slot)
                    .collect::<Vec<_>>()
            );
            assert_eq!(journal.free.last(), Some(&writer.slot));
            audit(&journal);
        }
    }
}

#[test]
fn disposal_shared_header_and_evidence_precedence_covers_every_writer_coordinate() {
    for phase in 0..3 {
        for coordinate in 0..4 {
            let (mut journal, writer, roster) = reserved_kind(8, 4, Kind::Submission);
            if phase > 0 {
                journal.begin_write(writer, &roster[..2]).unwrap();
            }
            if phase > 1 {
                journal.mark_unknown(writer).unwrap();
            }
            let mut evidence = writer;
            match coordinate {
                0 => evidence.slot = usize::MAX,
                1 => evidence.key.context_generation += 1,
                2 => evidence.key.local += 1,
                _ => evidence.key.kind = Kind::Synchronous,
            }
            let validate = match phase {
                0 => (Err(Error::InvalidReference), 1),
                1 => (Err(Error::InvalidState), 1),
                _ => (Ok(()), 7),
            };
            let error = if phase == 0 {
                Error::InvalidReference
            } else {
                Error::SettlementEvidenceMismatch
            };
            compare(
                &mut journal,
                writer,
                evidence,
                &roster[..2],
                validate,
                (Err(error), 1),
            );
            compare(
                &mut journal,
                evidence,
                writer,
                &roster[..2],
                (Err(Error::InvalidReference), 1),
                (Err(Error::InvalidReference), 1),
            );
        }
    }
}

#[test]
fn disposal_shared_roster_length_precedes_chain_and_descriptor_faults() {
    for length in [0, 2, 4, 8] {
        for corrupt in [false, true] {
            let (mut journal, writer, mut roster) = pending(3);
            journal.mark_unknown(writer).unwrap();
            if corrupt {
                let slots = chain(&journal, writer);
                journal.members[slots[2]].as_mut().unwrap().next = Some(slots[0]);
                roster[0].byte_extent += 1;
            }
            compare(
                &mut journal,
                writer,
                writer,
                &roster[..length],
                (Err(Error::SettlementEvidenceMismatch), 1),
                (Err(Error::SettlementEvidenceMismatch), 2),
            );
        }
    }
}

#[test]
fn disposal_shared_roster_checks_every_descriptor_after_the_complete_chain() {
    for index in 0..3 {
        for coordinate in 0..6 {
            let (mut journal, writer, mut roster) = pending(3);
            journal.mark_unknown(writer).unwrap();
            match coordinate {
                0 => roster[index].allocation.slot = usize::MAX,
                1 => roster[index].allocation.key.context_generation += 1,
                2 => roster[index].allocation.key.local += 1,
                3 => roster[index].device.context_generation += 1,
                4 => roster[index].device.local += 1,
                _ => roster[index].byte_extent += 1,
            }
            compare(
                &mut journal,
                writer,
                writer,
                &roster[..3],
                (Err(Error::SettlementEvidenceMismatch), 8 + index),
                (Err(Error::SettlementEvidenceMismatch), 9 + index),
            );
        }
    }
    let (mut journal, writer, mut roster) = pending(3);
    journal.mark_unknown(writer).unwrap();
    let slots = chain(&journal, writer);
    roster[0].byte_extent += 1;
    journal.members[slots[2]].as_mut().unwrap().next = Some(slots[0]);
    compare(
        &mut journal,
        writer,
        writer,
        &roster[..3],
        (Err(Error::InvalidState), 7),
        (Err(Error::InvalidState), 8),
    );
}

#[test]
fn disposal_shared_retains_original_physical_return_headroom() {
    for fault in 0..8 {
        let (mut journal, writer, roster) = pending(3);
        journal.mark_unknown(writer).unwrap();
        match fault {
            0 => {
                journal.free = core::mem::take(&mut journal.free)
                    .into_boxed_slice()
                    .into_vec()
            }
            1 => {
                journal.member_free = core::mem::take(&mut journal.member_free)
                    .into_boxed_slice()
                    .into_vec()
            }
            2 => {
                journal.allocation_free = core::mem::take(&mut journal.allocation_free)
                    .into_boxed_slice()
                    .into_vec()
            }
            3 => journal.free.resize(journal.writer_capacity, usize::MAX),
            4 => journal
                .member_free
                .resize(journal.allocation_capacity, usize::MAX),
            5 => journal
                .allocation_free
                .resize(journal.allocation_capacity, usize::MAX),
            6 => journal.scratch.truncate(2),
            _ => {
                let slot = chain(&journal, writer)[0];
                let member = journal.members[slot].unwrap();
                journal.scratch[0] = Some(BeginMemberPlanV1 {
                    member_slot: slot,
                    allocation: member.allocation,
                    prior_lineage: member.prior_lineage,
                    attempt_epoch: member.attempt_epoch,
                });
            }
        }
        compare(
            &mut journal,
            writer,
            writer,
            &roster[..3],
            (Err(Error::InvalidState), 10),
            (Err(Error::InvalidState), 11),
        );
    }
}

fn scan_scratch(journal: &Journal, count: usize) -> Result<(), Error> {
    disposal_scratch_scan_body!(disposal_test_expr, journal, count, index, [])
}

#[allow(clippy::question_mark)]
fn observed(
    journal: &Journal,
    writer: Reference,
    roster: &[Write],
    caps: [usize; 3],
    seen: &Cell<u32>,
) -> Result<(Option<usize>, usize), Error> {
    disposal_plan_body!(
        disposal_test_expr,
        journal,
        writer,
        roster,
        {
            seen.set(seen.get() * 10 + 1);
            caps[0]
        },
        {
            seen.set(seen.get() * 10 + 2);
            caps[1]
        },
        {
            seen.set(seen.get() * 10 + 3);
            caps[2]
        },
        crate::context_version_journal::retained::shared_retained_header_v1,
        crate::context_version_journal::retained::shared_retained_chain_v1,
        crate::context_version_journal::retained::shared_retained_allocation_v1,
        scan_scratch,
        head,
        count,
        cursor,
        index,
        [],
        [],
        []
    )
}

#[test]
fn disposal_shared_capacity_observations_are_ordered_lazy_and_once() {
    for fault in 0..10 {
        let (mut journal, writer, mut roster) = pending(3);
        journal.mark_unknown(writer).unwrap();
        let mut caps = [usize::MAX; 3];
        let expected_seen = match fault {
            0 => {
                roster[0].byte_extent += 1;
                0
            }
            1 => {
                journal.free.resize(journal.writer_capacity, usize::MAX);
                0
            }
            2 => {
                caps[0] = 0;
                1
            }
            3 => {
                journal
                    .member_free
                    .resize(journal.allocation_capacity, usize::MAX);
                1
            }
            4 => {
                caps[1] = 0;
                12
            }
            5 => {
                journal
                    .allocation_free
                    .resize(journal.allocation_capacity, usize::MAX);
                12
            }
            6 => {
                caps[2] = 0;
                123
            }
            7 => {
                journal.scratch.truncate(2);
                123
            }
            8 => {
                let member_slot = chain(&journal, writer)[0];
                let member = journal.members[member_slot].unwrap();
                journal.scratch[0] = Some(BeginMemberPlanV1 {
                    member_slot,
                    allocation: member.allocation,
                    prior_lineage: member.prior_lineage,
                    attempt_epoch: member.attempt_epoch,
                });
                123
            }
            _ => 123,
        };
        let before = snapshot(&journal);
        let seen = Cell::new(0);
        let result = observed(&journal, writer, &roster[..3], caps, &seen);
        assert_eq!(
            result,
            match fault {
                0 => Err(Error::SettlementEvidenceMismatch),
                9 => Ok((Some(chain(&journal, writer)[0]), 3)),
                _ => Err(Error::InvalidState),
            }
        );
        assert_eq!(seen.get(), expected_seen);
        assert_eq!(snapshot(&journal), before);
    }
}

#[test]
fn disposal_shared_preserves_raw_scratch_suffix_and_unrelated_corruption() {
    for count in [0, 3] {
        let (mut journal, writer, roster) = pending(count);
        journal.mark_unknown(writer).unwrap();
        journal.scratch[count] = Some(BeginMemberPlanV1 {
            member_slot: usize::MAX,
            allocation: roster[7].allocation,
            prior_lineage: u64::MAX,
            attempt_epoch: 0,
        });
        journal.registration_watermark = u64::MAX;
        journal.reserved_count = usize::MAX;
        journal.member_free[0] = usize::MAX;
        journal.free.push(usize::MAX);
        journal.allocation_free.push(usize::MAX);
        compare(
            &mut journal,
            writer,
            writer,
            &roster[..count],
            (Ok(()), 1 + 3 * count),
            (Ok(()), 4 + 4 * count),
        );
    }
    let (mut journal, writer, roster) = pending(0);
    journal.mark_unknown(writer).unwrap();
    journal.scratch.clear();
    compare(
        &mut journal,
        writer,
        writer,
        &roster[..0],
        (Ok(()), 1),
        (Ok(()), 4),
    );
}

#[test]
fn disposal_shared_returned_slots_reuse_only_fresh_identities() {
    let (mut journal, writer, roster) = pending(3);
    journal.mark_unknown(writer).unwrap();
    compare(
        &mut journal,
        writer,
        writer,
        &roster[..3],
        (Ok(()), 10),
        (Ok(()), 16),
    );
    let fresh = journal.register_writer(key(writer.key.local + 1)).unwrap();
    assert_eq!(fresh.slot, writer.slot);
    for (index, old) in roster[..3].iter().enumerate() {
        let new = journal
            .enroll_allocation(
                ContextAllocationKeyV1 {
                    context_generation: 7,
                    local: 50_000 + index as u64,
                },
                old.device,
                old.byte_extent,
            )
            .unwrap();
        assert_eq!(new.slot, roster[2 - index].allocation.slot);
        assert_eq!(
            journal.lookup_allocation(old.allocation),
            Err(Error::InvalidAllocationReference)
        );
    }
    audit(&journal);
}

#[test]
fn disposal_shared_nonmonotone_member_slots_follow_the_chain() {
    let (mut journal, writer, roster) = reserved_kind(8, 4, Kind::Submission);
    journal.member_free = vec![7, 6, 5, 4, 3, 1, 2, 0];
    journal.begin_write(writer, &roster[..3]).unwrap();
    journal.mark_unknown(writer).unwrap();
    assert_eq!(chain(&journal, writer), [0, 2, 1]);
    compare(
        &mut journal,
        writer,
        writer,
        &roster[..3],
        (Ok(()), 10),
        (Ok(()), 16),
    );
    assert_eq!(journal.member_free, [7, 6, 5, 4, 3, 0, 2, 1]);
    audit(&journal);
}
