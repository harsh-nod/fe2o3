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
