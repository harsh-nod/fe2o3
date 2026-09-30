fn source_descriptor_replay_site_v1762() -> ProductionSliceAccessSiteV1 {
    ProductionSliceAccessSiteV1::new(
        SemanticFunctionIdV1::from_index(0),
        SemanticFunctionIdV1::from_index(0),
        SemanticBlockIdV1::from_index(1),
        Some(0),
        0,
        SemanticBlockIdV1::from_index(0),
    )
}

pub(super) fn check_original_descriptor_replay_facts_v1762(
    original: &ProductionSourceCorrespondenceV18<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<()> {
    let physical = original.source.root(0, budget)?.1;
    let function = &original.inventory.functions()[physical];
    let operations = &original.inventory.operations()[function.operations.clone()];
    let read = operations
        .iter()
        .find(|operation| matches!(operation.operation.kind, OperationKind::Load { .. }))
        .unwrap();
    let OperationKind::Load { pointer, .. } = read.operation.kind else {
        unreachable!()
    };
    let definition = |value| {
        operations
            .iter()
            .find(|operation| {
                operation
                    .operation
                    .results
                    .iter()
                    .any(|result| result.id == value)
            })
            .unwrap()
    };
    let address = definition(pointer);
    let OperationKind::GetElementPointer { base, .. } = address.operation.kind else {
        panic!("real source address must be an element projection")
    };
    let data = definition(base);
    assert!(matches!(
        data.operation.kind,
        OperationKind::SliceData { .. }
    ));
    let compare = operations
        .iter()
        .find(|operation| {
            matches!(
                operation.operation.kind,
                OperationKind::Compare {
                    predicate: ComparePredicate::LessThan,
                    ..
                }
            )
        })
        .unwrap();
    let OperationKind::Compare { rhs, .. } = compare.operation.kind else {
        unreachable!()
    };
    // Follow the actual value's defining operation, not an optimized endpoint
    // or an ordinal reconstructed from counts.
    let mut length = definition(rhs);
    while let OperationKind::Cast {
        kind: CastKind::Bitcast,
        value,
        ..
    } = length.operation.kind
    {
        length = definition(value);
    }
    assert!(matches!(
        length.operation.kind,
        OperationKind::SliceLength { .. }
    ));
    let predecessors: Vec<_> = original.inventory.edges()[function.edges.clone()]
        .iter()
        .filter(|edge| edge.target == read.coordinate.block)
        .collect();
    assert_eq!(predecessors.len(), 1);
    let expected_edge = predecessors[0].coordinate;
    let entry = budget.storage();
    let visits = std::cell::Cell::new(0);
    original.with_descriptor_replay_access_v18(
        0,
        0,
        source_descriptor_replay_site_v1762(),
        false,
        budget,
        |view, local, facts| {
            visits.set(visits.get() + 1);
            assert_eq!(local, Some(SemanticLocalIdV1::from_index(1)));
            assert!(std::ptr::eq(view.facts, &facts.common));
            assert_eq!(view.access().operation, read.coordinate);
            assert_eq!(facts.address_operation, address.coordinate);
            assert_eq!(facts.data_operation, data.coordinate);
            assert_eq!(facts.length_operation, length.coordinate);
            assert_eq!(
                facts.guard_condition,
                SliceDefinition::Result {
                    operation: compare.coordinate,
                    result: 0,
                }
            );
            assert_eq!(facts.guard_edge, expected_edge);
            let summary = DescriptorAccessSummaryV18::from_source_facts(facts);
            assert_eq!(summary.access, read.coordinate);
            assert_eq!(summary.address, address.coordinate);
            assert_eq!(summary.data, data.coordinate);
            assert_eq!(summary.length, length.coordinate);
            assert_eq!(summary.logical.root, view.input());
            assert_eq!(summary.logical.index, view.index());
            assert_eq!(summary.logical.guard_condition, facts.guard_condition);
            assert_eq!(summary.logical.guard_edge, expected_edge);
            Ok(())
        },
    )?;
    assert_eq!(visits.get(), 1);
    assert_eq!(budget.storage(), entry);
    Ok(())
}

#[test]
fn original_descriptor_replay_facts_header_has_an_independent_full_layout_oracle() {
    #[allow(dead_code)]
    struct FactsMirror<'a> {
        common: SliceFacts<'a>,
        address: SliceOperation,
        data: SliceOperation,
        length: SliceOperation,
        condition: SliceDefinition,
        edge: fe2o3_kernel_ir::CanonicalKirEdgeCoordinateV1,
    }
    assert_eq!(
        source_slice_replay_headers_v18().unwrap(),
        size_of::<FactsMirror<'_>>() + std::mem::align_of::<FactsMirror<'_>>()
    );
    assert!(size_of::<FactsMirror<'_>>() > size_of::<SliceFacts<'_>>());
}

pub(super) fn check_original_origin_floor_v1762(
    original: &ProductionSourceCorrespondenceV18<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<()> {
    let physical = original.source.root(0, budget)?.1;
    let function = &original.inventory.functions()[physical];
    let floor = budget.storage();
    for _ in 0..2 {
        super::value_origin_v1::with_whole_value_origins_v18(
            original,
            function.coordinate,
            budget,
            |_origins, budget| -> SourceOwnedResultV18<()> {
                assert!(matches!(
                    function.function.signature.parameters.first(),
                    Some(Type::Slice(_))
                ));
                assert!(budget.storage() > floor);
                Ok(())
            },
        )?;
        assert_eq!(budget.storage(), floor);
    }
    check_original_descriptor_replay_facts_v1762(original, budget)
}

pub(super) fn check_foreign_descriptor_site_v1762(
    original: &ProductionSourceCorrespondenceV18<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
    visited: &std::cell::Cell<bool>,
) -> SourceOwnedResultV18<()> {
    let mut site = source_descriptor_replay_site_v1762();
    site.root = SemanticFunctionIdV1::from_index(1);
    let floor = budget.storage();
    let error = original
        .with_descriptor_replay_access_v18(0, 0, site, false, budget, |_, _, _| {
            visited.set(true);
            Ok(())
        })
        .unwrap_err();
    assert!(
        matches!(
            error,
            ProductionSourceOwnedViewErrorV18::Binding("slice source/root/forwarding association")
        ),
        "{error:?}"
    );
    assert_eq!(budget.storage(), floor);
    // Catching the local error must not clear the real source latch.
    Ok(())
}
