use super::*;

#[path = "begin_tests/benchmark_baseline.rs"]
mod benchmark_baseline;

#[path = "begin_tests/performance.rs"]
mod performance;

fn restore(journal: &mut Journal, before: &Snapshot) {
    journal.context_generation = before.context;
    (journal.allocation_capacity, journal.writer_capacity) = before.capacities;
    journal.registration_watermark = before.watermark;
    journal.reserved_count = before.reserved_count;
    journal.writers.clone_from(&before.writers);
    journal.free.clone_from(&before.free);
    journal.allocations.clone_from(&before.allocations);
    journal.allocation_free.clone_from(&before.allocation_free);
    journal.members.clone_from(&before.members);
    journal.member_free.clone_from(&before.member_free);
    journal.scratch.clone_from(&before.scratch);
    journal.reset_access_count_for_test_v1();
    assert_eq!(storage(journal), before.storage);
}

fn compare_frozen(
    journal: &mut Journal,
    writer: Reference,
    roster: &[Write],
) -> (Result<(), Error>, usize) {
    let before = snapshot(journal);
    journal.reset_access_count_for_test_v1();
    let expected = journal.baseline_begin_write_v1(writer, roster);
    let accesses = journal.indexed_accesses.get();
    let after = snapshot(journal);
    restore(journal, &before);
    let result = journal.begin_write(writer, roster);
    assert_eq!(result, expected);
    assert_eq!(journal.indexed_accesses.get(), accesses);
    assert_eq!(snapshot(journal), after);
    assert_eq!(storage(journal), before.storage);
    (result, accesses)
}

#[test]
fn shared_begin_matches_frozen_rejections_and_exact_access_counts() {
    for fault in 0..22 {
        for index in 0..3 {
            let mut journal = Journal::new(7, 3, 1).unwrap();
            let mut roster = enroll(&mut journal, &[100, 200, 300]);
            let mut writer = journal.register_writer(key(41)).unwrap();
            inject_begin_fault(&mut journal, &mut writer, &mut roster, fault, index);
            let before = snapshot(&journal);
            let expected_result = begin_fault_oracle(&journal, writer, &roster).map(|_| ());
            let (result, accesses) = compare_frozen(&mut journal, writer, &roster);
            assert_eq!(result, expected_result);
            let expected = match fault {
                0 | 19 | 20 => 12 * roster.len() + 2,
                1..=5 | 9 | 21 => 1,
                6..=8 | 10..=13 => index + 2,
                14..=15 => roster.len() + 1,
                16..=17 => roster.len() + 3 * index + 3,
                18 => roster.len() + 3 * index + 4,
                _ => unreachable!(),
            };
            assert_eq!(accesses, expected, "fault={fault}, index={index}");
            if result.is_err() {
                assert_eq!(snapshot(&journal), before);
            }
        }
    }
}

#[test]
fn shared_begin_preserves_dirty_tails_and_raw_lineage() {
    for count in 0..=2 {
        let mut journal = Journal::new(7, 4, 1).unwrap();
        let roster = enroll(&mut journal, &[100, 200, 300]);
        let writer = journal.register_writer(key(41)).unwrap();
        let dirty = BeginMemberPlanV1 {
            member_slot: usize::MAX,
            allocation: roster[2].allocation,
            prior_lineage: u64::MAX,
            attempt_epoch: 0,
        };
        journal.scratch[count..].fill(Some(dirty));
        journal.member_free[0] = usize::MAX;
        journal.members[3] = Some(MemberEntryV1 {
            writer,
            allocation: roster[2].allocation,
            prior_lineage: 99,
            attempt_epoch: 0,
            next: Some(usize::MAX),
        });
        for entry in journal.allocations.iter_mut().flatten() {
            entry.content_lineage = u64::MAX;
        }
        let before = snapshot(&journal);
        let (result, accesses) = compare_frozen(&mut journal, writer, &roster[..count]);
        assert_eq!(result, Ok(()));
        assert_eq!(accesses, 12 * count + 2);
        assert_eq!(journal.scratch, before.scratch);
        assert_eq!(journal.member_free[0], usize::MAX);
        assert_eq!(journal.members[3], before.members[3]);
        for destination in &roster[..count] {
            let entry = journal.allocations[destination.allocation.slot].unwrap();
            assert_eq!((entry.attempt_epoch, entry.content_lineage), (1, u64::MAX));
        }
    }
}

#[test]
fn shared_begin_keeps_raw_member_alias_overwrite_and_empty_metadata() {
    let mut journal = Journal::new(7, 4, 1).unwrap();
    let roster = enroll(&mut journal, &[100, 200]);
    let writer = journal.register_writer(key(41)).unwrap();
    journal.member_free = vec![0, 3, 0, 0];
    journal.reserved_count = usize::MAX;
    let before = snapshot(&journal);
    let (result, accesses) = compare_frozen(&mut journal, writer, &roster);
    assert_eq!(result, Ok(()));
    assert_eq!(accesses, 26);
    assert_eq!(journal.member_free, vec![0, 3]);
    assert_eq!(journal.reserved_count, usize::MAX - 1);
    assert_eq!(
        journal.members[0],
        Some(MemberEntryV1 {
            writer,
            allocation: roster[1].allocation,
            prior_lineage: 0,
            attempt_epoch: 1,
            next: None
        })
    );
    for destination in &roster {
        assert_eq!(
            journal.allocations[destination.allocation.slot]
                .unwrap()
                .pending_member,
            Some(0)
        );
    }
    restore(&mut journal, &before);
    journal.allocation_capacity = 0;
    journal.writer_capacity = 0;
    journal.registration_watermark = u64::MAX;
    journal.free.clear();
    journal.member_free.clear();
    journal.scratch.clear();
    let (result, accesses) = compare_frozen(&mut journal, writer, &[]);
    assert_eq!(result, Ok(()));
    assert_eq!(accesses, 2);
}
