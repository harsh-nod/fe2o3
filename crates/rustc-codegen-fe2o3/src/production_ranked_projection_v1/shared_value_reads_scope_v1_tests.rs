mod shared_value_reads_scope_v1_tests {
    use super::*;
    use fe2o3_kernel_ir::{
        CanonicalKernelIrVerificationResourceBudgetV1 as SharedBudget,
        CanonicalKernelIrWorkBudgetV1 as SharedWork,
    };

    #[test]
    fn shared_value_reads_scope_retains_consumer_bytes_and_refunds_on_semantic_error() {
        let owner = materialized_helper_v1(false);
        let source =
            ranked_projection_source_v1::RankedProjectionSourceV1::from_legacy(&owner).unwrap();
        let floor = owner.retained_analysis_storage_v1();
        let mut work = SharedWork::new(usize::MAX);
        let mut budget = SharedBudget::new(&mut work, usize::MAX);
        budget.reserve_storage(floor).unwrap();
        with_canonical_assertions_source_budget_v1(&source, &mut budget, |session| {
            let function = SemanticFunctionIdV1::from_index(0);
            let mut facts = session.for_source(function, function);
            let base = facts.scalar_private_storage_v1()?;
            let value = shared_value_reads_projection_v1::with_reads(
                owner.semantic_ssa(),
                function,
                &mut facts,
                |_, facts| {
                    facts.reserve_scalar_private_storage_v1(29)?;
                    Ok(17)
                },
            )?;
            assert_eq!(value, 17);
            assert_eq!(facts.scalar_private_storage_v1()?, base + 29);
            facts.release_scalar_private_storage_v1(29)?;
            let result: Result<(), _> = shared_value_reads_projection_v1::with_reads(
                owner.semantic_ssa(),
                function,
                &mut facts,
                |_, _| {
                    Err(ProductionRankedProjectionErrorV1::Unsupported(
                        "Shared scope original refusal",
                    ))
                },
            );
            assert!(matches!(
                result,
                Err(ProductionRankedProjectionErrorV1::Unsupported(
                    "Shared scope original refusal"
                ))
            ));
            assert_eq!(facts.scalar_private_storage_v1()?, base);
            Ok(())
        })
        .unwrap();
        assert_eq!(budget.storage(), floor);
    }

    #[test]
    fn shared_value_reads_scope_preserves_original_panic_after_cleanup() {
        let owner = materialized_helper_v1(false);
        let source =
            ranked_projection_source_v1::RankedProjectionSourceV1::from_legacy(&owner).unwrap();
        let floor = owner.retained_analysis_storage_v1();
        let mut work = SharedWork::new(usize::MAX);
        let mut budget = SharedBudget::new(&mut work, usize::MAX);
        budget.reserve_storage(floor).unwrap();
        with_canonical_assertions_source_budget_v1(&source, &mut budget, |session| {
            let function = SemanticFunctionIdV1::from_index(0);
            let mut facts = session.for_source(function, function);
            let base = facts.scalar_private_storage_v1()?;
            let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _: Result<(), _> = shared_value_reads_projection_v1::with_reads(
                    owner.semantic_ssa(),
                    function,
                    &mut facts,
                    |_, _| std::panic::panic_any(173usize),
                );
            }))
            .unwrap_err();
            assert_eq!(panic.downcast_ref::<usize>(), Some(&173));
            assert_eq!(facts.scalar_private_storage_v1()?, base);
            Ok(())
        })
        .unwrap();
        assert_eq!(budget.storage(), floor);
    }

    #[test]
    fn shared_value_reads_canonical_constructor_rejects_foreign_owner_before_debit() {
        let owner = materialized_helper_v1(false);
        let foreign = materialized_helper_v1(false);
        let source =
            ranked_projection_source_v1::RankedProjectionSourceV1::from_legacy(&owner).unwrap();
        let floor = owner.retained_analysis_storage_v1();
        let mut work = SharedWork::new(usize::MAX);
        let mut budget = SharedBudget::new(&mut work, usize::MAX);
        budget.reserve_storage(floor).unwrap();
        with_canonical_assertions_source_budget_v1(&source, &mut budget, |session| {
            let function = SemanticFunctionIdV1::from_index(0);
            let mut work = SharedWork::new(0);
            let mut query = SharedBudget::new(&mut work, usize::MAX);
            query.reserve_storage(floor).unwrap();
            {
                let mut facts =
                    session.for_source_with_query_budget_v1(&mut query, function, function);
                assert!(matches!(
                    facts.shared_value_reads_v1(foreign.semantic_ssa(), function),
                    Err(ProductionRankedProjectionErrorV1::Unsupported(
                        "Shared read owner/source-function mismatch"
                    ))
                ));
            }
            assert_eq!(query.work(), 0);
            assert_eq!(query.storage(), floor);
            Ok(())
        })
        .unwrap();
        assert_eq!(budget.storage(), floor);
    }
}

mod retained_whole_root_snapshot_controls_v1_tests {
    use super::*;
    use fe2o3_kernel_ir::{
        CanonicalKernelIrVerificationResourceBudgetV1 as SnapshotBudget,
        CanonicalKernelIrWorkBudgetV1 as SnapshotWork,
    };
    // Real materialization over the existing semantic-model fixture; not fresh Rust evidence.
    #[test]
    fn retained_whole_root_snapshot_is_observation_only_and_preserves_exact_binding() {
        let owner = materialized_helper_v1(false);
        let foreign = materialized_helper_v1(false);
        let source =
            ranked_projection_source_v1::RankedProjectionSourceV1::from_legacy(&owner).unwrap();
        let floor = owner.retained_analysis_storage_v1();
        let mut work = SnapshotWork::new(usize::MAX);
        let mut budget = SnapshotBudget::new(&mut work, usize::MAX);
        budget.reserve_storage(floor).unwrap();
        with_canonical_assertions_source_budget_v1(&source, &mut budget, |session| {
            let function = SemanticFunctionIdV1::from_index(0);
            let mut facts = session.for_retained_whole_root_snapshot_control_v1(function, function);
            let mut owned = 0usize;
            let first =
                facts.retained_whole_root_snapshot_v1(&owner, function, &mut owned, None)?;
            let second =
                facts.retained_whole_root_snapshot_v1(&owner, function, &mut owned, Some(first))?;
            assert!(
                first.budget_slot == second.budget_slot
                    && first.work_ledger == second.work_ledger
                    && first.owned_slot == second.owned_slot
                    && first.owned == second.owned
                    && first.storage == second.storage
                    && first.work == second.work
                    && first.peak == second.peak
            );
            assert!(
                facts
                    .retained_whole_root_snapshot_v1(&foreign, function, &mut owned, Some(first))
                    .is_err()
            );
            assert!(
                facts
                    .retained_whole_root_snapshot_v1(
                        &owner,
                        SemanticFunctionIdV1::from_index(1),
                        &mut owned,
                        Some(first)
                    )
                    .is_err()
            );
            let mut copied = owned;
            assert!(
                facts
                    .retained_whole_root_snapshot_v1(&owner, function, &mut copied, Some(first))
                    .is_err()
            );
            let last =
                facts.retained_whole_root_snapshot_v1(&owner, function, &mut owned, Some(first))?;
            assert!(
                last.work == first.work
                    && last.storage == first.storage
                    && last.owned == first.owned
            );
            Ok(())
        })
        .unwrap();
        assert_eq!(budget.storage(), floor);
    }
    #[test]
    fn retained_whole_root_snapshot_surplus_cannot_replace_held_owner_or_counter() {
        let owner = materialized_helper_v1(false);
        let source =
            ranked_projection_source_v1::RankedProjectionSourceV1::from_legacy(&owner).unwrap();
        let floor = owner.retained_analysis_storage_v1();
        let mut work = SnapshotWork::new(usize::MAX);
        let mut budget = SnapshotBudget::new(&mut work, usize::MAX);
        budget.reserve_storage(floor).unwrap();
        with_canonical_assertions_source_budget_v1(&source, &mut budget, |session| {
            let function = SemanticFunctionIdV1::from_index(0);
            let mut facts = session.for_retained_whole_root_snapshot_control_v1(function, function);
            let mut owned = 0usize;
            let entry =
                facts.retained_whole_root_snapshot_v1(&owner, function, &mut owned, None)?;
            facts.reserve_scalar_private_storage_v1(31)?;
            owned = 31;
            let held =
                facts.retained_whole_root_snapshot_v1(&owner, function, &mut owned, Some(entry))?;
            facts.reserve_scalar_private_storage_v1(29)?;
            assert!(
                facts
                    .retained_whole_root_snapshot_v1(&owner, function, &mut owned, Some(held))
                    .is_ok()
            );
            owned -= 1;
            assert!(
                facts
                    .retained_whole_root_snapshot_v1(&owner, function, &mut owned, Some(held))
                    .is_err()
            );
            owned += 1;
            // Remove all unrelated surplus plus one held byte. Keep original held, never rebase.
            facts.release_scalar_private_storage_v1(30)?;
            assert!(
                facts
                    .retained_whole_root_snapshot_v1(&owner, function, &mut owned, Some(held))
                    .is_err()
            );
            facts.release_scalar_private_storage_v1(30)?;
            owned = 0;
            let after =
                facts.retained_whole_root_snapshot_v1(&owner, function, &mut owned, Some(entry))?;
            assert_eq!(after.storage, entry.storage);
            assert_eq!(after.work, entry.work);
            Ok(())
        })
        .unwrap();
        assert_eq!(budget.storage(), floor);
    }
    #[test]
    fn retained_whole_root_snapshot_checked_floor_arithmetic_and_owned_bounds_refuse() {
        let owner = materialized_helper_v1(false);
        let source =
            ranked_projection_source_v1::RankedProjectionSourceV1::from_legacy(&owner).unwrap();
        let floor = owner.retained_analysis_storage_v1();
        let mut work = SnapshotWork::new(usize::MAX);
        let mut budget = SnapshotBudget::new(&mut work, usize::MAX);
        budget.reserve_storage(floor).unwrap();
        with_canonical_assertions_source_budget_v1(&source, &mut budget, |session| {
            let function = SemanticFunctionIdV1::from_index(0);
            let mut facts = session.for_retained_whole_root_snapshot_control_v1(function, function);
            let mut owned = 0usize;
            let entry =
                facts.retained_whole_root_snapshot_v1(&owner, function, &mut owned, None)?;
            owned = entry.storage.checked_add(1).unwrap();
            assert!(
                facts
                    .retained_whole_root_snapshot_v1(&owner, function, &mut owned, None)
                    .is_err()
            );
            owned = 1;
            let mut hostile = entry;
            hostile.storage = usize::MAX;
            assert!(matches!(
                facts.retained_whole_root_snapshot_v1(&owner, function, &mut owned, Some(hostile)),
                Err(ProductionRankedProjectionErrorV1::CanonicalAssertions(
                    CanonicalAssertionErrorV1::Resource(
                        fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1::Arithmetic
                    )
                ))
            ));
            owned = 0;
            let after =
                facts.retained_whole_root_snapshot_v1(&owner, function, &mut owned, Some(entry))?;
            assert_eq!(after.storage, entry.storage);
            assert_eq!(after.work, entry.work);
            Ok(())
        })
        .unwrap();
        assert_eq!(budget.storage(), floor);
    }
    #[test]
    fn retained_whole_root_snapshot_refuses_first_and_later_sticky_denial() {
        let owner = materialized_helper_v1(false);
        let source =
            ranked_projection_source_v1::RankedProjectionSourceV1::from_legacy(&owner).unwrap();
        let floor = owner.retained_analysis_storage_v1();
        for deny_work in [true, false] {
            let mut work = SnapshotWork::new(usize::MAX);
            let mut budget = SnapshotBudget::new(&mut work, usize::MAX);
            budget.reserve_storage(floor).unwrap();
            let result =
                with_canonical_assertions_source_budget_v1(&source, &mut budget, |session| {
                    let function = SemanticFunctionIdV1::from_index(0);
                    let mut facts =
                        session.for_retained_whole_root_snapshot_control_v1(function, function);
                    let mut owned = 0usize;
                    let held = facts
                        .retained_whole_root_snapshot_v1(&owner, function, &mut owned, None)?;
                    let denial = if deny_work {
                        facts.charge_private_array_work(usize::MAX)
                    } else {
                        facts.reserve_scalar_private_storage_v1(usize::MAX)
                    };
                    assert!(denial.is_err());
                    assert!(
                        facts
                            .retained_whole_root_snapshot_v1(&owner, function, &mut owned, None)
                            .is_err()
                    );
                    assert!(
                        facts
                            .retained_whole_root_snapshot_v1(
                                &owner,
                                function,
                                &mut owned,
                                Some(held)
                            )
                            .is_err()
                    );
                    assert_eq!(owned, 0);
                    denial
                });
            assert!(result.is_err());
            if deny_work {
                assert!(budget.failed_work().is_some());
            } else {
                assert!(budget.failed_storage().is_some());
            }
        }
    }
}
