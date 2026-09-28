use super::storage_transfer_tests::{TransferCase, transfer_owner};
use super::*;

macro_rules! transfer_scope {
    ($owner:expr, |$plan:ident, $root:ident, $budget:ident| $body:block) => {{
        let owner = $owner;
        let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
        let mut budget = ArgumentBudgetV1::new(&mut work, 64 * 1024 * 1024);
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
        let result = production_call_instances_v1::with_production_call_instances_v1(
            &owner,
            ROOT,
            &mut budget,
            |instances, budget| {
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
            },
        )
        .unwrap();
        let cleanup = layouts.release(&mut budget);
        assert_eq!(cleanup.is_err(), result.is_err());
        demands.discard(&mut budget).unwrap();
        assert_eq!(budget.storage(), 0);
        result
    }};
}

#[test]
fn source_partial_identical_snapshot_join_has_an_independent_exact_work_boundary() {
    for short in [false, true] {
        let checked = std::cell::Cell::new(false);
        let result = transfer_scope!(
            transfer_owner(TransferCase::NestedMove),
            |plan, root, budget| {
                let helper = capture_instance(plan, 0);
                let entry = plan.entries[helper.index()].unwrap();
                let snapshot = plan.storage_snapshots[plan.states[entry][1].storage.unwrap()];
                // Owner authentication1 + one by-value root fact comparison1.
                // The original CAPTURE argument has no paths, guards, enum rows or
                // physical byte-write intervals, and equality allocates nothing.
                let exact = 1 + 1;
                budget.charge_work(usize::MAX - budget.work() - exact + usize::from(short))?;
                let before_work = budget.work();
                let before_storage = budget.storage();
                let joined = root.join_snapshots(snapshot, snapshot, budget);
                assert_eq!(joined.is_err(), short);
                assert_eq!(budget.storage(), before_storage);
                assert_eq!(
                    budget.work().checked_sub(before_work).unwrap(),
                    exact - usize::from(short)
                );
                if short {
                    let stopped = budget.work();
                    assert!(root.join_snapshots(snapshot, snapshot, budget).is_err());
                    assert_eq!(budget.work(), stopped);
                } else {
                    assert_eq!(joined.unwrap(), snapshot);
                }
                checked.set(true);
                Err::<(), _>(source_reference_error_v29("completed exact snapshot boundary").into())
            }
        );
        assert!(
            checked.get(),
            "resource assertions must finish without a caught panic"
        );
        if short {
            assert!(matches!(
                result,
                Err(
                    ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                        ArgumentResourceV1::Work(_)
                    )
                )
            ));
        } else {
            assert!(matches!(
                result,
                Err(ProductionSemanticKirErrorV1::Unsupported {
                    detail: "completed exact snapshot boundary",
                    ..
                })
            ));
        }
    }
}

#[test]
fn source_partial_equal_transfers_do_not_retain_temporary_snapshot_copies() {
    transfer_scope!(
        transfer_owner(TransferCase::NestedMove),
        |plan, root, budget| {
            let helper = capture_instance(plan, 0);
            let entry = plan.entries[helper.index()].unwrap();
            let snapshot = plan.storage_snapshots[plan.states[entry][1].storage.unwrap()];
            let written = root.mutate_snapshot(
                snapshot,
                &[],
                source_storage_v29::SourceStorageRootMutationV29::Initialize,
                budget,
            )?;
            let retained = root.snapshot_statistics(budget)?;
            let storage = budget.storage();
            for _ in 0..16 {
                assert_eq!(root.join_snapshots(written, written, budget)?, written);
                assert_eq!(
                    root.mutate_snapshot(
                        written,
                        &[],
                        source_storage_v29::SourceStorageRootMutationV29::Initialize,
                        budget
                    )?,
                    written
                );
                assert_eq!(root.snapshot_statistics(budget)?, retained);
                assert_eq!(budget.storage(), storage);
            }
            Ok(())
        }
    )
    .unwrap();
}

#[test]
fn source_partial_equal_lifetime_transfer_has_exact_and_one_short_temporary_storage() {
    for short in [false, true] {
        let result = transfer_scope!(
            transfer_owner(TransferCase::NestedMove),
            |plan, root, budget| {
                let helper = capture_instance(plan, 0);
                let entry = plan.entries[helper.index()].unwrap();
                let empty = plan.storage_snapshots[plan.states[entry][4].storage.unwrap()];
                let restarted = root.snapshot_lifetime(empty, true, budget)?;
                let retained = root.snapshot_statistics(budget)?;
                // This source-local snapshot has no facts, byte ranges or enum
                // rows. A repeated StorageLive copies only these two owned state
                // envelopes, then interns against the existing exact state.
                let exact =
                    std::mem::size_of::<source_storage_v29::SourceStorageStateV29<'_, '_>>()
                        + std::mem::size_of::<
                            Result<
                                source_storage_v29::SourceStorageStateV29<'_, '_>,
                                ProductionSemanticKirErrorV1,
                            >,
                        >();
                let padding =
                    budget.storage_limit() - budget.storage() - exact + usize::from(short);
                budget.reserve_storage(padding)?;
                let before = budget.storage();
                let repeated = root.snapshot_lifetime(restarted, true, budget);
                assert_eq!(repeated.is_err(), short);
                assert_eq!(budget.storage(), before);
                if short {
                    assert_eq!(budget.failed_storage(), Some(before + exact));
                    let work = budget.work();
                    assert!(root.snapshot_lifetime(restarted, true, budget).is_err());
                    assert_eq!(budget.work(), work);
                } else {
                    assert_eq!(repeated.unwrap(), restarted);
                    assert_eq!(root.snapshot_statistics(budget)?, retained);
                    assert_eq!(budget.failed_storage(), None);
                }
                budget.release_storage(padding)?;
                Ok(())
            }
        );
        if short {
            assert!(matches!(
                result,
                Err(
                    ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                        ArgumentResourceV1::Storage(_)
                    )
                )
            ));
        } else {
            result.unwrap();
        }
    }
}

#[test]
fn source_partial_repeated_cfg_and_call_summaries_reuse_retained_c2_states() {
    transfer_scope!(
        transfer_owner(TransferCase::RepeatedCall),
        |plan, root, budget| {
            let retained = root.snapshot_statistics(budget)?;
            let floor = budget.storage();
            budget.reserve_storage(source_reference_headers_v29::<()>()?)?;
            let mut builder = SourceReferenceBuilderV29::new_with_root(
                plan.instances,
                SourceReferenceStorageV29::ScalarCells,
                Some(root),
                budget,
            )?;
            for _ in 0..8 {
                builder.function(plan.root, None, budget)?;
                assert_eq!(
                    builder
                        .storage_root
                        .as_ref()
                        .unwrap()
                        .snapshot_statistics(budget)?,
                    retained
                );
            }
            // The additional planner is paid C1 scratch. Its C2 snapshots were
            // canonical before every replay, so dropping it leaves only this exact
            // independently observed C1 delta to refund inside the callback.
            drop(builder);
            budget.release_storage(budget.storage() - floor)?;
            Ok(())
        }
    )
    .unwrap();
}

#[test]
fn source_partial_snapshot_join_rejects_distinct_original_instance_storage() {
    let error = transfer_scope!(
        transfer_owner(TransferCase::NestedMove),
        |plan, root, budget| {
            let left = capture_instance(plan, 0);
            let right = capture_instance(plan, 1);
            let left = plan.storage_snapshots[plan.states[plan.entries[left.index()].unwrap()][1]
                .storage
                .unwrap()];
            let right = plan.storage_snapshots[plan.states[plan.entries[right.index()].unwrap()]
                [1]
            .storage
            .unwrap()];
            assert!(!root.snapshots_equivalent(left, right, budget)?);
            assert!(root.join_snapshots(left, right, budget).is_err());
            let work = budget.work();
            assert!(root.snapshots_equivalent(left, left, budget).is_err());
            assert_eq!(budget.work(), work);
            Ok(())
        }
    )
    .unwrap_err();
    assert!(matches!(
        error,
        ProductionSemanticKirErrorV1::Unsupported { .. }
    ));
}
