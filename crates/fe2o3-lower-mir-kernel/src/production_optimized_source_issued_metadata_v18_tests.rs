#[test]
fn pending_global_commoned_metadata_has_independent_header_equation_and_exact_cuts() {
    use fe2o3_kernel_analysis::{
        CanonicalKirDefinitionRefV1, CanonicalKirInventoryV18, CanonicalKirOperationRefV1,
    };
    use fe2o3_kernel_ir::CanonicalKirDefinitionDescendantKindV1 as Kind;
    fn h<T>() -> usize {
        size_of::<T>() + 2 * size_of::<SourceOwnedResultV18<T>>()
    }
    let expected = h::<(
        &ProductionSourceCorrespondenceV18<'_>,
        &ProductionOptimizedSourceCorrespondenceV18<'_>,
        IssuedMetadataKindV18,
        [SliceOperation; 3],
        &mut ArgumentBudgetV1<'_>,
    )>() + h::<(
        &ProductionSourceCorrespondenceV18<'_>,
        &ProductionOptimizedSourceCorrespondenceV18<'_>,
        [SliceDefinition; 2],
        Option<Kind>,
        &mut ArgumentBudgetV1<'_>,
    )>() + h::<(
        (
            &CanonicalKirInventoryV18<'_>,
            SliceOperation,
            &mut ArgumentBudgetV1<'_>,
        ),
        (
            &CanonicalKirInventoryV18<'_>,
            SliceDefinition,
            &mut ArgumentBudgetV1<'_>,
        ),
        (
            &CanonicalKirInventoryV18<'_>,
            fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1,
            ValueId,
            &mut ArgumentBudgetV1<'_>,
        ),
    )>() + h::<(
        [&CanonicalKirOperationRefV1<'_>; 3],
        [&fe2o3_kernel_ir::Operation; 2],
        [&fe2o3_kernel_ir::OperationKind; 3],
        [&[fe2o3_kernel_ir::ValueDef]; 2],
        [&fe2o3_kernel_ir::ValueDef; 2],
        [&ValueId; 3],
        [&CanonicalKirDefinitionRefV1<'_>; 3],
        &CanonicalKirInventoryV18<'_>,
        [ValueId; 2],
        u32,
        ProductionOptimizedSourceOperationV18,
        [SliceOperation; 3],
        Option<SliceOperation>,
        (Option<SliceOperation>, bool),
        bool,
        fe2o3_kernel_analysis::CanonicalKirOutputUseV1,
        Option<fe2o3_kernel_analysis::CanonicalKirOutputUseV1>,
        [fe2o3_kernel_ir::CanonicalKirUseCoordinateV1; 2],
        [SliceDefinition; 3],
        [Option<ValueId>; 2],
        [bool; 3],
        (SliceOperation, bool),
        Option<(SliceOperation, bool)>,
        &[fe2o3_kernel_ir::CanonicalKirDefinitionDescendantV1],
        std::slice::Iter<'_, fe2o3_kernel_ir::CanonicalKirDefinitionDescendantV1>,
        &fe2o3_kernel_ir::CanonicalKirDefinitionDescendantV1,
        bool,
        Option<Kind>,
        Kind,
        (Kind, &fe2o3_kernel_ir::CanonicalKirDefinitionDescendantV1),
        (SliceOperation, &SliceOperation),
    )>() + h::<Option<(SliceOperation, bool)>>()
        + h::<&CanonicalKirOperationRefV1<'_>>()
        + h::<&CanonicalKirDefinitionRefV1<'_>>()
        + h::<Option<&CanonicalKirDefinitionRefV1<'_>>>()
        + size_of::<
            Result<
                Option<&CanonicalKirDefinitionRefV1<'_>>,
                fe2o3_kernel_analysis::CanonicalKirInventoryErrorV1,
            >,
        >()
        + h::<&[fe2o3_kernel_ir::CanonicalKirDefinitionDescendantV1]>()
        + h::<Option<fe2o3_kernel_analysis::CanonicalKirOutputUseV1>>()
        + h::<ProductionOptimizedSourceOperationV18>()
        + h::<()>()
        + h::<Result<(), ArgumentResourceV1>>()
        + 2 * h::<&Type>()
        + h::<[bool; 4]>()
        + h::<[SliceOperation; 4]>()
        + h::<[usize; 2]>()
        + h::<std::array::IntoIter<usize, 2>>()
        + h::<Option<usize>>()
        + h::<usize>()
        + h::<[(usize, IssuedMetadataKindV18, usize); 2]>()
        + h::<(usize, IssuedMetadataKindV18, usize)>()
        + h::<Option<(usize, IssuedMetadataKindV18, usize)>>()
        + h::<std::array::IntoIter<(usize, IssuedMetadataKindV18, usize), 2>>();
    assert_eq!(issued_metadata_headers_v18().unwrap(), expected);
    for short in [false, true] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(10);
        let mut budget = ArgumentBudgetV1::new(&mut work, expected + 17 - usize::from(short));
        budget.reserve_storage(17).unwrap();
        let result = budget.reserve_storage(issued_metadata_headers_v18().unwrap());
        if short {
            assert!(matches!(result, Err(ArgumentResourceV1::Storage(error))
                if error.actual() == expected + 17 && error.limit() == expected + 16));
            assert_eq!(budget.storage(), 17);
        } else {
            result.unwrap();
            assert_eq!(budget.storage(), expected + 17);
            budget.release_storage(expected).unwrap();
        }
        assert_eq!(budget.work(), 0);
        assert_eq!(budget.storage(), 17);
    }
}

pub(super) fn test_issued_commoned_metadata_v18(
    original: &ProductionSourceCorrespondenceV18<'_>,
    optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    fault: Option<u8>,
    reached: &std::cell::Cell<bool>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<()> {
    use fe2o3_kernel_ir::CanonicalKirDefinitionDescendantKindV1 as Kind;
    let floor = budget.storage();
    let result = source_scalar_normalization_scratch_v18(
        original.source.cleanup,
        budget,
        0,
        |budget| {
            original.source.retain_construction(|| {
            budget.reserve_storage(issued_output_headers_v18()?)?;
            let rows = scoped_raw_admission_v29::checked_issued_source_rows_v18(original, 0, budget)?.unwrap();
            assert_eq!(rows.issuers.len(), 2);
            let first = scoped_raw_admission_v29::source_issued_tail_locations_v18(original, 0, &rows.issuers[0], budget)?;
            let second = scoped_raw_admission_v29::source_issued_tail_locations_v18(original, 0, &rows.issuers[1], budget)?;
            let function = optimized_source_root_function_v18(original, optimized, 0, budget)?;
            let inventory = optimized.output_inventory(budget)?;
            let actual = SourceIssuedActualV29::from_function(function.function, budget).map_err(source_emission_error_v18)?;
            let mut selected = [second[0], second[2]];
            for (index, ordinal, kind, consumer) in [(0, 0, IssuedMetadataKindV18::Length, 1),
                (1, 2, IssuedMetadataKindV18::Data, 3)] {
                let ProductionOptimizedSourceOperationV18::Retained { output: target, .. } =
                    optimized.operation(second[consumer], budget)? else { panic!("retained second consumer"); };
                assert!(matches!(optimized.operation(second[ordinal], budget)?,
                    ProductionOptimizedSourceOperationV18::Rewritten { input } if input == second[ordinal]));
                let (output, substituted) = issued_metadata_output_v18(original, optimized, kind,
                    second[ordinal], second[consumer], target, budget)?.expect("commoned pure metadata");
                assert!(substituted);
                selected[index] = output;
                issued_metadata_descendant_v18(original, optimized,
                    SliceDefinition::Result { operation: second[ordinal], result: 0 },
                    SliceDefinition::Result { operation: output, result: 0 }, Some(Kind::Substituted), budget)?;
                let ProductionOptimizedSourceOperationV18::Retained { output: first_consumer, .. } =
                    optimized.operation(first[consumer], budget)? else { panic!("retained first consumer"); };
                let first_output = issued_metadata_output_v18(original, optimized, kind,
                    first[ordinal], first[consumer], first_consumer, budget)?.expect("first retained metadata");
                assert_eq!(first_output, (output, false), "one authentic retained output anchor");
            }
            let fact = issued_output_issuer_v18(original, optimized, 0, &rows.issuers[1], function, &actual, budget)?
                .expect("complete commoned tail schema");
            assert_eq!([fact.output[0], fact.output[2]], selected);
            let Some(fault) = fault else { reached.set(true); return Ok(()); };
            let original_length = SliceDefinition::Result { operation: second[0], result: 0 };
            let output_length = SliceDefinition::Result { operation: selected[0], result: 0 };
            let expected;
            let refused = match fault {
                0 => {
                    expected = "issued metadata changed original consuming use";
                    issued_metadata_output_v18(original, optimized, IssuedMetadataKindV18::Length,
                        first[0], second[1], fact.output[1], budget).map(|_| ())
                }
                1 => {
                    expected = "issued metadata changed retained consumer";
                    let ProductionOptimizedSourceOperationV18::Retained { output, .. } =
                        optimized.operation(first[1], budget)? else { panic!("retained first comparison"); };
                    assert_ne!(output, fact.output[1]);
                    issued_metadata_output_v18(original, optimized, IssuedMetadataKindV18::Length,
                        second[0], second[1], output, budget).map(|_| ())
                }
                2 => {
                    expected = "issued metadata original opcode";
                    issued_metadata_output_v18(original, optimized, IssuedMetadataKindV18::Data,
                        second[0], second[3], fact.output[3], budget).map(|_| ())
                }
                3 => {
                    expected = "issued metadata changed original consuming use";
                    issued_metadata_output_v18(original, optimized, IssuedMetadataKindV18::Length,
                        second[0], second[3], fact.output[3], budget).map(|_| ())
                }
                4 => {
                    expected = "issued metadata selected definition has no checked descendant";
                    issued_metadata_descendant_v18(original, optimized, original_length,
                        SliceDefinition::Result { operation: fact.output[3], result: 0 },
                        Some(Kind::Substituted), budget)
                }
                5 => {
                    expected = "issued metadata selected definition has no checked descendant";
                    let input_function = original.source.root(0, budget)?.1;
                    let receiver = original.inventory.definition_for_value(
                        original.inventory.functions()[input_function].coordinate, rows.issuers[1].receiver, budget)
                        .map_err(source_pointer_inventory_error_v18)?.unwrap();
                    let other = function.function.body.as_ref().unwrap().parameters[1];
                    let other = inventory.definition_for_value(function.coordinate, other, budget)
                        .map_err(source_pointer_inventory_error_v18)?.unwrap();
                    assert_eq!(receiver.ty, other.ty);
                    assert_ne!(Some(fact.root_input), other.value);
                    issued_metadata_descendant_v18(original, optimized, receiver.coordinate, other.coordinate, None, budget)
                }
                6 => {
                    expected = "issued metadata descendant kind or uniqueness";
                    issued_metadata_descendant_v18(original, optimized, original_length, output_length,
                        Some(Kind::Retained), budget)
                }
                7 => {
                    expected = "issued metadata selected definition has no checked descendant";
                    issued_metadata_descendant_v18(original, optimized, original_length,
                        SliceDefinition::Result { operation: selected[0], result: 1 },
                        Some(Kind::Substituted), budget)
                }
                _ => panic!("closed commoned-metadata fault"),
            };
            let error = refused.unwrap_err();
            assert!(matches!(error, ProductionSourceOwnedViewErrorV18::Binding(detail) if detail == expected),
                "fault {fault}: {error:?}");
            reached.set(true);
            Err(error)
        })
        },
    );
    assert_eq!(budget.storage(), floor);
    if let Err(ProductionSourceOwnedViewErrorV18::Binding(expected)) = result.as_ref() {
        let before = (budget.work(), budget.storage());
        assert!(
            matches!(original.check(budget), Err(ProductionSourceOwnedViewErrorV18::Binding(detail))
            if detail == *expected)
        );
        assert_eq!((budget.work(), budget.storage()), before);
    }
    result
}

pub(super) fn test_issued_commoned_native_stores_v18(
    original: &ProductionSourceCorrespondenceV18<'_>,
    optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    reached: &std::cell::Cell<bool>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<()> {
    let output = optimized.output_inventory(budget)?;
    let function = optimized_source_root_function_v18(original, optimized, 0, budget)?;
    let operations = &output.operations()[function.operations.clone()];
    assert_eq!(
        operations
            .iter()
            .filter(|row| matches!(row.operation.kind, OperationKind::Store { .. }))
            .count(),
        2
    );
    original.with_pending_global_accesses_v18(optimized, 0, budget, |source, budget| {
        with_native_global_test_view_v18(optimized, budget, |native, budget| {
            let mut stores = 0;
            assert_eq!(operations.len(), source.roles.rows.len());
            for (actual, row) in operations.iter().zip(source.roles.rows) {
                assert_eq!(actual.coordinate, row.output);
                let writing = matches!(actual.operation.kind, OperationKind::Store { .. });
                let floor = budget.storage();
                source
                    .with_native_access_v18(native, actual.coordinate, budget, |view, _| {
                        assert_eq!(view.is_some(), writing);
                        if let Some(view) = view {
                            assert!(view.pair.output.writing);
                            assert!(!view.grants_memory_or_launch_authority());
                            assert_eq!(
                                view.pair.output.logical.access.operation,
                                actual.coordinate
                            );
                            assert_eq!(
                                view.pair.output.logical.root,
                                SliceDefinition::FunctionArgument {
                                    function: function.coordinate,
                                    argument: 0,
                                }
                            );
                            stores += 1;
                        }
                        Ok(())
                    })
                    .unwrap();
                assert_eq!(budget.storage(), floor);
            }
            assert_eq!(stores, 2);
            reached.set(true);
        })
    })
}

pub(super) fn test_issued_metadata_closed_consumer_v18(
    original: &ProductionSourceCorrespondenceV18<'_>,
    optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    mode: u8,
    reached: &std::cell::Cell<bool>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<()> {
    let floor = budget.storage();
    let result = source_scalar_normalization_scratch_v18(
        original.source.cleanup,
        budget,
        0,
        |budget| {
            original.source.retain_construction(|| {
                budget.reserve_storage(issued_output_headers_v18()?)?;
                let rows = scoped_raw_admission_v29::checked_issued_source_rows_v18(
                    original, 0, budget)?.unwrap();
                let function = optimized_source_root_function_v18(original, optimized, 0, budget)?;
                let actual = SourceIssuedActualV29::from_function(function.function, budget)
                    .map_err(source_emission_error_v18)?;
                let first = scoped_raw_admission_v29::source_issued_tail_locations_v18(
                    original, 0, &rows.issuers[0], budget)?;
                let anchor = issued_output_issuer_v18(original, optimized, 0,
                    &rows.issuers[0], function, &actual, budget)?.expect("first authentic issuer");
                if mode <= 1 {
                    assert_eq!(rows.issuers.len(), 2);
                    let second = scoped_raw_admission_v29::source_issued_tail_locations_v18(
                        original, 0, &rows.issuers[1], budget)?;
                    for (ordinal, input) in second.into_iter().enumerate() {
                        let disposition = optimized.operation(input, budget)?;
                        assert!(if mode == 0 {
                            matches!(disposition, ProductionOptimizedSourceOperationV18::RemovedUnreachable { input: found } if found == input)
                        } else if ordinal != 3 {
                            matches!(disposition, ProductionOptimizedSourceOperationV18::Rewritten { input: found } if found == input)
                        } else {
                            matches!(disposition, ProductionOptimizedSourceOperationV18::Retained { input: found, .. } if found == input)
                        }, "mode={mode}, input={input:?}, disposition={disposition:?}");
                    }
                    for (ordinal, kind, consumer) in [(0, IssuedMetadataKindV18::Length, 1),
                        (2, IssuedMetadataKindV18::Data, 3)] {
                        if mode == 1 && consumer == 3 {
                            let ProductionOptimizedSourceOperationV18::Retained { output, .. } =
                                optimized.operation(second[consumer], budget)? else { unreachable!() };
                            assert_ne!(output, anchor.output[consumer]);
                            assert_eq!(issued_metadata_output_v18(original, optimized, kind,
                                second[ordinal], second[consumer], output, budget)?,
                                Some((anchor.output[ordinal], true)),
                                "retained GEP still selects its authentic commoned data");
                        } else {
                            assert!(issued_metadata_output_v18(original, optimized, kind,
                                second[ordinal], second[consumer], anchor.output[consumer], budget)?.is_none(),
                                "a nonretained consumer cannot select a metadata occurrence");
                        }
                    }
                    assert!(issued_output_issuer_v18(original, optimized, 0,
                        &rows.issuers[1], function, &actual, budget)?.is_none());
                    let input_function = original.source.root(0, budget)?.1;
                    let input_stores = original.inventory.functions()[input_function].function.body
                        .as_ref().unwrap().blocks.iter().flat_map(|block| &block.operations)
                        .filter(|operation| matches!(operation.kind, OperationKind::Store { .. })).count();
                    let output_stores = function.function.body.as_ref().unwrap().blocks.iter()
                        .flat_map(|block| &block.operations)
                        .filter(|operation| matches!(operation.kind, OperationKind::Store { .. })).count();
                    assert_eq!(input_stores, 2, "both actual original Stores exist");
                    assert_eq!(output_stores, if mode == 0 { 1 } else { 2 });
                    reached.set(true);
                    return Ok(());
                }
                assert_eq!(rows.issuers.len(), 1);
                let foreign_rows = scoped_raw_admission_v29::checked_issued_source_rows_v18(
                    original, 1, budget)?.unwrap();
                assert_eq!(foreign_rows.issuers.len(), 1);
                let foreign_function = optimized_source_root_function_v18(original, optimized, 1, budget)?;
                assert_ne!(function.coordinate, foreign_function.coordinate);
                let foreign_actual = SourceIssuedActualV29::from_function(foreign_function.function, budget)
                    .map_err(source_emission_error_v18)?;
                let foreign = issued_output_issuer_v18(original, optimized, 1,
                    &foreign_rows.issuers[0], foreign_function, &foreign_actual, budget)?
                    .expect("second genuine root issuer");
                let output = optimized.output_inventory(budget)?;
                let a = optimized_source_definition_row_v18(output,
                    SliceDefinition::Result { operation: anchor.output[0], result: 0 }, budget)?;
                let b = optimized_source_definition_row_v18(output,
                    SliceDefinition::Result { operation: foreign.output[0], result: 0 }, budget)?;
                assert_eq!(a.ty, b.ty);
                assert_ne!(a.coordinate, b.coordinate);
                let (refused, expected) = if mode == 2 {
                    (issued_metadata_output_v18(original, optimized, IssuedMetadataKindV18::Length,
                        first[0], first[1], foreign.output[1], budget).map(|_| ()),
                        "issued metadata changed retained consumer")
                } else {
                    assert_eq!(mode, 3);
                    (issued_metadata_descendant_v18(original, optimized,
                        SliceDefinition::Result { operation: first[0], result: 0 },
                        SliceDefinition::Result { operation: foreign.output[0], result: 0 }, None, budget),
                        "issued metadata selected definition has no checked descendant")
                };
                let error = refused.unwrap_err();
                assert!(matches!(error, ProductionSourceOwnedViewErrorV18::Binding(detail) if detail == expected),
                    "mode={mode}: {error:?}");
                reached.set(true);
                Err(error)
            })
        },
    );
    assert_eq!(budget.storage(), floor);
    if let Err(ProductionSourceOwnedViewErrorV18::Binding(expected)) = result.as_ref() {
        let before = (budget.work(), budget.storage());
        assert!(
            matches!(original.check(budget), Err(ProductionSourceOwnedViewErrorV18::Binding(detail))
            if detail == *expected)
        );
        assert_eq!((budget.work(), budget.storage()), before);
    }
    result
}
