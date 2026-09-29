#[test]
fn generic_global_representation_gate_does_not_widen_assertions_or_other_address_spaces() {
    let assertion = GlobalSourceAccessOriginV18::Assertion(ProductionSliceAccessSiteV1::new(
        SemanticFunctionIdV1::from_index(0),
        SemanticFunctionIdV1::from_index(0),
        SemanticBlockIdV1::from_index(0),
        Some(0),
        0,
        SemanticBlockIdV1::from_index(0),
    ));
    let issued = GlobalSourceAccessOriginV18::Issued {
        instance: 0,
        definition: SsaValueV1::Definition(fe2o3_mir_model::SsaDefinitionIdV1::new(0)),
    };
    for space in [
        AddressSpace::Global,
        AddressSpace::Generic,
        AddressSpace::Private,
        AddressSpace::Workgroup,
        AddressSpace::Constant,
    ] {
        assert_eq!(
            global_source_memory_space_v26(&assertion, space),
            space == AddressSpace::Global
        );
        assert_eq!(
            global_source_memory_space_v26(&issued, space),
            matches!(space, AddressSpace::Global | AddressSpace::Generic)
        );
    }
}

pub(super) fn test_issued_generic_global_domains_v26(
    original: &ProductionSourceCorrespondenceV18<'_>,
    optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<usize> {
    let floor = budget.storage();
    let result = original.with_pending_global_accesses_v18(optimized, 0, budget, |view, budget| {
        let output = optimized.output_inventory(budget)?;
        let mut count = 0;
        for operation in output.operations() {
            let Some(pair) = view.access(operation.coordinate, budget)? else { continue; };
            if pair.output.memory.address_space != AddressSpace::Generic { continue; }
            let GlobalSourceAccessOriginV18::Issued { instance, .. } = pair.origin else {
                panic!("Generic source role must retain an exact issued origin");
            };
            assert_ne!(instance, pair.instance, "issuer and helper access are distinct instances");
            assert_eq!(pair.input.memory.address_space, AddressSpace::Generic);
            assert_eq!(pair.input.memory, pair.output.memory);
            for (inventory, endpoint) in [(original.inventory, &pair.input), (output, &pair.output)] {
                let root = optimized_source_definition_row_v18(inventory, endpoint.logical.root, budget)?;
                assert!(matches!(root.ty, Type::Slice(slice) if slice.address_space == AddressSpace::Global));
                let actual = source_operation_row_v18(inventory, endpoint.logical.access.operation, budget)?;
                assert!(matches!(&actual.operation.kind,
                    OperationKind::Load { pointer, access } | OperationKind::Store { pointer, access, .. }
                        if *pointer == endpoint.pointer && *access == endpoint.memory));
                let function = &inventory.functions()[actual.coordinate.block.function.0 as usize];
                let query_floor = budget.storage();
                let origin = source_issued_global_pointer_origin_v26(function.function, endpoint.pointer, budget)
                    .map_err(source_emission_error_v18)?;
                assert_eq!(budget.storage(), query_floor);
                let formation = source_operation_row_v18(inventory, endpoint.logical.address, budget)?;
                assert!(matches!(formation.operation.results.as_slice(), [value] if origin == Some(value.id)));
                assert_eq!(global_source_endpoint_v18(inventory, endpoint.logical, &pair.origin, budget)?, Some(*endpoint));
                let assertion = GlobalSourceAccessOriginV18::Assertion(ProductionSliceAccessSiteV1::new(
                    SemanticFunctionIdV1::from_index(0), SemanticFunctionIdV1::from_index(0),
                    SemanticBlockIdV1::from_index(0), Some(0), 0, SemanticBlockIdV1::from_index(0),
                ));
                assert!(global_source_endpoint_v18(inventory, endpoint.logical, &assertion, budget)?.is_none());
            }
            count += 1;
        }
        Ok::<_, ProductionSourceOwnedViewErrorV18>(count)
    });
    assert_eq!(budget.storage(), floor, "{result:?}");
    result
}
