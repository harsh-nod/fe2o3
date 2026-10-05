use super::layout_tests::*;

const INDEX_WORK: usize = 20_000_000;
const INDEX_STORAGE: usize = 64 * 1024 * 1024;

fn index_scope(consume: impl FnOnce(&SourceStorageRootV29<'_, '_, '_>, &mut Budget<'_>)) {
    super::root_custody_tests::instances(|instances, budget| {
        let before = budget.storage();
        let mut layouts = SourceStorageLayoutsV29::new(instances.owner(), &[WORD], budget).unwrap();
        let table = budget.storage();
        let owned = layouts.lease.owned.get();
        let checkpoint =
            SourceStorageRootCheckpointV29::begin(&mut layouts, instances, budget).unwrap();
        let arena = checkpoint.arena(budget).unwrap();
        consume(&arena.view(), budget);
        // All vectors, states and even unpublished failed-append backing are
        // owned by this arena. The settlement token cannot consume before drop.
        drop(arena);
        let (first, refund) = checkpoint.prepare_refund(budget);
        refund.unwrap().release(budget).unwrap();
        assert_eq!(budget.storage(), table);
        assert_eq!(layouts.lease.owned.get(), owned);
        assert!(layouts.lease.root.get().is_none());
        assert_eq!(layouts.release(budget).is_err(), first.is_some());
        assert_eq!(budget.storage(), before);
    });
}

fn state<'root, 'source>(
    arena: &SourceStorageRootArenaV29<'root, 'source>,
    local: SemanticLocalIdV1,
    budget: &mut Budget<'_>,
) -> SourceStorageStateV29<'root, 'source> {
    SourceStorageStateV29::new_source(
        arena.layouts,
        arena.instances,
        arena.instances.root(),
        local,
        budget,
    )
    .unwrap()
}

#[test]
fn source_snapshot_index_matches_original_linear_canonical_order() {
    for _ in 0..2 {
        index_scope(|root, budget| {
            let arena = root.arena;
            let mut ids = Vec::new();
            for (local, initialized) in [
                (local_for(TWO_ZST), false),
                (local_for(WORD), false),
                (local_for(TWO_ZST), true),
                (local_for(PAIR), false),
                (local_for(TWO_ZST), false),
                (local_for(WORD), false),
                (local_for(TWO_ZST), true),
            ] {
                let mut value = state(arena, local, budget);
                if initialized {
                    let place = value.root_subobject(budget).unwrap();
                    value.set_fact(&place, true, budget).unwrap();
                    place.discard(budget).unwrap();
                }
                let snapshots = arena.snapshots.borrow();
                let expected = snapshots
                    .iter()
                    .position(|row| row.equivalent(&value, budget).unwrap())
                    .unwrap_or(snapshots.len());
                drop(snapshots);
                let retained = arena.record(root.retain_snapshot(value, budget)).unwrap();
                assert_eq!(retained.index, expected);
                ids.push(retained.index);
            }
            assert_eq!(ids, [0, 1, 2, 3, 0, 1, 2]);
            let index = arena.snapshot_index.borrow();
            assert_eq!(index.next, [Some(2), None, None, None]);
            assert_eq!(index.objects.len(), arena.instances.instances().len());
            assert_eq!(
                index.objects[arena.instances.root().index()].len(),
                arena
                    .instances
                    .instance(arena.instances.root())
                    .unwrap()
                    .declaration()
                    .locals()
                    .len()
            );
        });
    }
}

#[test]
fn source_snapshot_index_does_not_merge_physical_and_logical_associations() {
    index_scope(|root, budget| {
        let arena = root.arena;
        let physical = root
            .snapshot_local(arena.instances.root(), local_for(WORD), false, budget)
            .unwrap()
            .unwrap();
        let logical = arena
            .record(root.retain_snapshot(state(arena, local_for(WORD), budget), budget))
            .unwrap();
        assert_ne!(physical, logical);
        assert!(
            !root
                .snapshots_equivalent(physical, logical, budget)
                .unwrap()
        );
        assert_eq!(
            root.snapshot_local(arena.instances.root(), local_for(WORD), false, budget)
                .unwrap(),
            Some(physical)
        );
        assert_eq!(
            arena
                .record(root.retain_snapshot(state(arena, local_for(WORD), budget), budget))
                .unwrap(),
            logical
        );
    });
}

#[test]
fn source_snapshot_index_ignores_unrelated_objects_without_growing_equal_replays() {
    index_scope(|root, budget| {
        let arena = root.arena;
        let target = local_for(TWO_ZST);
        let original = arena
            .record(root.retain_snapshot(state(arena, target, budget), budget))
            .unwrap();
        for local in 0..arena
            .instances
            .instance(arena.instances.root())
            .unwrap()
            .declaration()
            .locals()
            .len()
        {
            let local = SemanticLocalIdV1::from_index(local as u32);
            if local != target {
                let value = state(arena, local, budget);
                arena.record(root.retain_snapshot(value, budget)).unwrap();
            }
            let value = state(arena, target, budget);
            let work = budget.work();
            let retained = root.snapshot_statistics(budget).unwrap();
            let statistics_work = 1 + 1;
            let result = arena.record(root.retain_snapshot(value, budget)).unwrap();
            // Arena identity3 + original object4 + tail3 + one link4.
            // Empty State equivalence and consuming discard charge no work.
            assert_eq!(budget.work() - work, statistics_work + 3 + 4 + 3 + 4);
            assert_eq!(result, original);
            let after = root.snapshot_statistics(budget).unwrap();
            assert_eq!(after.0, retained.0);
            assert_eq!(
                after.1
                    + size_of::<SourceStorageStateV29<'_, '_>>()
                    + size_of::<Result<SourceStorageStateV29<'_, '_>, Error>>(),
                retained.1
            );
        }
    });
}

#[test]
fn source_snapshot_index_lazy_source_rosters_have_independent_exact_limits() {
    for short_work in [false, true] {
        for short_storage in [false, true] {
            if short_work && short_storage {
                continue;
            }
            index_scope(|root, budget| {
                let arena = root.arena;
                let instances = arena.instances.instances().len();
                let locals = arena
                    .instances
                    .instance(arena.instances.root())
                    .unwrap()
                    .declaration()
                    .locals()
                    .len();
                let exact_work = 4 + 3 + instances + 3 + locals;
                let exact_storage = instances * size_of::<Vec<SourceStorageSnapshotBucketV29>>()
                    + locals * size_of::<SourceStorageSnapshotBucketV29>();
                budget
                    .charge_work(INDEX_WORK - budget.work() - exact_work + usize::from(short_work))
                    .unwrap();
                let pressure =
                    INDEX_STORAGE - budget.storage() - exact_storage + usize::from(short_storage);
                budget.reserve_storage(pressure).unwrap();
                let before_work = budget.work();
                let before_storage = budget.storage();
                let result = arena.record(arena.snapshot_index.borrow_mut().prepare_object(
                    arena,
                    arena.instances.root(),
                    local_for(TWO_ZST),
                    budget,
                ));
                assert_eq!(result.is_err(), short_work || short_storage);
                if !short_work && !short_storage {
                    assert_eq!(budget.work(), before_work + exact_work);
                    assert_eq!(budget.storage(), before_storage + exact_storage);
                } else {
                    let stopped = budget.work();
                    assert!(root.snapshot_statistics(budget).is_err());
                    assert_eq!(budget.work(), stopped);
                }
                budget.release_storage(pressure).unwrap();
            });
        }
    }
}

#[test]
fn source_snapshot_index_first_append_has_exact_and_late_one_short_limits() {
    for short_work in [false, true] {
        for short_storage in [false, true] {
            if short_work && short_storage {
                continue;
            }
            index_scope(|root, budget| {
                let arena = root.arena;
                let local = local_for(TWO_ZST);
                arena
                    .snapshot_index
                    .borrow_mut()
                    .prepare_object(arena, arena.instances.root(), local, budget)
                    .unwrap();
                let value = state(arena, local, budget);
                let exact_work = 3 + 4 + 4 + (2 + 3) + (2 + 3);
                let exact_storage =
                    4 * size_of::<Option<usize>>() + 4 * size_of::<SourceStorageStateV29<'_, '_>>();
                budget
                    .charge_work(INDEX_WORK - budget.work() - exact_work + usize::from(short_work))
                    .unwrap();
                let pressure =
                    INDEX_STORAGE - budget.storage() - exact_storage + usize::from(short_storage);
                budget.reserve_storage(pressure).unwrap();
                let before_work = budget.work();
                let before_storage = budget.storage();
                let result = arena.record(root.retain_snapshot(value, budget));
                assert_eq!(result.is_err(), short_work || short_storage);
                let index = arena.snapshot_index.borrow();
                assert_eq!(index.next.len(), 1);
                if !short_work && !short_storage {
                    assert_eq!(result.unwrap().index, 0);
                    assert_eq!(budget.work(), before_work + exact_work);
                    assert_eq!(budget.storage(), before_storage + exact_storage);
                    assert_eq!(
                        index.objects[arena.instances.root().index()][local.index() as usize].first,
                        Some(0)
                    );
                } else {
                    assert!(arena.snapshots.borrow().is_empty());
                    assert_eq!(
                        index.objects[arena.instances.root().index()][local.index() as usize].first,
                        None
                    );
                    assert_eq!(index.next.capacity(), 4);
                    let stopped = budget.work();
                    assert!(root.snapshot_statistics(budget).is_err());
                    assert_eq!(budget.work(), stopped);
                }
                drop(index);
                budget.release_storage(pressure).unwrap();
            });
        }
    }
}

#[test]
fn source_snapshot_index_equal_and_unequal_existing_buckets_have_exact_work_limits() {
    for unequal in [false, true] {
        for short in [false, true] {
            index_scope(|root, budget| {
                let arena = root.arena;
                let local = local_for(TWO_ZST);
                let original = arena
                    .record(root.retain_snapshot(state(arena, local, budget), budget))
                    .unwrap();
                let mut value = state(arena, local, budget);
                if unequal {
                    value.storage_dead(budget).unwrap();
                }
                let exact = 3 + 4 + 3 + 4 + if unequal { 4 + 2 + 2 } else { 0 };
                budget
                    .charge_work(INDEX_WORK - budget.work() - exact + usize::from(short))
                    .unwrap();
                let before = budget.work();
                let result = arena.record(root.retain_snapshot(value, budget));
                assert_eq!(result.is_err(), short);
                if !short {
                    assert_eq!(budget.work(), before + exact);
                    assert_eq!(result.unwrap() == original, !unequal);
                } else {
                    let stopped = budget.work();
                    assert!(root.snapshot_statistics(budget).is_err());
                    assert_eq!(budget.work(), stopped);
                }
            });
        }
    }
}

#[test]
fn source_snapshot_index_rejects_foreign_arena_and_forged_ordinal() {
    for foreign in [false, true] {
        index_scope(|root, budget| {
            let arena = root.arena;
            let snapshot = root
                .snapshot_local(arena.instances.root(), local_for(TWO_ZST), false, budget)
                .unwrap()
                .unwrap();
            let other =
                SourceStorageRootArenaV29::new(arena.layouts, arena.instances, budget).unwrap();
            let forged = SourceStorageSnapshotV29 {
                arena: arena.identity(),
                index: usize::MAX,
                view: PhantomData,
            };
            let result = if foreign {
                other.view().snapshot_ordinal(snapshot, budget)
            } else {
                root.snapshot_ordinal(forged, budget)
            };
            assert!(result.is_err());
            let stopped = budget.work();
            assert!(root.snapshot_ordinal(snapshot, budget).is_err());
            assert_eq!(budget.work(), stopped);
            drop(other);
        });
    }
}

#[test]
fn source_snapshot_index_rejects_cross_object_links_and_cycles() {
    for cycle in [false, true] {
        index_scope(|root, budget| {
            let arena = root.arena;
            let local = local_for(TWO_ZST);
            let first = arena
                .record(root.retain_snapshot(state(arena, local, budget), budget))
                .unwrap();
            let other = arena
                .record(root.retain_snapshot(state(arena, local_for(PAIR), budget), budget))
                .unwrap();
            let mut dead = state(arena, local, budget);
            dead.storage_dead(budget).unwrap();
            let last = arena.record(root.retain_snapshot(dead, budget)).unwrap();
            arena.snapshot_index.borrow_mut().next[first.index] =
                Some(if cycle { first.index } else { other.index });
            let mut value = state(arena, local, budget);
            value.storage_dead(budget).unwrap();
            assert!(arena.record(root.retain_snapshot(value, budget)).is_err());
            assert_eq!(last.index, 2);
        });
    }
}

#[test]
fn source_snapshot_index_checked_ordinal_has_an_independent_exact_work_boundary() {
    for short in [false, true] {
        index_scope(|root, budget| {
            let arena = root.arena;
            let snapshot = root
                .snapshot_local(arena.instances.root(), local_for(TWO_ZST), false, budget)
                .unwrap()
                .unwrap();
            let exact = 1 + 1;
            budget
                .charge_work(INDEX_WORK - budget.work() - exact + usize::from(short))
                .unwrap();
            let before = budget.work();
            let result = root.snapshot_ordinal(snapshot, budget);
            assert_eq!(result.is_err(), short);
            if !short {
                assert_eq!(result.unwrap(), snapshot.index);
                assert_eq!(budget.work(), before + exact);
            } else {
                let stopped = budget.work();
                assert!(root.snapshot_ordinal(snapshot, budget).is_err());
                assert_eq!(budget.work(), stopped);
            }
        });
    }
}

#[test]
fn source_snapshot_index_forged_source_counts_do_not_allocate() {
    index_scope(|root, budget| {
        let arena = root.arena;
        let before = budget.storage();
        let result = arena.record(arena.snapshot_index.borrow_mut().prepare_object(
            arena,
            arena.instances.root(),
            SemanticLocalIdV1::from_index(u32::MAX),
            budget,
        ));
        assert!(result.is_err());
        assert_eq!(budget.storage(), before);
        assert!(arena.snapshot_index.borrow().objects.is_empty());
        assert!(arena.snapshot_index.borrow().next.is_empty());
    });
}

#[test]
fn source_snapshot_index_actual_callback_error_and_panic_drop_owned_backing_before_refund() {
    super::root_custody_tests::instances(|instances, budget| {
        for panic in [false, true] {
            let before = budget.storage();
            let mut layouts =
                SourceStorageLayoutsV29::new(instances.owner(), &[WORD], budget).unwrap();
            let table = budget.storage();
            let owned = layouts.lease.owned.get();
            let result: Result<(), Error> =
                with_source_storage_root_v29(&mut layouts, instances, budget, |_, root, budget| {
                    let empty = root
                        .snapshot_local(instances.root(), local_for(TWO_ZST), false, budget)?
                        .unwrap();
                    let initial = root.snapshot_statistics(budget)?;
                    let written = root.mutate_snapshot(
                        empty,
                        &[],
                        SourceStorageRootMutationV29::Initialize,
                        budget,
                    )?;
                    let final_state = root.snapshot_statistics(budget)?;
                    assert!(final_state.0 > initial.0 && final_state.1 > initial.1);
                    assert_ne!(empty, written);
                    assert_eq!(root.arena.snapshot_index.borrow().next.len(), final_state.0);
                    if panic {
                        panic!("indexed callback panic");
                    }
                    Err(error("indexed callback selected error").into())
                });
            assert!(
                matches!(result, Err(Error::Unsupported { detail, .. }) if detail == if panic { "source reference callback panicked" } else { "indexed callback selected error" })
            );
            assert_eq!(budget.storage(), table);
            assert_eq!(layouts.lease.owned.get(), owned);
            assert!(layouts.lease.root.get().is_none());
            assert!(layouts.release(budget).is_err());
            assert_eq!(budget.storage(), before);
        }
    });
}

#[test]
fn source_snapshot_index_ordinal_checks_cannot_authorize_lost_floor_refund() {
    super::root_custody_tests::instances(|instances, budget| {
        let mut layouts = SourceStorageLayoutsV29::new(instances.owner(), &[WORD], budget).unwrap();
        let table = budget.storage();
        let result =
            with_source_storage_root_v29(&mut layouts, instances, budget, |_, root, budget| {
                let snapshot = root
                    .snapshot_local(instances.root(), local_for(TWO_ZST), false, budget)?
                    .unwrap();
                budget.release_storage(1)?;
                assert!(root.snapshot_ordinal(snapshot, budget).is_err());
                Ok(())
            });
        assert!(matches!(
            result,
            Err(Error::ArgumentCorrespondenceResource(
                ArgumentResourceV1::Accounting
            ))
        ));
        assert!(budget.storage() > table);
        let retained = budget.storage();
        assert!(!layouts.permits_root_emission_refund(instances.owner(), 0, budget));
        assert!(layouts.release(budget).is_err());
        assert_eq!(budget.storage(), retained);
    });
}

#[test]
fn source_snapshot_index_ordinal_rejects_foreign_ledger_without_debit_or_refund() {
    index_scope(|root, budget| {
        let arena = root.arena;
        let snapshot = root
            .snapshot_local(arena.instances.root(), local_for(TWO_ZST), false, budget)
            .unwrap()
            .unwrap();
        let before = budget.storage();
        let mut foreign_work = work();
        let mut foreign = Budget::new(&mut foreign_work, INDEX_STORAGE);
        foreign.reserve_storage(before).unwrap();
        assert!(root.snapshot_ordinal(snapshot, &mut foreign).is_err());
        assert_eq!(foreign.work(), 0);
        assert_eq!(foreign.storage(), before);
        assert_eq!(budget.storage(), before);
        assert!(matches!(
            arena.layouts.lease.failure.resource(),
            Some(ArgumentResourceV1::Accounting)
        ));
    });
}
