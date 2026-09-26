use super::*;

macro_rules! indexed_c1_scope {
    (|$plan:ident, $root:ident, $budget:ident| $body:block) => {{
        let mut owner = nominal_owner(Input::CapturedContext, false);
        let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
        let mut budget = ArgumentBudgetV1::new(&mut work, 64 * 1024 * 1024);
        let capture = owner
            .try_capture_occurrences_with_budget_v1(&mut budget)
            .unwrap();
        budget.reserve_storage(capture.retained_storage()).unwrap();
        let demands =
            source_storage_demands_v29::SourceStorageDemandsV29::collect(&owner, &mut budget)
                .unwrap();
        let mut layouts = source_storage_v29::SourceStorageLayoutsV29::new_with_limits(
            &owner,
            demands.types(&owner, &mut budget).unwrap(),
            ProductionSemanticKirLimitsV1::default().storage_layout_limits(),
            &mut budget,
        )
        .unwrap();
        let result =
            with_production_call_instances_v1(&owner, ROOT, &mut budget, |instances, budget| {
                let floor = budget.storage();
                let result = source_storage_v29::with_source_storage_root_v29(
                    &mut layouts,
                    instances,
                    budget,
                    |$plan, $root, $budget| $body,
                );
                assert_eq!(budget.storage(), floor);
                assert!(layouts.permits_root_emission_refund(&owner, 0, budget));
                Ok::<_, production_call_instances_v1::ProductionCallInstanceErrorV1>(result)
            })
            .unwrap();
        assert_eq!(layouts.release(&mut budget).is_err(), result.is_err());
        demands.discard(&mut budget).unwrap();
        drop(owner);
        budget.release_storage(capture.retained_storage()).unwrap();
        assert_eq!(budget.storage(), 0);
        result
    }};
}

#[test]
fn source_snapshot_plan_index_preserves_roster_ids_and_repeated_call_replay() {
    indexed_c1_scope!(|plan, root, budget| {
        let retained = root.snapshot_statistics(budget)?;
        let floor = budget.storage();
        budget.reserve_storage(source_reference_headers_v29::<()>()?)?;
        let mut builder = SourceReferenceBuilderV29::new_with_root(
            plan.instances,
            SourceReferenceStorageV29::ScalarCells,
            Some(root),
            budget,
        )?;
        builder.function(plan.root, None, budget)?;
        assert_eq!(builder.plan.storage_snapshots, plan.storage_snapshots);
        let count = builder.storage_snapshot_indices.len();
        let capacity = builder.storage_snapshot_indices.capacity();
        for _ in 0..8 {
            builder.function(plan.root, None, budget)?;
            assert_eq!(builder.plan.storage_snapshots, plan.storage_snapshots);
            assert_eq!(
                (
                    builder.storage_snapshot_indices.len(),
                    builder.storage_snapshot_indices.capacity()
                ),
                (count, capacity)
            );
            assert_eq!(
                builder
                    .storage_root
                    .as_ref()
                    .unwrap()
                    .snapshot_statistics(budget)?,
                retained
            );
            for (expected, snapshot) in plan.storage_snapshots.iter().copied().enumerate().rev() {
                assert_eq!(builder.retain_storage_snapshot(snapshot, budget)?, expected);
            }
        }
        // Only C1 scratch was added: every C2 state was canonical beforehand.
        drop(builder);
        budget.release_storage(budget.storage() - floor)?;
        Ok(())
    })
    .unwrap();
}

#[test]
fn source_snapshot_plan_index_preserves_first_plan_order_despite_arena_gaps() {
    indexed_c1_scope!(|plan, root, budget| {
        assert!(plan.storage_snapshots.len() > 2);
        let highest = *plan.storage_snapshots.last().unwrap();
        let ordinal = root.snapshot_ordinal(highest, budget)?;
        assert!(ordinal > 0);
        let floor = budget.storage();
        budget.reserve_storage(source_reference_headers_v29::<()>()?)?;
        let mut builder = SourceReferenceBuilderV29::new_with_root(
            plan.instances,
            SourceReferenceStorageV29::ScalarCells,
            Some(root),
            budget,
        )?;
        assert_eq!(builder.retain_storage_snapshot(highest, budget)?, 0);
        assert_eq!(builder.storage_snapshot_indices.len(), ordinal + 1);
        assert!(
            builder.storage_snapshot_indices[..ordinal]
                .iter()
                .all(Option::is_none)
        );
        assert_eq!(
            builder.retain_storage_snapshot(plan.storage_snapshots[0], budget)?,
            1
        );
        assert_eq!(builder.retain_storage_snapshot(highest, budget)?, 0);
        assert_eq!(
            builder.plan.storage_snapshots,
            [highest, plan.storage_snapshots[0]]
        );
        drop(builder);
        budget.release_storage(budget.storage() - floor)?;
        Ok(())
    })
    .unwrap();
}

#[test]
fn source_snapshot_plan_index_equal_lookup_has_independent_exact_work_boundary() {
    for short in [false, true] {
        let result = indexed_c1_scope!(|plan, root, budget| {
            let snapshot = plan.storage_snapshots[0];
            let floor = budget.storage();
            budget.reserve_storage(source_reference_headers_v29::<()>()?)?;
            let mut builder = SourceReferenceBuilderV29::new_with_root(
                plan.instances,
                SourceReferenceStorageV29::ScalarCells,
                Some(root),
                budget,
            )?;
            builder.retain_storage_snapshot(snapshot, budget)?;
            // Plan custody5 + arena owner1 + ordinal existence1 + map slot1.
            let exact = 5 + 1 + 1 + 1;
            budget.charge_work(usize::MAX - budget.work() - exact + usize::from(short))?;
            let before = budget.work();
            let storage = budget.storage();
            let result = builder.retain_storage_snapshot(snapshot, budget);
            assert_eq!(result.is_err(), short);
            assert_eq!(budget.storage(), storage);
            if short {
                let stopped = budget.work();
                assert!(builder.retain_storage_snapshot(snapshot, budget).is_err());
                assert_eq!(budget.work(), stopped);
            } else {
                assert_eq!(result.unwrap(), 0);
                assert_eq!(budget.work(), before + exact);
            }
            drop(builder);
            budget.release_storage(budget.storage() - floor)?;
            Ok(())
        });
        assert_eq!(result.is_err(), short);
    }
}

fn reverse_index_growth(count: usize) -> (usize, usize, usize) {
    let slot = std::mem::size_of::<Option<usize>>();
    let mut capacity = 0;
    let mut work = 0;
    let mut peak = 0;
    for length in 0..count {
        work += 2;
        if length == capacity {
            let next = capacity.max(2) * 2;
            work += 3 + length;
            peak = peak.max((capacity + next) * slot);
            capacity = next;
        }
    }
    (work, peak, capacity * slot)
}

#[test]
fn source_snapshot_plan_index_gap_growth_has_independent_exact_and_short_limits() {
    for short_work in [false, true] {
        for short_storage in [false, true] {
            if short_work && short_storage {
                continue;
            }
            let result = indexed_c1_scope!(|plan, root, budget| {
                let snapshot = *plan.storage_snapshots.last().unwrap();
                let ordinal = root.snapshot_ordinal(snapshot, budget)?;
                let floor = budget.storage();
                budget.reserve_storage(source_reference_headers_v29::<()>()?)?;
                let mut builder = SourceReferenceBuilderV29::new_with_root(
                    plan.instances,
                    SourceReferenceStorageV29::ScalarCells,
                    Some(root),
                    budget,
                )?;
                let (gap_work, gap_peak, gap_retained) = reverse_index_growth(ordinal + 1);
                let exact_work = 5 + 1 + 1 + 1 + gap_work + 2 + 3;
                let exact_storage = gap_peak.max(
                    gap_retained
                        + 4 * std::mem::size_of::<source_storage_v29::SourceStorageSnapshotV29<'_>>(
                        ),
                );
                budget.charge_work(
                    usize::MAX - budget.work() - exact_work + usize::from(short_work),
                )?;
                let pressure = budget.storage_limit() - budget.storage() - exact_storage
                    + usize::from(short_storage);
                budget.reserve_storage(pressure)?;
                let before_work = budget.work();
                let before_storage = budget.storage();
                let result = builder.retain_storage_snapshot(snapshot, budget);
                assert_eq!(result.is_err(), short_work || short_storage);
                if !short_work && !short_storage {
                    assert_eq!(result.unwrap(), 0);
                    assert_eq!(budget.work(), before_work + exact_work);
                    assert_eq!(budget.peak_storage(), before_storage + exact_storage);
                } else {
                    assert!(builder.plan.storage_snapshots.is_empty());
                    let stopped = budget.work();
                    assert!(builder.retain_storage_snapshot(snapshot, budget).is_err());
                    assert_eq!(budget.work(), stopped);
                }
                drop(builder);
                // No C2 allocation occurred; refund only these dropped C1
                // vectors and caller pressure, never the module/root backing.
                budget.release_storage(budget.storage() - floor)?;
                Ok(())
            });
            assert_eq!(result.is_err(), short_work || short_storage);
        }
    }
}

#[test]
fn source_snapshot_plan_index_rejects_changed_reverse_mapping_and_keeps_first_error() {
    let result = indexed_c1_scope!(|plan, root, budget| {
        let snapshot = plan.storage_snapshots[0];
        let ordinal = root.snapshot_ordinal(snapshot, budget)?;
        let floor = budget.storage();
        budget.reserve_storage(source_reference_headers_v29::<()>()?)?;
        let mut builder = SourceReferenceBuilderV29::new_with_root(
            plan.instances,
            SourceReferenceStorageV29::ScalarCells,
            Some(root),
            budget,
        )?;
        builder.retain_storage_snapshot(snapshot, budget)?;
        builder.storage_snapshot_indices[ordinal] = Some(usize::MAX);
        assert!(matches!(
            builder.retain_storage_snapshot(snapshot, budget),
            Err(
                ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                    ArgumentResourceV1::Accounting
                )
            )
        ));
        let stopped = budget.work();
        assert!(builder.retain_storage_snapshot(snapshot, budget).is_err());
        assert_eq!(budget.work(), stopped);
        assert_eq!(builder.plan.storage_snapshots, [snapshot]);
        drop(builder);
        budget.release_storage(budget.storage() - floor)?;
        Ok(())
    });
    assert!(matches!(
        result,
        Err(
            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                ArgumentResourceV1::Accounting
            )
        )
    ));
}
