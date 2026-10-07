use super::*;

#[test]
fn shared_writer_lookup_preserves_all_states_payloads_and_raw_keys() {
    for local in [0, 10, u64::MAX] {
        for kind in [
            ContextWriterKindV1::Submission,
            ContextWriterKindV1::Synchronous,
        ] {
            for state in [
                ContextWriterStateV1::Reserved,
                ContextWriterStateV1::Pending { member_count: 0 },
                ContextWriterStateV1::Pending {
                    member_count: usize::MAX,
                },
                ContextWriterStateV1::Unknown { member_count: 17 },
                ContextWriterStateV1::Unknown {
                    member_count: usize::MAX,
                },
            ] {
                let mut journal = ContextVersionJournalV1::new(7, 1, 2).unwrap();
                let reference = ContextWriterReferenceV1 {
                    slot: 0,
                    key: ContextWriterKeyV1 {
                        context_generation: 7,
                        local,
                        kind,
                    },
                };
                journal.query_replace_writer_for_test_v1(reference, Some(state));
                let storage = journal.guard_storage_for_test_v1();
                let before = alloc::format!("{journal:?}");
                for candidate in [false, true] {
                    let result = if candidate {
                        journal.lookup_writer(reference)
                    } else {
                        journal.baseline_lookup_writer_v1(reference)
                    };
                    assert_eq!(result, Ok(state));
                    assert_eq!(journal.guard_accesses_for_test_v1(), 1);
                    journal.reset_access_count_for_test_v1();
                    assert_eq!(alloc::format!("{journal:?}"), before);
                    assert_eq!(journal.guard_storage_for_test_v1(), storage);
                }
            }
        }
    }
}

#[test]
fn shared_writer_lookup_checks_every_identity_component_and_context() {
    for fault in 0..6 {
        let mut journal = ContextVersionJournalV1::new(7, 1, 2).unwrap();
        let mut reference = ContextWriterReferenceV1 {
            slot: 0,
            key: ContextWriterKeyV1 {
                context_generation: 7,
                local: 10,
                kind: ContextWriterKindV1::Submission,
            },
        };
        journal.query_replace_writer_for_test_v1(
            reference,
            Some(ContextWriterStateV1::Pending { member_count: 1 }),
        );
        match fault {
            0 => reference.slot = usize::MAX,
            1 => reference.slot = 1,
            2 => reference.key.context_generation += 1,
            3 => reference.key.local += 1,
            4 => reference.key.kind = ContextWriterKindV1::Synchronous,
            5 => journal.context_generation += 1,
            _ => unreachable!(),
        }
        let before = alloc::format!("{journal:?}");
        for candidate in [false, true] {
            let result = if candidate {
                journal.lookup_writer(reference)
            } else {
                journal.baseline_lookup_writer_v1(reference)
            };
            assert_eq!(result, Err(ContextVersionJournalErrorV1::InvalidReference));
            assert_eq!(journal.guard_accesses_for_test_v1(), 1);
            journal.reset_access_count_for_test_v1();
            assert_eq!(alloc::format!("{journal:?}"), before);
        }
    }
}
