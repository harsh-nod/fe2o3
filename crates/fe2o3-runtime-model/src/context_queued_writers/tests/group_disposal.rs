use super::*;

type Failure = ContextQueuedWriterGroupDisposalErrorV1;
type Evidence<'a> = ContextQueuedWriterGroupDisposalEvidenceV1<'a>;

fn roots<'a>(
    writers: &[Writer],
    rosters: &'a [Vec<Write>],
) -> Vec<ContextWriterDisposalEvidenceV1<'a>> {
    writers
        .iter()
        .zip(rosters)
        .map(|(&writer, allocations)| ContextWriterDisposalEvidenceV1 {
            writer,
            allocations,
        })
        .collect()
}

struct GroupFixture {
    f: Fixture,
    writers: Vec<Writer>,
    rosters: Vec<Vec<Write>>,
    union: Vec<Write>,
    neighbor: Writer,
}

impl GroupFixture {
    fn new() -> Self {
        let mut f = Fixture::new(5, 16);
        let a = f.active(&[0, 1]);
        let b = f.active(&[2]);
        let c = f.queued(&[(0, Some(a)), (2, Some(b)), (3, None)]);
        let d = f.queued(&[(1, Some(a)), (3, Some(c))]);
        let neighbor = f.active(&[4]);
        let writers = vec![a, b, c, d];
        let rosters = [vec![0, 1], vec![2], vec![0, 2, 3], vec![1, 3]]
            .into_iter()
            .map(|slots| slots.into_iter().map(|i| f.writes[i]).collect())
            .collect();
        for &writer in &writers {
            f.journal.mark_unknown(writer).unwrap();
        }
        let union = f.writes[..4].to_vec();
        audit(&f.journal);
        Self {
            f,
            writers,
            rosters,
            union,
            neighbor,
        }
    }
}

fn rejected(
    j: &mut Journal,
    writers: &[ContextWriterDisposalEvidenceV1<'_>],
    allocations: &[Write],
) {
    let before = snapshot(j);
    let error = j
        .validate_unknown_group_disposal(writers, allocations)
        .unwrap_err();
    assert_eq!(
        j.dispose_unknown_group(&Evidence {
            writers,
            allocations
        }),
        Err(Failure::Rejected(error))
    );
    assert_eq!(snapshot(j), before);
    assert!(!j.disposal_is_terminal());
}

#[test]
fn closed_diamond_with_idle_member_destroys_union_once_and_preserves_neighbor() {
    let mut g = GroupFixture::new();
    let saved_storage = storage(&g.f.journal);
    let outside =
        g.f.journal
            .lookup_allocation(g.f.writes[4].allocation)
            .unwrap();
    let entries = roots(&g.writers, &g.rosters);
    g.f.journal
        .validate_unknown_group_disposal(&entries, &g.union)
        .unwrap();
    g.f.journal
        .dispose_unknown_group(&Evidence {
            writers: &entries,
            allocations: &g.union,
        })
        .unwrap();
    assert!(!g.f.journal.disposal_is_terminal());
    audit(&g.f.journal);
    for write in &g.union {
        assert_eq!(
            g.f.journal.lookup_allocation(write.allocation),
            Err(Error::InvalidAllocationReference)
        );
    }
    for &writer in &g.writers {
        assert_eq!(
            g.f.journal.lookup_writer(writer),
            Err(Error::InvalidReference)
        );
    }
    assert_eq!(
        g.f.journal.lookup_allocation(g.f.writes[4].allocation),
        Ok(outside)
    );
    assert_eq!(g.f.journal.remaining_allocation_slots(), 4);
    assert_eq!(g.f.journal.remaining_member_slots(), 15);
    assert_eq!(storage(&g.f.journal), saved_storage);
    assert_eq!(g.f.journal.registration_watermark(), g.neighbor.key.local);
    g.f.success(g.neighbor);
}

#[test]
fn shared_active_and_queued_unknown_no_longer_deadlock_on_disposal() {
    let mut f = Fixture::new(1, 4);
    let a = f.active(&[0]);
    let b = f.queued(&[(0, Some(a))]);
    f.journal.mark_unknown(a).unwrap();
    f.journal.mark_unknown(b).unwrap();
    assert_eq!(
        f.journal.validate_unknown_disposal(a, &f.writes),
        Err(Error::AllocationBusy)
    );
    assert_eq!(
        f.journal
            .settle_no_effect(b, &ContextWriterNoEffectEvidenceV1 { writer: b }),
        Err(Error::InvalidState)
    );
    let entries = [
        ContextWriterDisposalEvidenceV1 {
            writer: a,
            allocations: &f.writes,
        },
        ContextWriterDisposalEvidenceV1 {
            writer: b,
            allocations: &f.writes,
        },
    ];
    f.journal
        .dispose_unknown_group(&Evidence {
            writers: &entries,
            allocations: &f.writes,
        })
        .unwrap();
    audit(&f.journal);
    assert_eq!(f.journal.remaining_allocation_slots(), 1);
    assert_eq!(f.journal.remaining_member_slots(), 4);
}

#[test]
fn queued_only_and_zero_member_unknown_groups_are_supported() {
    for empty in [false, true] {
        let mut f = Fixture::new(1, 4);
        let writer = if empty {
            f.active(&[])
        } else {
            f.queued(&[(0, None)])
        };
        f.journal.mark_unknown(writer).unwrap();
        let members = if empty { &[][..] } else { &f.writes[..] };
        let entries = [ContextWriterDisposalEvidenceV1 {
            writer,
            allocations: members,
        }];
        let before = f.journal.lookup_allocation(f.writes[0].allocation).unwrap();
        assert_eq!((before.attempt_epoch, before.content_lineage), (0, 0));
        f.journal
            .dispose_unknown_group(&Evidence {
                writers: &entries,
                allocations: members,
            })
            .unwrap();
        audit(&f.journal);
        assert_eq!(
            f.journal.lookup_writer(writer),
            Err(Error::InvalidReference)
        );
        if empty {
            assert_eq!(
                f.journal.lookup_allocation(f.writes[0].allocation),
                Ok(before)
            );
        } else {
            assert_eq!(
                f.journal.lookup_allocation(f.writes[0].allocation),
                Err(Error::InvalidAllocationReference)
            );
        }
    }
}

#[test]
fn every_omitted_writer_or_allocation_rejects_without_refund() {
    for writer in 0..4 {
        let mut g = GroupFixture::new();
        let _ = g.writers.remove(writer);
        g.rosters.remove(writer);
        rejected(&mut g.f.journal, &roots(&g.writers, &g.rosters), &g.union);
    }
    for allocation in 0..4 {
        let mut g = GroupFixture::new();
        g.union.remove(allocation);
        rejected(&mut g.f.journal, &roots(&g.writers, &g.rosters), &g.union);
    }
}

#[test]
fn malformed_rosters_identities_and_unowned_union_members_reject_before_commit() {
    for fault in 0..13 {
        let mut g = GroupFixture::new();
        match fault {
            0 => g.writers[0].key.local += 20,
            1 => g.writers[0].slot = usize::MAX,
            2 => g.writers.swap(0, 1),
            3 => g.writers[1] = g.writers[0],
            4 => g.union.swap(0, 1),
            5 => g.union[1] = g.union[0],
            6 => g.union[0].device.local += 1,
            7 => g.union[0].byte_extent -= 1,
            8 => g.rosters[2].pop().map(|_| ()).unwrap(),
            9 => g.rosters[2][0].byte_extent -= 1,
            10 => g.union[0].allocation.key.local += 30,
            11 => {
                g.f.success(g.neighbor);
                g.union.push(g.f.writes[4]);
            }
            _ => g.rosters[3].swap(0, 1),
        }
        rejected(&mut g.f.journal, &roots(&g.writers, &g.rosters), &g.union);
    }
}

#[test]
fn active_pending_and_unmarked_queued_roots_are_not_disposal_premises() {
    for mark_parent in [false, true] {
        let mut f = Fixture::new(1, 4);
        let a = f.active(&[0]);
        let b = f.queued(&[(0, Some(a))]);
        if mark_parent {
            f.journal.mark_unknown(a).unwrap();
        } else {
            f.journal.mark_unknown(b).unwrap();
        }
        let entries = [
            ContextWriterDisposalEvidenceV1 {
                writer: a,
                allocations: &f.writes,
            },
            ContextWriterDisposalEvidenceV1 {
                writer: b,
                allocations: &f.writes,
            },
        ];
        rejected(&mut f.journal, &entries, &f.writes);
    }
}

#[test]
fn destination_readers_and_consumers_of_outside_inputs_must_release_first() {
    for producer in [false, true] {
        for outside in [false, true] {
            // Stable destination reads coexist with queued-only ownership only
            // if injected, which is deliberately not done: use a stable outside
            // input, and real producer reads on active destinations.
            if !producer && !outside {
                continue;
            }
            let mut f = Fixture::new(2, 8);
            let source = if producer && outside {
                Some(f.active(&[1]))
            } else {
                None
            };
            let a = f.active(&[0]);
            let consumer = if outside { a.key } else { key(12) };
            let mut stable_refs = [None];
            let mut producer_refs = [None];
            if producer {
                let request = ContextProducerReadV1 {
                    producer: source.unwrap_or(a),
                    read: f.read(usize::from(outside)),
                };
                f.journal
                    .acquire_producer_reads(consumer, &[request], &mut producer_refs)
                    .unwrap();
            } else {
                f.journal
                    .acquire_reads(consumer, &[f.read(1)], &mut stable_refs)
                    .unwrap();
            }
            f.journal.mark_unknown(a).unwrap();
            let roster = [f.writes[0]];
            let entries = [ContextWriterDisposalEvidenceV1 {
                writer: a,
                allocations: &roster,
            }];
            rejected(&mut f.journal, &entries, &roster);
            let evidence = ContextReadQuiescenceEvidenceV1 { consumer };
            if producer {
                f.journal
                    .release_producer_reads(consumer, &[producer_refs[0].unwrap()], &evidence)
                    .unwrap();
            } else {
                f.journal
                    .release_reads(consumer, &[stable_refs[0].unwrap()], &evidence)
                    .unwrap();
            }
            f.journal
                .dispose_unknown_group(&Evidence {
                    writers: &entries,
                    allocations: &roster,
                })
                .unwrap();
            audit(&f.journal);
        }
    }
}

#[test]
fn queue_index_corruption_is_rejected_before_any_group_destruction() {
    for fault in 0..5 {
        let mut g = GroupFixture::new();
        let head = g.f.journal.heads[0].unwrap();
        match fault {
            0 => g.f.journal.queued_counts[0] += 1,
            1 => g.f.journal.tails[0] = Some(head),
            2 => g.f.journal.members[head].as_mut().unwrap().next_allocation = Some(usize::MAX),
            3 => g.f.journal.roots[g.writers[2].slot].as_mut().unwrap().count -= 1,
            _ => g.f.journal.heads[0] = None,
        }
        rejected(&mut g.f.journal, &roots(&g.writers, &g.rosters), &g.union);
    }
}

#[test]
fn every_inner_commit_boundary_retains_outer_custody_and_becomes_terminal_on_error_or_panic() {
    for panic in [false, true] {
        // One queued-only retirement, four writer retirements, then outer cleanup.
        for prefix in 0..6 {
            let mut g = GroupFixture::new();
            let entries = roots(&g.writers, &g.rosters);
            let evidence = Evidence {
                writers: &entries,
                allocations: &g.union,
            };
            let free = g.f.journal.remaining_member_slots();
            g.f.journal.disposal_fault = Some((prefix, panic));
            let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                g.f.journal.dispose_unknown_group(&evidence)
            }));
            if panic {
                assert!(outcome.is_err());
            } else {
                assert_eq!(
                    outcome.unwrap(),
                    Err(Failure::Terminal(Error::InvalidState))
                );
            }
            assert!(g.f.journal.disposal_is_terminal());
            assert_eq!(g.f.journal.remaining_member_slots(), free);
            for &writer in &g.writers {
                assert!(g.f.journal.roots[writer.slot].is_some());
            }
            assert_eq!(
                g.f.journal.inner.remaining_allocation_slots(),
                match prefix {
                    0 => 0,
                    1 => 1,
                    2 => 3,
                    _ => 4,
                }
            );
            assert_eq!(
                g.f.journal.inner.remaining_writer_slots(),
                11 + prefix.saturating_sub(1)
            );
            assert_eq!(
                g.f.journal.dispose_unknown_group(&evidence),
                Err(Failure::Terminal(Error::InvalidState))
            );
            assert_eq!(
                g.f.journal.register_writer(key(20)),
                Err(Error::InvalidState)
            );
            assert_eq!(
                g.f.journal
                    .validate_no_queued_writer(g.f.writes[4].allocation),
                Err(Error::InvalidState)
            );
            assert_eq!(
                g.f.journal.settle_success(
                    g.neighbor,
                    &ContextWriterSuccessEvidenceV1 { writer: g.neighbor }
                ),
                Err(Error::InvalidState)
            );
            assert_eq!(
                g.f.journal.enroll_allocation(
                    ContextAllocationKeyV1 {
                        context_generation: 7,
                        local: 30
                    },
                    g.f.writes[0].device,
                    64
                ),
                Err(Error::InvalidState)
            );
            assert_eq!(
                g.f.journal
                    .acquire_mixed_reads(key(20), &[], &mut [], &[], &mut []),
                Err(Error::InvalidState)
            );
            assert_eq!(
                g.f.journal.settle_success(
                    g.neighbor,
                    &ContextWriterSuccessEvidenceV1 {
                        writer: g.writers[0]
                    }
                ),
                Err(Error::InvalidState)
            );
            assert_eq!(
                g.f.journal.settle_no_effect(
                    g.neighbor,
                    &ContextWriterNoEffectEvidenceV1 {
                        writer: g.writers[0]
                    }
                ),
                Err(Error::InvalidState)
            );
            assert_eq!(
                g.f.journal.dispose_unknown(g.neighbor, &entries[0]),
                Err(Error::InvalidState)
            );
        }
    }
}

#[test]
fn adapter_can_discover_the_complete_component_through_current_neighbors() {
    let mut g = GroupFixture::new();
    let mut visited = vec![g.writers[3]];
    let mut cursor = 0;
    while cursor < visited.len() {
        g.f.journal
            .visit_writer_neighbors(visited[cursor], |writer| {
                if !visited.contains(&writer) {
                    visited.push(writer);
                }
            })
            .unwrap();
        cursor += 1;
    }
    visited.sort_by_key(|writer| writer.key.local);
    assert_eq!(visited, g.writers);
    let entries = roots(&visited, &g.rosters);
    g.f.journal
        .dispose_unknown_group(&Evidence {
            writers: &entries,
            allocations: &g.union,
        })
        .unwrap();
    audit(&g.f.journal);
}

#[test]
fn neighbor_discovery_fails_before_any_callback_on_late_corruption() {
    let mut g = GroupFixture::new();
    let writer = g.writers[2];
    let head = g.f.journal.roots[writer.slot].unwrap().head.unwrap();
    let second = g.f.journal.members[head].unwrap().next_writer.unwrap();
    let third = g.f.journal.members[second].unwrap().next_writer.unwrap();
    g.f.journal.members[third].as_mut().unwrap().next_allocation = Some(usize::MAX);
    let mut called = false;
    assert_eq!(
        g.f.journal
            .visit_writer_neighbors(writer, |_| called = true),
        Err(Error::InvalidState)
    );
    assert!(!called);
}

#[test]
fn completed_group_cannot_dispose_recycled_slots_or_publish_old_lineage() {
    let mut g = GroupFixture::new();
    let entries = roots(&g.writers, &g.rosters);
    let evidence = Evidence {
        writers: &entries,
        allocations: &g.union,
    };
    g.f.journal.dispose_unknown_group(&evidence).unwrap();
    let write = Write {
        allocation: g
            .f
            .journal
            .enroll_allocation(
                ContextAllocationKeyV1 {
                    context_generation: 7,
                    local: 10,
                },
                g.union[0].device,
                64,
            )
            .unwrap(),
        device: g.union[0].device,
        byte_extent: 64,
    };
    assert!(
        g.union
            .iter()
            .any(|old| old.allocation.slot == write.allocation.slot)
    );
    let state = g.f.journal.lookup_allocation(write.allocation).unwrap();
    assert_eq!((state.attempt_epoch, state.content_lineage), (0, 0));
    let writer = g.f.writer();
    assert!(g.writers.iter().any(|old| old.slot == writer.slot));
    g.f.journal.begin_write(writer, &[write]).unwrap();
    rejected(&mut g.f.journal, &entries, &g.union);
    g.f.journal
        .settle_success(writer, &ContextWriterSuccessEvidenceV1 { writer })
        .unwrap();
    audit(&g.f.journal);
}

#[test]
fn terminal_owner_rejects_even_empty_or_malformed_mutations_and_admission() {
    let mut g = GroupFixture::new();
    let entries = roots(&g.writers, &g.rosters);
    g.f.journal.disposal_fault = Some((0, false));
    assert!(matches!(
        g.f.journal.dispose_unknown_group(&Evidence {
            writers: &entries,
            allocations: &g.union
        }),
        Err(Failure::Terminal(_))
    ));
    let read = g.f.read(4);
    let producer = ContextProducerReadV1 {
        read,
        producer: g.neighbor,
    };
    let consumer = key(20);
    let before = snapshot(&g.f.journal);
    let j = &mut g.f.journal;
    assert_eq!(j.begin_write(g.neighbor, &[]), Err(Error::InvalidState));
    assert_eq!(
        j.begin_queued_write(g.neighbor, &[]),
        Err(Error::InvalidState)
    );
    assert_eq!(
        j.validate_queued_write_admission(consumer, &[]),
        Err(Error::InvalidState)
    );
    assert_eq!(j.abort_reserved(g.neighbor), Err(Error::InvalidState));
    assert_eq!(j.validate_read(&read), Err(Error::InvalidState));
    assert_eq!(
        j.validate_producer_read(&producer),
        Err(Error::InvalidState)
    );
    assert_eq!(j.validate_read_capacity(0), Err(Error::InvalidState));
    assert_eq!(
        j.validate_producer_read_capacity(0),
        Err(Error::InvalidState)
    );
    assert_eq!(
        j.acquire_reads(consumer, &[], &mut []),
        Err(Error::InvalidState)
    );
    assert_eq!(
        j.acquire_producer_reads(consumer, &[], &mut []),
        Err(Error::InvalidState)
    );
    assert_eq!(
        j.release_reads(consumer, &[], &ContextReadQuiescenceEvidenceV1 { consumer }),
        Err(Error::InvalidState)
    );
    assert_eq!(
        j.release_producer_reads(consumer, &[], &ContextReadQuiescenceEvidenceV1 { consumer }),
        Err(Error::InvalidState)
    );
    assert_eq!(j.enroll_allocations(&[], &mut []), Err(Error::InvalidState));
    assert_eq!(j.retire_allocations(&[]), Err(Error::InvalidState));
    assert_eq!(j.mark_unknown(g.neighbor), Err(Error::InvalidState));
    assert_eq!(
        j.activate_queued_writer(g.neighbor),
        Err(Error::InvalidState)
    );
    assert_eq!(
        j.validate_queued_writer(g.neighbor, &[]),
        Err(Error::InvalidState)
    );
    assert_eq!(j.latest_writer(read.allocation), Err(Error::InvalidState));
    assert_eq!(
        j.visit_writer_neighbors(g.neighbor, |_| panic!("terminal callback")),
        Err(Error::InvalidState)
    );
    assert_eq!(snapshot(j), before);
}
