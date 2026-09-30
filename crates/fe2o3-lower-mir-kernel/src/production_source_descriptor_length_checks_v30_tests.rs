pub(super) fn test_descriptor_length_refusal_v30(
    original: &ProductionSourceCorrespondenceV18<'_>,
    optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    fault: u8,
    reached: &std::cell::Cell<bool>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<()> {
    original.global_expression_entry_v23(optimized, budget)?;
    let rows = &original
        .source
        .root_row(0)?
        .rvalue_results
        .as_ref()
        .unwrap()
        .rows;
    let locator = &rows[0];
    let assignment = descriptor_length_assignment_v30(original, 0, locator, budget)?;
    assert_eq!(
        descriptor_length_source_scalar_v30(original, assignment, budget)?,
        Some(ScalarType::U32)
    );
    let SourceRvalueEndpointV30::Scalar {
        value,
        scalar: ScalarType::Index,
    } = locator.endpoint
    else {
        panic!("original raw length locator");
    };
    let function = &original.inventory.functions()[original.source.root(0, budget)?.1];
    let definition = original
        .inventory
        .definition_for_value(function.coordinate, value, budget)
        .map_err(source_pointer_inventory_error_v18)?
        .unwrap();
    let baseline = descriptor_length_operation_v30(
        original,
        original.inventory,
        definition.coordinate,
        ScalarType::U32,
        budget,
    )?;
    assert_eq!(baseline.0.block.function, function.coordinate);
    original.check_descriptor_operand_v30(0, locator, baseline.1, budget)?;
    let expected;
    let result = match fault {
        0 => {
            let cloned = *locator;
            expected = "descriptor length locator differs";
            descriptor_length_assignment_v30(original, 0, &cloned, budget).map(|_| ())
        }
        1 => {
            expected = "descriptor length has no exact operation result";
            descriptor_length_operation_v30(
                original,
                original.inventory,
                SliceDefinition::FunctionArgument {
                    function: function.coordinate,
                    argument: 0,
                },
                ScalarType::U32,
                budget,
            )
            .map(|_| ())
        }
        2 => {
            expected = "descriptor length original or output opcode differs";
            let compare = original
                .inventory
                .operations()
                .iter()
                .find(|row| matches!(row.operation.kind, OperationKind::Compare { .. }))
                .unwrap();
            descriptor_length_operation_v30(
                original,
                original.inventory,
                SliceDefinition::Result {
                    operation: compare.coordinate,
                    result: 0,
                },
                ScalarType::U32,
                budget,
            )
            .map(|_| ())
        }
        3 => {
            expected = "descriptor length result or element type differs";
            descriptor_length_operation_v30(
                original,
                original.inventory,
                definition.coordinate,
                ScalarType::U64,
                budget,
            )
            .map(|_| ())
        }
        4 => {
            expected = "descriptor operand archived receiver differs";
            let other = &rows[1];
            assert_eq!(other.ty, locator.ty);
            let other_receiver = other
                .descriptor
                .expect("second original descriptor")
                .receiver;
            assert_ne!(other_receiver, baseline.1);
            let before_type = original
                .inventory
                .definition_for_value(function.coordinate, baseline.1, budget)
                .map_err(source_pointer_inventory_error_v18)?
                .unwrap()
                .ty;
            let after_type = original
                .inventory
                .definition_for_value(function.coordinate, other_receiver, budget)
                .map_err(source_pointer_inventory_error_v18)?
                .unwrap()
                .ty;
            assert_eq!(before_type, after_type);
            original.check_descriptor_operand_v30(0, other, other_receiver, budget)?;
            original.check_descriptor_operand_v30(0, locator, other_receiver, budget)
        }
        _ => panic!("closed descriptor length fault"),
    };
    let error = result.unwrap_err();
    assert!(
        matches!(&error, ProductionSourceOwnedViewErrorV18::Binding(detail) if *detail == expected),
        "{error:?}"
    );
    reached.set(true);
    Err(error)
}

fn descriptor_origin_header_oracle_v30() -> usize {
    #[allow(dead_code)]
    enum Origin {
        Exact(SliceDefinition),
        Unknown,
        Cyclic,
    }
    assert_eq!(size_of::<Origin>(), size_of::<DescriptorOriginV30>());
    type Frame<'a> = (
        [&'a (); 6],
        [usize; 4],
        ValueId,
        Origin,
        SliceDefinition,
        SliceOperation,
        Option<SliceDefinition>,
        &'a CanonicalKirOperationRefV1<'a>,
        &'a fe2o3_kernel_analysis::CanonicalKirDefinitionRefV1<'a>,
        SliceResult<Option<SliceDefinition>>,
        SliceResult<Origin>,
        Result<Option<SliceDefinition>, CanonicalKirInventoryErrorV1>,
        Result<
            Option<&'a fe2o3_kernel_analysis::CanonicalKirDefinitionRefV1<'a>>,
            CanonicalKirInventoryErrorV1,
        >,
        Option<&'a CanonicalKirOperationRefV1<'a>>,
        std::ops::Range<usize>,
    );
    size_of::<Frame<'_>>() + std::mem::align_of::<Frame<'_>>()
}

#[test]
fn descriptor_length_headers_have_exact_independent_query_and_transport_oracles() {
    type Frame<'a> = (
        [&'a (); 12],
        [usize; 8],
        &'a SourceRvalueRowV30,
        &'a fe2o3_mir_model::semantic_mir_v1::SemanticAssignmentV1,
        std::slice::Iter<'a, SourceRvalueRowV30>,
        std::slice::Iter<'a, Option<SourceSliceArgumentCompletionV25>>,
        [SliceDefinition; 4],
        [SliceOperation; 2],
        [ValueId; 2],
        [DescriptorOriginV30; 2],
        SourceOwnedResultV18<()>,
        SourceOwnedResultV18<(
            &'a SourceRvalueRowV30,
            &'a fe2o3_mir_model::semantic_mir_v1::SemanticAssignmentV1,
        )>,
        SourceOwnedResultV18<Option<ScalarType>>,
        SourceOwnedResultV18<(SliceOperation, ValueId)>,
        SourceOwnedResultV18<&'a fe2o3_mir_model::semantic_mir_v1::SemanticAssignmentV1>,
        Result<Type, ProductionSemanticKirErrorV1>,
    );
    let origin = descriptor_origin_header_oracle_v30();
    assert_eq!(source_descriptor_origin_headers_v30().unwrap(), origin);
    assert_eq!(
        descriptor_length_headers_v30().unwrap(),
        size_of::<Frame<'_>>() + std::mem::align_of::<Frame<'_>>() + origin
    );
}
