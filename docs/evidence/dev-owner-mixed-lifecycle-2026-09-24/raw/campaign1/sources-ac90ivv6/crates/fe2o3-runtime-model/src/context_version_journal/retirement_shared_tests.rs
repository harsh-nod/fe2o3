use super::*;

#[allow(unused_macros)]
#[macro_use]
mod templates {
    include!("retirement_bodies.rs");
}

macro_rules! retirement_test_expr {
    ($body:expr) => {
        $body
    };
}

fn fixture() -> (Journal, Vec<ContextAllocationReferenceV1>) {
    let mut journal = Journal::new(7, 4, 1).unwrap();
    journal.fault_enrollment_for_test_v1(&[3, 1, 2, 0], 4);
    let references = [10, 20, 30]
        .into_iter()
        .map(|local| {
            journal
                .enroll_allocation(
                    ContextAllocationKeyV1 {
                        context_generation: 7,
                        local,
                    },
                    ContextJournalDeviceKeyV1 {
                        context_generation: 7,
                        local: 11,
                    },
                    64,
                )
                .unwrap()
        })
        .collect();
    (journal, references)
}

fn compare(
    journal: &mut Journal,
    references: &[ContextAllocationReferenceV1],
    expected: Result<(), Error>,
    accesses: usize,
) {
    let saved = journal.guard_copy_for_test_v1();
    let before = snapshot(journal);
    for baseline in [true, false] {
        journal.reset_access_count_for_test_v1();
        let result = if baseline {
            journal.baseline_validate_allocation_retirement_v1(references)
        } else {
            journal.validate_allocation_retirement(references)
        };
        assert_eq!(result, expected);
        assert_eq!(journal.guard_accesses_for_test_v1(), accesses);
        assert_eq!(snapshot(journal), before);
    }
    journal.reset_access_count_for_test_v1();
    assert_eq!(journal.baseline_retire_allocations_v1(references), expected);
    assert_eq!(journal.guard_accesses_for_test_v1(), accesses);
    let frozen = snapshot(journal);
    assert_eq!(storage(journal), before.storage);
    if expected.is_err() {
        assert_eq!(frozen, before);
    }
    journal.restore_retirement_for_test_v1(&saved, references);
    assert_eq!(snapshot(journal), before);
    assert_eq!(journal.retire_allocations(references), expected);
    assert_eq!(journal.guard_accesses_for_test_v1(), accesses);
    assert_eq!(snapshot(journal), frozen);
    assert_eq!(storage(journal), before.storage);
}

#[test]
fn retirement_shared_roster_header_precedes_identity() {
    for capacity in 0..3 {
        let (mut journal, mut references) = fixture();
        journal.allocation_capacity = capacity;
        references[0].slot = usize::MAX;
        compare(&mut journal, &references, Err(Error::RosterCapacity), 0);
    }
}

#[test]
fn retirement_shared_exact_identity_at_every_position() {
    for index in 0..3 {
        for fault in 0..5 {
            let (mut journal, mut references) = fixture();
            match fault {
                0 => references[index].slot = usize::MAX,
                1 => references[index].slot = 3,
                2 => references[index].key.local = 0,
                3 => references[index].key.context_generation = 8,
                _ => {
                    journal.allocations[references[index].slot]
                        .as_mut()
                        .unwrap()
                        .key
                        .context_generation = 8
                }
            }
            compare(
                &mut journal,
                &references,
                Err(Error::InvalidAllocationReference),
                index + 1,
            );
        }
    }
}

#[test]
fn retirement_shared_identity_then_order_then_pending() {
    for invalid in [true, false] {
        let (mut journal, references) = fixture();
        journal.allocations[references[0].slot]
            .as_mut()
            .unwrap()
            .pending_member = Some(usize::MAX);
        let mut reversed = [references[1], references[0]];
        if invalid {
            reversed[1].slot = usize::MAX;
        }
        compare(
            &mut journal,
            &reversed,
            Err(if invalid {
                Error::InvalidAllocationReference
            } else {
                Error::NonCanonicalRoster
            }),
            2,
        );
    }
    let (mut journal, references) = fixture();
    compare(
        &mut journal,
        &[references[0]; 2],
        Err(Error::NonCanonicalRoster),
        2,
    );
    for index in 0..3 {
        for unknown in [false, true] {
            let (mut journal, references) = fixture();
            let writer = journal.register_writer(key(1)).unwrap();
            journal
                .begin_write(
                    writer,
                    &[ContextAllocationWriteV1 {
                        allocation: references[index],
                        device: ContextJournalDeviceKeyV1 {
                            context_generation: 7,
                            local: 11,
                        },
                        byte_extent: 64,
                    }],
                )
                .unwrap();
            if unknown {
                journal.mark_unknown(writer).unwrap();
            }
            compare(
                &mut journal,
                &references,
                Err(Error::AllocationBusy),
                index + 1,
            );
        }
    }
}

#[test]
fn retirement_shared_headroom_uses_original_physical_storage() {
    for tight in [false, true] {
        let (mut journal, references) = fixture();
        if tight {
            journal.allocation_free = core::mem::take(&mut journal.allocation_free)
                .into_boxed_slice()
                .into_vec();
            assert_eq!(journal.allocation_free.capacity(), 1);
        } else {
            journal.allocation_capacity = 3;
        }
        compare(&mut journal, &references, Err(Error::InvalidState), 3);
    }
    let (mut journal, _) = fixture();
    journal.allocation_capacity = 0;
    compare(&mut journal, &[], Err(Error::InvalidState), 0);
    journal.allocation_free.clear();
    compare(&mut journal, &[], Ok(()), 0);
}

#[allow(clippy::question_mark)]
fn observed(
    journal: &Journal,
    references: &[ContextAllocationReferenceV1],
    capacity: usize,
    seen: &Cell<usize>,
) -> Result<(), Error> {
    retirement_preflight_body!(
        retirement_test_expr,
        journal,
        references,
        retained::shared_retained_allocation_v1,
        retained::shared_retained_allocation_less_v1,
        {
            seen.set(seen.get() + 1);
            capacity
        },
        previous,
        index,
        []
    )
}

#[test]
fn retirement_shared_capacity_observation_is_lazy_and_once() {
    for fault in 0..7 {
        let (mut journal, mut references) = fixture();
        let mut capacity = journal.allocation_free.capacity();
        let (expected, observations) = match fault {
            0 => {
                journal.allocation_capacity = 0;
                (Err(Error::RosterCapacity), 0)
            }
            1 => {
                references[0].slot = usize::MAX;
                (Err(Error::InvalidAllocationReference), 0)
            }
            2 => {
                references.swap(0, 1);
                (Err(Error::NonCanonicalRoster), 0)
            }
            3 => {
                journal.allocations[references[1].slot]
                    .as_mut()
                    .unwrap()
                    .pending_member = Some(usize::MAX);
                (Err(Error::AllocationBusy), 0)
            }
            4 => {
                journal.allocation_capacity = 3;
                (Err(Error::InvalidState), 0)
            }
            5 => {
                capacity = 0;
                (Err(Error::InvalidState), 1)
            }
            _ => (Ok(()), 1),
        };
        let before = snapshot(&journal);
        let seen = Cell::new(0);
        assert_eq!(observed(&journal, &references, capacity, &seen), expected);
        assert_eq!(seen.get(), observations);
        assert_eq!(snapshot(&journal), before);
    }
    let (journal, _) = fixture();
    let seen = Cell::new(0);
    assert_eq!(observed(&journal, &[], 0, &seen), Err(Error::InvalidState));
    assert_eq!(seen.get(), 1);
}

#[test]
fn retirement_shared_preserves_unrelated_malformed_fields_and_free_prefix() {
    let (mut journal, references) = fixture();
    journal.registration_watermark = u64::MAX;
    journal.reserved_count = usize::MAX;
    journal.writer_capacity = 0;
    journal.free.clear();
    journal.member_free.clear();
    journal.allocation_free[0] = usize::MAX;
    for reference in &references {
        let entry = journal.allocations[reference.slot].as_mut().unwrap();
        entry.device = ContextJournalDeviceKeyV1 {
            context_generation: 0,
            local: 0,
        };
        entry.byte_extent = 0;
        entry.attempt_epoch = 0;
        entry.content_lineage = u64::MAX;
    }
    journal.scratch[0] = Some(BeginMemberPlanV1 {
        allocation: references[0],
        member_slot: usize::MAX,
        attempt_epoch: 0,
        prior_lineage: u64::MAX,
    });
    compare(&mut journal, &references, Ok(()), 3);
    assert_eq!(journal.allocation_free, [usize::MAX, 0, 2, 1]);
}

#[test]
fn retirement_shared_keeps_caller_order_and_fresh_identity_reuse() {
    let (mut journal, references) = fixture();
    compare(&mut journal, &references, Ok(()), 3);
    assert_eq!(journal.allocation_free, [3, 0, 2, 1]);
    audit(&journal);
    let replacement = journal
        .enroll_allocation(
            ContextAllocationKeyV1 {
                context_generation: 7,
                local: 40,
            },
            ContextJournalDeviceKeyV1 {
                context_generation: 7,
                local: 11,
            },
            64,
        )
        .unwrap();
    assert_eq!(replacement.slot, references[2].slot);
    for reference in references {
        assert_eq!(
            journal.lookup_allocation(reference),
            Err(Error::InvalidAllocationReference)
        );
    }
    audit(&journal);
}

#[test]
fn retirement_shared_does_not_add_issuable_identity_checks() {
    for context in [0, 7, u64::MAX] {
        let (mut journal, mut references) = fixture();
        journal.context_generation = context;
        for (reference, local) in references.iter_mut().zip([0, 1, u64::MAX]) {
            reference.key = ContextAllocationKeyV1 {
                context_generation: context,
                local,
            };
            journal.allocations[reference.slot].as_mut().unwrap().key = reference.key;
        }
        compare(&mut journal, &references, Ok(()), 3);
    }
}
