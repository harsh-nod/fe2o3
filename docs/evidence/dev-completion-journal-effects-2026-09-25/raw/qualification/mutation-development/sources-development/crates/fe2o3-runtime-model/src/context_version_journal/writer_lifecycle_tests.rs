use super::*;

fn compare_register(
    journal: &mut Journal,
    key: Key,
    expected: Result<Reference, Error>,
    accesses: usize,
) {
    let copy = journal.guard_copy_for_test_v1();
    let before = snapshot(journal);
    journal.reset_access_count_for_test_v1();
    assert_eq!(journal.baseline_register_writer_v1(key), expected);
    assert_eq!(journal.guard_accesses_for_test_v1(), accesses);
    let frozen = snapshot(journal);
    let slot = copy.free.last().copied().unwrap_or(usize::MAX);
    journal.restore_writer_for_test_v1(&copy, slot);
    assert_eq!(snapshot(journal), before);
    assert_eq!(journal.register_writer(key), expected);
    assert_eq!(journal.guard_accesses_for_test_v1(), accesses);
    assert_eq!(snapshot(journal), frozen);
    assert_eq!(storage(journal), before.storage);
    if expected.is_err() {
        assert_eq!(snapshot(journal), before);
    }
}

fn compare_abort(
    journal: &mut Journal,
    reference: Reference,
    expected: Result<(), Error>,
    accesses: usize,
) {
    let copy = journal.guard_copy_for_test_v1();
    let before = snapshot(journal);
    journal.reset_access_count_for_test_v1();
    assert_eq!(journal.baseline_abort_reserved_v1(reference), expected);
    assert_eq!(journal.guard_accesses_for_test_v1(), accesses);
    let frozen = snapshot(journal);
    journal.restore_writer_for_test_v1(&copy, reference.slot);
    assert_eq!(snapshot(journal), before);
    assert_eq!(journal.abort_reserved(reference), expected);
    assert_eq!(journal.guard_accesses_for_test_v1(), accesses);
    assert_eq!(snapshot(journal), frozen);
    assert_eq!(storage(journal), before.storage);
    if expected.is_err() {
        assert_eq!(snapshot(journal), before);
    }
}

#[test]
fn writer_shared_registration_header_and_selected_slot_order() {
    for kind in [Kind::Synchronous, Kind::Submission] {
        for local in [0, 9, 10, 11, u64::MAX] {
            for context in [7, 8] {
                for free in [&[][..], &[usize::MAX], &[1, 1], &[usize::MAX, 1]] {
                    for count in [0, 2, usize::MAX] {
                        for occupied in [false, true] {
                            let mut journal = Journal::new(7, 1, 2).unwrap();
                            journal.registration_watermark = 10;
                            journal.reserved_count = count;
                            journal.free.clear();
                            journal.free.extend_from_slice(free);
                            if occupied {
                                journal.writers[1] = Some(WriterEntryV1::Reserved(key(8)));
                            }
                            let k = Key {
                                context_generation: context,
                                local,
                                kind,
                            };
                            let (expected, accesses) = if context != 7 {
                                (Err(Error::ForeignContext), 0)
                            } else if local == 0 || local == u64::MAX {
                                (Err(Error::InvalidWriterId), 0)
                            } else if local <= 10 {
                                (Err(Error::WriterReplay), 0)
                            } else if free.is_empty() {
                                (Err(Error::WriterCapacity), 1)
                            } else if free.last() == Some(&usize::MAX) || occupied || count >= 2 {
                                (Err(Error::InvalidState), 2)
                            } else {
                                (Ok(Reference { slot: 1, key: k }), 4)
                            };
                            compare_register(&mut journal, k, expected, accesses);
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn writer_shared_raw_registration_ignores_unrelated_storage_and_logical_slot_limit() {
    let mut journal = Journal::new(7, 1, 3).unwrap();
    journal.writer_capacity = 1;
    journal.free = vec![usize::MAX, 2, 2];
    journal.allocations.clear();
    journal.members.clear();
    journal.scratch.clear();
    compare_register(
        &mut journal,
        key(10),
        Ok(Reference {
            slot: 2,
            key: key(10),
        }),
        4,
    );
    assert_eq!(journal.free, [usize::MAX, 2]);
}

#[test]
fn writer_shared_lookup_checks_exact_reserved_identity_without_id_admission() {
    for kind in [Kind::Synchronous, Kind::Submission] {
        for local in [0, 10, u64::MAX] {
            for state in 0..4 {
                let mut journal = Journal::new(7, 1, 1).unwrap();
                let k = Key {
                    context_generation: 7,
                    local,
                    kind,
                };
                journal.writers[0] = match state {
                    0 => None,
                    1 => Some(WriterEntryV1::Reserved(k)),
                    2 => Some(WriterEntryV1::Pending {
                        key: k,
                        head: Some(usize::MAX),
                        count: usize::MAX,
                    }),
                    _ => Some(WriterEntryV1::Unknown {
                        key: k,
                        head: None,
                        count: 0,
                    }),
                };
                for slot in [0, usize::MAX] {
                    for fault in 0..4 {
                        let mut reference = Reference { slot, key: k };
                        match fault {
                            1 => reference.key.context_generation = 8,
                            2 => reference.key.local = local.wrapping_add(1),
                            3 => {
                                reference.key.kind = match kind {
                                    Kind::Submission => Kind::Synchronous,
                                    Kind::Synchronous => Kind::Submission,
                                }
                            }
                            _ => {}
                        }
                        let before = snapshot(&journal);
                        let expected = if state == 1 && slot == 0 && fault == 0 {
                            Ok(k)
                        } else {
                            Err(Error::InvalidReference)
                        };
                        journal.reset_access_count_for_test_v1();
                        assert_eq!(journal.baseline_lookup_reserved_v1(reference), expected);
                        assert_eq!(journal.guard_accesses_for_test_v1(), 1);
                        journal.reset_access_count_for_test_v1();
                        assert_eq!(journal.lookup_reserved(reference), expected);
                        assert_eq!(journal.guard_accesses_for_test_v1(), 1);
                        assert_eq!(snapshot(&journal), before);
                    }
                }
            }
        }
    }
}

#[test]
fn writer_shared_abort_raw_count_prefix_capacity_and_reference_order() {
    for local in [0, 10, u64::MAX] {
        for count in [0, 1, usize::MAX] {
            for capacity in [0, 1, 3] {
                for physical in [0, 3] {
                    for free in [&[][..], &[0, 0]] {
                        if free.len() > physical {
                            continue;
                        }
                        for valid in [false, true] {
                            let mut journal = Journal::new(7, 1, 1).unwrap();
                            journal.writer_capacity = capacity;
                            journal.reserved_count = count;
                            journal.registration_watermark = 77;
                            journal.free = Vec::with_capacity(physical);
                            journal.free.extend_from_slice(free);
                            let k = key(local);
                            journal.writers[0] = Some(WriterEntryV1::Reserved(k));
                            let reference = Reference {
                                slot: if valid { 0 } else { usize::MAX },
                                key: k,
                            };
                            let expected = if !valid {
                                Err(Error::InvalidReference)
                            } else if free.len() >= capacity
                                || free.len() >= journal.free.capacity()
                                || count == 0
                            {
                                Err(Error::InvalidState)
                            } else {
                                Ok(())
                            };
                            compare_abort(
                                &mut journal,
                                reference,
                                expected,
                                if expected.is_ok() { 3 } else { 1 },
                            );
                            assert_eq!(journal.registration_watermark, 77);
                        }
                    }
                }
            }
        }
    }
}

#[allow(clippy::question_mark)]
fn observed_abort(
    journal: &mut Journal,
    reference: Reference,
    observations: &Cell<usize>,
    capacity: usize,
) -> Result<(), Error> {
    writer_abort_body!(
        writer_rust_expr,
        journal,
        reference,
        {
            observations.set(observations.get() + 1);
            capacity
        },
        Journal::count_indexed_access
    )
}

#[test]
fn writer_shared_abort_observes_capacity_only_after_reference_and_logical_room() {
    for (valid, logical, observed, count, reads, result) in [
        (false, 0, 0, 0, 0, Err(Error::InvalidReference)),
        (true, 0, 0, 0, 0, Err(Error::InvalidState)),
        (true, 1, 0, 0, 1, Err(Error::InvalidState)),
        (true, 1, 1, 0, 1, Err(Error::InvalidState)),
        (true, 1, 1, 1, 1, Ok(())),
    ] {
        let mut journal = Journal::new(7, 1, 1).unwrap();
        let reference = journal.register_writer(key(10)).unwrap();
        journal.writer_capacity = logical;
        journal.reserved_count = count;
        let reference = Reference {
            slot: if valid { reference.slot } else { usize::MAX },
            ..reference
        };
        let observations = Cell::new(0);
        assert_eq!(
            observed_abort(&mut journal, reference, &observations, observed),
            result
        );
        assert_eq!(observations.get(), reads);
    }
}

#[test]
fn writer_shared_abort_reuse_keeps_watermark_and_constant_indexed_work() {
    for size in [1, 64, 4096] {
        let mut journal = Journal::new(7, size, size).unwrap();
        let first = Reference {
            slot: 0,
            key: key(10),
        };
        compare_register(&mut journal, first.key, Ok(first), 4);
        compare_abort(&mut journal, first, Ok(()), 3);
        compare_register(&mut journal, first.key, Err(Error::WriterReplay), 0);
        let next = Reference {
            slot: 0,
            key: key(11),
        };
        compare_register(&mut journal, next.key, Ok(next), 4);
        compare_abort(&mut journal, first, Err(Error::InvalidReference), 1);
        assert_eq!(journal.lookup_reserved(next), Ok(next.key));
        audit(&journal);
    }
}

#[test]
fn writer_shared_matching_foreign_reserved_key_is_not_owned() {
    let mut journal = Journal::new(7, 1, 1).unwrap();
    let foreign = Key {
        context_generation: 8,
        ..key(10)
    };
    let reference = Reference {
        slot: 0,
        key: foreign,
    };
    journal.writers[0] = Some(WriterEntryV1::Reserved(foreign));
    journal.free.clear();
    journal.reserved_count = 1;
    let before = snapshot(&journal);
    assert_eq!(
        journal.baseline_lookup_reserved_v1(reference),
        Err(Error::InvalidReference)
    );
    assert_eq!(journal.guard_accesses_for_test_v1(), 1);
    journal.reset_access_count_for_test_v1();
    assert_eq!(
        journal.lookup_reserved(reference),
        Err(Error::InvalidReference)
    );
    assert_eq!(journal.guard_accesses_for_test_v1(), 1);
    compare_abort(&mut journal, reference, Err(Error::InvalidReference), 1);
    let observations = Cell::new(0);
    assert_eq!(
        observed_abort(&mut journal, reference, &observations, 1),
        Err(Error::InvalidReference)
    );
    assert_eq!(observations.get(), 0);
    assert_eq!(snapshot(&journal), before);
}
