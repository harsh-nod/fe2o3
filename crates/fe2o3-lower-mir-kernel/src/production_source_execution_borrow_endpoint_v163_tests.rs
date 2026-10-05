fn source_execution_borrow_probe_v163(
    work_limit: usize,
    storage_limit: usize,
    query: impl FnOnce(
        &ProductionSourceCorrespondenceV18<'_>,
        &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<()>,
) -> (
    Result<(), ProductionSourceOptimizationErrorV18<ProductionSourceOwnedViewErrorV18>>,
    usize,
    usize,
) {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
    let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let result = (|| {
        let prepared = prepared_tile_schedule_result_v155(&mut budget)
            .map_err(ProductionSourceOptimizationErrorV18::Source)?;
        prepared.with_source_consumer_v18(&mut budget, |source, budget| {
            let (output, (), _) =
                source.with_checked_optimization_v18(budget, |relation, _, budget| {
                    query(relation, budget)?;
                    Ok::<_, ProductionSourceOwnedViewErrorV18>(((), 0))
                })?;
            drop(output);
            Ok(())
        })
    })();
    (result, budget.work(), budget.peak_storage())
}

fn check_source_execution_borrows_v163(
    relation: &ProductionSourceCorrespondenceV18<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<()> {
    let archive = relation
        .source
        .root_row(0)?
        .rvalue_results
        .as_ref()
        .unwrap();
    let semantic = relation.source.source_semantic(budget)?;
    let (mut shared, mut mutable) = (0, 0);
    for row in &archive.values {
        let SourceSsaPhysicalV36::ExecutionBorrow(retained) = row.typed.physical else {
            continue;
        };
        let endpoint = relation.ssa_typed_endpoint_v36(0, row.instance, row.original, budget)?;
        assert!(endpoint.reference(budget)?.is_none());
        let before = budget.storage();
        let coordinates = endpoint.execution_borrow_v163(budget)?.unwrap();
        assert_eq!(budget.storage(), before);
        assert_eq!(coordinates, retained.coordinates);
        let (function, _) = relation
            .source
            .instance(0, coordinates.site.instance, budget)?;
        let statement = &semantic.functions()[function.index() as usize].blocks()
            [coordinates.site.block.index() as usize]
            .statements()[coordinates.site.statement];
        let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
            panic!("execution borrow is not an original assignment")
        };
        let SemanticRvalueKindV1::Borrow { kind, place } = assignment.value().kind() else {
            panic!("execution borrow lost its original rvalue")
        };
        assert_eq!(*kind, coordinates.kind);
        assert_eq!(place.local(), coordinates.source_local);
        assert_eq!(place.ty(), coordinates.referent_type);
        assert_eq!(
            assignment.destination().local(),
            coordinates.destination_local
        );
        assert_eq!(
            coordinates.parent.is_some(),
            !place.projections().is_empty()
        );
        assert_eq!(
            endpoint.physical_type(budget)?,
            Some(&Type::Execution(retained.role))
        );
        let definition = endpoint.original_definition(budget)?.unwrap();
        assert_eq!(
            relation.inventory.definitions()[definition].value,
            Some(retained.value)
        );
        match coordinates.kind {
            SemanticBorrowKindV1::Shared => shared += 1,
            SemanticBorrowKindV1::Mutable => mutable += 1,
            SemanticBorrowKindV1::Fake => panic!("fake borrow admitted"),
        }
    }
    assert!(shared > 0 && mutable > 0);
    Ok(())
}

#[test]
fn source_execution_borrow_endpoint_rejoins_actual_context_and_workgroup_loans() {
    source_execution_borrow_probe_v163(
        OPTIMIZED_SOURCE_WORK_LIMIT_V18,
        MODULE_LIMIT,
        check_source_execution_borrows_v163,
    )
    .0
    .unwrap();
}

#[test]
fn source_execution_borrow_endpoint_exact_and_one_short_resources() {
    let run = |work, storage| {
        source_execution_borrow_probe_v163(work, storage, check_source_execution_borrows_v163)
    };
    let (result, work, peak) = run(OPTIMIZED_SOURCE_WORK_LIMIT_V18, MODULE_LIMIT);
    result.unwrap();
    let (result, used, retained_peak) = run(work, peak);
    result.unwrap();
    assert_eq!((used, retained_peak), (work, peak));
    assert!(run(work - 1, peak).0.is_err());
    assert!(run(work, peak - 1).0.is_err());
}

#[test]
fn source_execution_borrow_endpoint_preserves_account_custody() {
    for foreign in [false, true] {
        let (result, _, _) = source_execution_borrow_probe_v163(
            OPTIMIZED_SOURCE_WORK_LIMIT_V18,
            MODULE_LIMIT,
            |relation, budget| {
                let archive = relation
                    .source
                    .root_row(0)?
                    .rvalue_results
                    .as_ref()
                    .unwrap();
                let row = archive
                    .values
                    .iter()
                    .find(|row| {
                        matches!(row.typed.physical, SourceSsaPhysicalV36::ExecutionBorrow(_))
                    })
                    .unwrap();
                let endpoint =
                    relation.ssa_typed_endpoint_v36(0, row.instance, row.original, budget)?;
                let error = if foreign {
                    let mut work =
                        CanonicalKernelIrWorkBudgetV1::new(OPTIMIZED_SOURCE_WORK_LIMIT_V18);
                    let mut other = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
                    other.reserve_storage(budget.storage())?;
                    endpoint.execution_borrow_v163(&mut other).unwrap_err()
                } else {
                    budget.release_storage(1)?;
                    endpoint.execution_borrow_v163(budget).unwrap_err()
                };
                assert!(matches!(
                    error,
                    ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Accounting)
                ));
                assert!(endpoint.execution_borrow_v163(budget).is_err());
                Err(error)
            },
        );
        assert!(result.is_err());
    }
}
