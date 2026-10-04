// Metadata-only source queries are not necessarily dependencies of a Load or
// Store. Complete them only under the genuine full parameter census, using the
// archived original assignment and the actual retained occurrence relation.
#[cfg(test)]
include!("production_source_descriptor_length_checks_v30_tests.rs");

#[derive(Clone, Copy)]
enum DescriptorLengthSourceV76<'a> {
    Assignment(&'a SourceRvalueRowV30),
    Call(&'a PendingSourceLengthV76),
}

fn descriptor_length_assignment_v30<'a>(
    original: &'a ProductionSourceCorrespondenceV18<'_>,
    root: usize,
    locator: &SourceRvalueRowV30,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<&'a fe2o3_mir_model::semantic_mir_v1::SemanticAssignmentV1> {
    let (row, assignment) = original.assignment_result_row_v30(
        root,
        locator.instance,
        SemanticBlockIdV1::from_index(locator.block),
        locator.statement,
        budget,
    )?;
    if !std::ptr::eq(row, locator) {
        return original.source.missing("descriptor length locator differs");
    }
    Ok(assignment)
}
fn descriptor_length_headers_v30() -> Result<usize, ArgumentResourceV1> {
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
    argument_sum_v1(&[
        size_of::<Frame<'_>>(),
        std::mem::align_of::<Frame<'_>>(),
        source_descriptor_origin_headers_v30()?,
        scoped_raw_admission_v29::source_length_replay_headers_v76()?,
        size_of::<DescriptorLengthSourceV76<'_>>(),
        size_of::<std::slice::Iter<'_, PendingSourceLengthV76>>(),
        size_of::<&[PendingSourceLengthV76]>(),
        argument_product_v1(2, size_of::<fn()>())?,
        argument_product_v1(3, size_of::<&()>())?,
    ])
}

fn descriptor_length_source_scalar_v30(
    original: &ProductionSourceCorrespondenceV18<'_>,
    assignment: &fe2o3_mir_model::semantic_mir_v1::SemanticAssignmentV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<Option<ScalarType>> {
    let types = original.source.source_semantic(budget)?.types();
    budget.charge_work(6)?;
    let slice_type = match assignment.value().kind() {
        SemanticRvalueKindV1::Length(place) => place.ty(),
        SemanticRvalueKindV1::Unary {
            operation: SemanticUnaryOpV1::PointerMetadata,
            operand: SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place),
        } => {
            let Some(SemanticTypeShapeV1::Pointer(pointer)) = types
                .get(place.ty().index() as usize)
                .map(SemanticTypeDeclV1::shape)
            else {
                return Ok(None);
            };
            if pointer.kind() != SemanticPointerKindV1::Reference
                || pointer.metadata() != SemanticPointerMetadataV1::SliceLength
                || pointer.pointer_width_bits() != 64
            {
                return Ok(None);
            }
            pointer.pointee()
        }
        _ => return Ok(None),
    };
    let Some(SemanticTypeShapeV1::Slice { element }) = types
        .get(slice_type.index() as usize)
        .map(SemanticTypeDeclV1::shape)
    else {
        return Ok(None);
    };
    if lower_scalar_type(types, assignment.value().result_type())
        .map_err(source_emission_error_v18)?
        != Type::Scalar(ScalarType::U64)
    {
        return original
            .source
            .missing("descriptor length semantic result type differs");
    }
    let Type::Scalar(scalar) =
        lower_scalar_type(types, *element).map_err(source_emission_error_v18)?
    else {
        return Ok(None);
    };
    Ok(Some(scalar))
}

fn descriptor_length_operation_v30(
    original: &ProductionSourceCorrespondenceV18<'_>,
    inventory: &fe2o3_kernel_analysis::CanonicalKirInventoryV18<'_>,
    definition: SliceDefinition,
    scalar: ScalarType,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<(SliceOperation, ValueId)> {
    let SliceDefinition::Result {
        operation,
        result: 0,
    } = definition
    else {
        return original
            .source
            .missing("descriptor length has no exact operation result");
    };
    let row = source_operation_row_v18(inventory, operation, budget)?;
    let OperationKind::SliceLength { slice } = row.operation.kind else {
        return original
            .source
            .missing("descriptor length original or output opcode differs");
    };
    let [result] = row.operation.results.as_slice() else {
        return original
            .source
            .missing("descriptor length result census differs");
    };
    let receiver = inventory
        .definition_for_value(operation.block.function, slice, budget)
        .map_err(source_pointer_inventory_error_v18)?
        .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
            "descriptor length receiver is absent",
        ))?;
    let Type::Slice(slice_type) = receiver.ty else {
        return original
            .source
            .missing("descriptor length receiver is not a slice");
    };
    if result.ty != Type::INDEX || slice_type.element.as_ref() != &Type::Scalar(scalar) {
        return original
            .source
            .missing("descriptor length result or element type differs");
    }
    Ok((operation, slice))
}

fn complete_source_descriptor_lengths_v30(
    original: &ProductionSourceCorrespondenceV18<'_>,
    optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    root: usize,
    completion: &SourceSliceRootCompletionV25<'_, '_, '_>,
    operations: &mut [Option<CompletedGlobalOperationV26>],
    operation_count: &mut usize,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<()> {
    original.global_expression_entry_v23(optimized, budget)?;
    if !std::ptr::eq(completion.source.original(), original)
        || !std::ptr::eq(completion.source.source.optimized(), optimized)
        || completion.source.root() != root
    {
        return original
            .source
            .missing("descriptor length completion owner differs");
    }
    let headers = original.retain_query(descriptor_length_headers_v30().map_err(Into::into))?;
    let result = source_scalar_normalization_scratch_v18(
        original.source.cleanup,
        budget,
        headers,
        |budget| {
            let result = (|| {
                let input_function = original
                    .inventory
                    .functions()
                    .get(original.source.root(root, budget)?.1)
                    .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                        "descriptor length original root is absent",
                    ))?;
                let output_function =
                    optimized_source_root_function_v18(original, optimized, root, budget)?;
                let output = optimized.output_inventory(budget)?;
                if operations.len() != output.operations().len() {
                    return original
                        .source
                        .missing("descriptor length output census differs");
                }
                let owner = original.source.root_row(root)?;
                let roster = owner.rvalue_results.as_ref().ok_or(
                    ProductionSourceOwnedViewErrorV18::Binding(
                        "original assignment result roster is absent",
                    ),
                )?;
                let lengths =
                    scoped_raw_admission_v29::checked_source_lengths_v76(original, root, budget)?;
                super::value_origin_v1::with_whole_value_origins_v18(
                    original,
                    input_function.coordinate,
                    budget,
                    |before, budget| {
                        let result = super::value_origin_v1::with_optimized_whole_value_origins_v18(
                            original,
                            optimized,
                            output,
                            output_function.coordinate,
                            budget,
                            |after, budget| {
                                let result = (|| {
                                    for source in roster
                                        .rows
                                        .iter()
                                        .map(DescriptorLengthSourceV76::Assignment)
                                        .chain(lengths.iter().map(DescriptorLengthSourceV76::Call))
                                    {
                                        budget.charge_work(4)?;
                                        let (value, scalar) = match source {
                                            DescriptorLengthSourceV76::Assignment(locator) => {
                                                let assignment = descriptor_length_assignment_v30(
                                                    original, root, locator, budget,
                                                )?;
                                                let Some(scalar) =
                                                    descriptor_length_source_scalar_v30(
                                                        original, assignment, budget,
                                                    )?
                                                else {
                                                    continue;
                                                };
                                                let SourceRvalueEndpointV30::Scalar {
                                                    value,
                                                    scalar: ScalarType::Index,
                                                } = locator.endpoint
                                                else {
                                                    return original.source.missing(
                                                "descriptor length archived representation differs",
                                            );
                                                };
                                                (value, scalar)
                                            }
                                            DescriptorLengthSourceV76::Call(row) => {
                                                (row.length, row.element)
                                            }
                                        };
                                        let definition = original
                                            .inventory
                                            .definition_for_value(
                                                input_function.coordinate,
                                                value,
                                                budget,
                                            )
                                            .map_err(source_pointer_inventory_error_v18)?
                                            .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                                                "descriptor length original definition is absent",
                                            ))?;
                                        let (input_operation, input_slice) =
                                            descriptor_length_operation_v30(
                                                original,
                                                original.inventory,
                                                definition.coordinate,
                                                scalar,
                                                budget,
                                            )?;
                                        match source {
                                            DescriptorLengthSourceV76::Assignment(locator) => {
                                                original.check_descriptor_operand_v30(
                                                    root,
                                                    locator,
                                                    input_slice,
                                                    budget,
                                                )?
                                            }
                                            DescriptorLengthSourceV76::Call(row) => {
                                                budget.charge_work(2)?;
                                                if input_slice != row.receiver {
                                                    return original.source.missing(
                                                        "source length call receiver differs",
                                                    );
                                                }
                                            }
                                        }
                                        let ProductionOptimizedSourceOperationV18::Retained {
                                            output: output_operation,
                                            ..
                                        } = optimized.operation(input_operation, budget)?
                                        else {
                                            continue;
                                        };
                                        let index = completed_global_index_v26(
                                            output,
                                            output_operation,
                                            budget,
                                        )?;
                                        if operations[index]
                                            == Some(CompletedGlobalOperationV26::Length)
                                            && matches!(
                                                source,
                                                DescriptorLengthSourceV76::Assignment(_)
                                            )
                                        {
                                            continue;
                                        }
                                        let output_definition = SliceDefinition::Result {
                                            operation: output_operation,
                                            result: 0,
                                        };
                                        let (_, output_slice) = descriptor_length_operation_v30(
                                            original,
                                            output,
                                            output_definition,
                                            scalar,
                                            budget,
                                        )?;
                                        issued_output_definition_v18(
                                            original,
                                            optimized,
                                            definition.coordinate,
                                            output_definition,
                                            budget,
                                        )?;
                                        issued_output_operand_v18(
                                            original,
                                            optimized,
                                            input_operation,
                                            output_operation,
                                            0,
                                            output_slice,
                                            budget,
                                        )?;
                                        let DescriptorOriginV30::Exact(
                                            input_parameter @ SliceDefinition::FunctionArgument {
                                                function: input_owner,
                                                ..
                                            },
                                        ) = source_descriptor_origin_v30(
                                            original.inventory,
                                            input_function,
                                            before,
                                            input_slice,
                                            budget,
                                        )
                                        .map_err(source_emission_error_v18)?
                                        else {
                                            return original.source.missing(
                                                "descriptor length original parameter is not exact",
                                            );
                                        };
                                        let DescriptorOriginV30::Exact(
                                            output_parameter @ SliceDefinition::FunctionArgument {
                                                function: output_owner,
                                                ..
                                            },
                                        ) = source_descriptor_origin_v30(
                                            output,
                                            output_function,
                                            after,
                                            output_slice,
                                            budget,
                                        )
                                        .map_err(source_emission_error_v18)?
                                        else {
                                            return original.source.missing(
                                                "descriptor length output parameter is not exact",
                                            );
                                        };
                                        if input_owner != input_function.coordinate
                                            || output_owner != output_function.coordinate
                                        {
                                            return original.source.missing(
                                            "descriptor length parameter belongs to another root",
                                        );
                                        }
                                        if let DescriptorLengthSourceV76::Call(row) = source {
                                            budget.charge_work(3)?;
                                            if input_parameter
                                                != (SliceDefinition::FunctionArgument {
                                                    function: input_owner,
                                                    argument: u32::try_from(row.root_parameter)
                                                        .map_err(|_| {
                                                            ArgumentResourceV1::Arithmetic
                                                        })?,
                                                })
                                            {
                                                return original.source.missing(
                                                    "source length call root parameter differs",
                                                );
                                            }
                                        }
                                        issued_output_definition_v18(
                                            original,
                                            optimized,
                                            input_parameter,
                                            output_parameter,
                                            budget,
                                        )?;
                                        let mut matches = 0usize;
                                        for premise in completion.arguments.iter().flatten() {
                                            budget.charge_work(2)?;
                                            if premise.parameter == output_parameter
                                                && premise.scalar == scalar
                                            {
                                                matches += 1;
                                            }
                                        }
                                        if matches != 1 {
                                            return original.source.missing(
                                            "descriptor length lacks one admitted source parameter",
                                        );
                                        }
                                        match operations
                                            .get_mut(index)
                                            .ok_or(ArgumentResourceV1::Accounting)?
                                        {
                                            slot @ None => {
                                                *slot = Some(CompletedGlobalOperationV26::Length);
                                                *operation_count =
                                                    operation_count
                                                        .checked_add(1)
                                                        .ok_or(ArgumentResourceV1::Arithmetic)?;
                                            }
                                            Some(CompletedGlobalOperationV26::Length) => {}
                                            Some(_) => {
                                                return original.source.missing(
                                                    "descriptor length completion role conflicts",
                                                );
                                            }
                                        }
                                    }
                                    Ok(())
                                })();
                                original.source.retain_aggregate_source_result_v30(result)
                            },
                        );
                        original.source.retain_aggregate_source_result_v30(result)
                    },
                )
            })();
            original.source.retain_aggregate_source_result_v30(result)
        },
    );
    original.retain_query(result)
}
