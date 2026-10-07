use super::*;

#[test]
fn lookup_shared_matches_frozen_backlink_validation_without_strengthening_it() {
    for fault in 0..11 {
        let mut journal = Journal::new(7, 2, 1).unwrap();
        let roster = enroll(&mut journal, &[100]);
        let mut reference = roster[0].allocation;
        let writer = journal.register_writer(key(41)).unwrap();
        journal.begin_write(writer, &roster).unwrap();
        let slot = journal.allocations[reference.slot]
            .unwrap()
            .pending_member
            .unwrap();
        match fault {
            0 => {}
            1 => reference.slot = usize::MAX,
            2 => reference.slot = 1,
            3 => reference.key.local += 1,
            4 => reference.key.context_generation += 1,
            5 => {
                journal.allocations[reference.slot]
                    .as_mut()
                    .unwrap()
                    .pending_member = Some(usize::MAX)
            }
            6 => journal.members[slot] = None,
            7 => journal.members[slot].as_mut().unwrap().allocation.slot += 1,
            8 => journal.members[slot].as_mut().unwrap().allocation.key.local += 1,
            9 => {
                journal.members[slot]
                    .as_mut()
                    .unwrap()
                    .allocation
                    .key
                    .context_generation += 1
            }
            10 => {
                let member = journal.members[slot].as_mut().unwrap();
                member.writer.slot = usize::MAX;
                member.writer.key.context_generation = 0;
                member.prior_lineage = u64::MAX;
                member.attempt_epoch = 0;
                member.next = Some(usize::MAX);
            }
            _ => unreachable!(),
        }
        let before = snapshot(&journal);
        journal.reset_access_count_for_test_v1();
        let baseline = journal.baseline_lookup_allocation_v1(reference);
        let counted = journal.guard_accesses_for_test_v1();
        journal.reset_access_count_for_test_v1();
        let result = journal.lookup_allocation(reference);
        assert_eq!(result, baseline, "fault={fault}");
        assert_eq!(journal.guard_accesses_for_test_v1(), counted);
        assert_eq!(counted, if (1..=4).contains(&fault) { 1 } else { 2 });
        assert_eq!(snapshot(&journal), before);
        match fault {
            0 | 10 => assert_eq!(
                result.unwrap().pending_writer,
                Some(journal.members[slot].unwrap().writer)
            ),
            1..=4 => assert_eq!(result, Err(Error::InvalidAllocationReference)),
            _ => assert_eq!(result, Err(Error::InvalidState)),
        }
    }
}
