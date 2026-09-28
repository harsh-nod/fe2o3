// Commoned pure metadata is selected by its retained consumer, never by shape.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum IssuedMetadataKindV18 {
    Length,
    Data,
}

fn issued_metadata_descendant_v18(
    original: &ProductionSourceCorrespondenceV18<'_>,
    optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    input: SliceDefinition,
    output: SliceDefinition,
    kind: Option<fe2o3_kernel_ir::CanonicalKirDefinitionDescendantKindV1>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<()> {
    let mut found = false;
    for row in optimized.definition_descendants(input, budget)? {
        budget.charge_work(3)?;
        if row.output == output {
            if found || kind.is_some_and(|kind| row.kind != kind) {
                return original
                    .source
                    .missing("issued metadata descendant kind or uniqueness");
            }
            found = true;
        }
    }
    if !found {
        return original
            .source
            .missing("issued metadata selected definition has no checked descendant");
    }
    Ok(())
}

fn issued_metadata_output_v18(
    original: &ProductionSourceCorrespondenceV18<'_>,
    optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    kind: IssuedMetadataKindV18,
    input: SliceOperation,
    input_consumer: SliceOperation,
    output_consumer: SliceOperation,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<Option<(SliceOperation, bool)>> {
    use fe2o3_kernel_ir::CanonicalKirDefinitionDescendantKindV1 as Descendant;
    use fe2o3_kernel_ir::CanonicalKirUseCoordinateV1 as Usage;
    optimized.check_exact_original_v18(original, budget)?;
    // Fixed shallow schema/coordinate comparisons; indexed queries and every
    // descendant row are separately charged below.
    budget.charge_work(64)?;
    let input_row = optimized_source_operation_row_v18(original.inventory, input, budget)?;
    let input_operation = input_row.operation;
    let [input_result] = input_operation.results.as_slice() else {
        return original
            .source
            .missing("issued metadata original result census");
    };
    let input_receiver = match (kind, &input_operation.kind) {
        (IssuedMetadataKindV18::Length, OperationKind::SliceLength { slice })
        | (IssuedMetadataKindV18::Data, OperationKind::SliceData { slice }) => *slice,
        _ => return original.source.missing("issued metadata original opcode"),
    };
    let consumer = optimized_source_operation_row_v18(original.inventory, input_consumer, budget)?;
    let operand = match (kind, &consumer.operation.kind) {
        (
            IssuedMetadataKindV18::Length,
            OperationKind::Compare {
                predicate: ComparePredicate::LessThan,
                rhs,
                ..
            },
        ) if *rhs == input_result.id => 1,
        (IssuedMetadataKindV18::Data, OperationKind::GetElementPointer { base, .. })
            if *base == input_result.id =>
        {
            0
        }
        _ => {
            return original
                .source
                .missing("issued metadata changed original consuming use");
        }
    };
    let ProductionOptimizedSourceOperationV18::Retained {
        output: retained_consumer,
        ..
    } = optimized.operation(input_consumer, budget)?
    else {
        return Ok(None);
    };
    if retained_consumer != output_consumer {
        return original
            .source
            .missing("issued metadata changed retained consumer");
    }
    let disposition = optimized.operation(input, budget)?;
    let (retained, substituted) = match disposition {
        ProductionOptimizedSourceOperationV18::Retained { output, .. } => (Some(output), false),
        ProductionOptimizedSourceOperationV18::Rewritten { .. } => (None, true),
        ProductionOptimizedSourceOperationV18::RemovedUnreachable { .. } => return Ok(None),
    };
    let selected = optimized
        .operand(
            Usage::OperationOperand {
                operation: input_consumer,
                operand,
            },
            budget,
        )?
        .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
            "issued metadata consuming use removed",
        ))?;
    if selected.coordinate
        != (Usage::OperationOperand {
            operation: output_consumer,
            operand,
        })
    {
        return original
            .source
            .missing("issued metadata changed actual consuming use");
    }
    let SliceDefinition::Result {
        operation: output,
        result: 0,
    } = selected.definition
    else {
        return original
            .source
            .missing("issued metadata selected non-operation result");
    };
    if output.block.function != output_consumer.block.function
        || retained.is_some_and(|retained| retained != output)
    {
        return original
            .source
            .missing("issued metadata selected output owner or occurrence");
    }
    issued_metadata_descendant_v18(
        original,
        optimized,
        SliceDefinition::Result {
            operation: input,
            result: 0,
        },
        selected.definition,
        Some(if substituted {
            Descendant::Substituted
        } else {
            Descendant::Retained
        }),
        budget,
    )?;
    let inventory = optimized.output_inventory(budget)?;
    let output_row = optimized_source_operation_row_v18(inventory, output, budget)?;
    let operation = output_row.operation;
    let [result] = operation.results.as_slice() else {
        return original
            .source
            .missing("issued metadata selected result census");
    };
    let receiver = match (kind, &operation.kind) {
        (IssuedMetadataKindV18::Length, OperationKind::SliceLength { slice })
        | (IssuedMetadataKindV18::Data, OperationKind::SliceData { slice }) => *slice,
        _ => return original.source.missing("issued metadata selected opcode"),
    };
    let selected_definition =
        optimized_source_definition_row_v18(inventory, selected.definition, budget)?;
    if selected_definition.value != Some(result.id) || result.ty != input_result.ty {
        return original
            .source
            .missing("issued metadata selected value or type");
    }
    // An eliminated producer has no retained operand occurrence. Its exact
    // original receiver still needs an authenticated selected descendant.
    let input_receiver_definition = original
        .inventory
        .definition_for_value(input.block.function, input_receiver, budget)
        .map_err(source_pointer_inventory_error_v18)?
        .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
            "issued metadata original receiver",
        ))?;
    let receiver_definition = inventory
        .definition_for_value(output.block.function, receiver, budget)
        .map_err(source_pointer_inventory_error_v18)?
        .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
            "issued metadata output receiver",
        ))?;
    issued_metadata_descendant_v18(
        original,
        optimized,
        input_receiver_definition.coordinate,
        receiver_definition.coordinate,
        None,
        budget,
    )?;
    if !substituted {
        issued_output_operand_v18(original, optimized, input, output, 0, receiver, budget)?;
    }
    Ok(Some((output, substituted)))
}

fn issued_metadata_headers_v18() -> Result<usize, ArgumentResourceV1> {
    use fe2o3_kernel_analysis::{
        CanonicalKirDefinitionRefV1, CanonicalKirInventoryV18, CanonicalKirOperationRefV1,
    };
    use fe2o3_kernel_ir::CanonicalKirDefinitionDescendantKindV1 as Kind;
    type SelectFrame<'a> = (
        &'a ProductionSourceCorrespondenceV18<'a>,
        &'a ProductionOptimizedSourceCorrespondenceV18<'a>,
        IssuedMetadataKindV18,
        [SliceOperation; 3],
        &'a mut ArgumentBudgetV1<'a>,
    );
    type DescendantFrame<'a> = (
        &'a ProductionSourceCorrespondenceV18<'a>,
        &'a ProductionOptimizedSourceCorrespondenceV18<'a>,
        [SliceDefinition; 2],
        Option<Kind>,
        &'a mut ArgumentBudgetV1<'a>,
    );
    type LookupFrames<'a> = (
        (
            &'a CanonicalKirInventoryV18<'a>,
            SliceOperation,
            &'a mut ArgumentBudgetV1<'a>,
        ),
        (
            &'a CanonicalKirInventoryV18<'a>,
            SliceDefinition,
            &'a mut ArgumentBudgetV1<'a>,
        ),
        (
            &'a CanonicalKirInventoryV18<'a>,
            fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1,
            ValueId,
            &'a mut ArgumentBudgetV1<'a>,
        ),
    );
    type Locals<'a> = (
        [&'a CanonicalKirOperationRefV1<'a>; 3],
        [&'a fe2o3_kernel_ir::Operation; 2],
        [&'a fe2o3_kernel_ir::OperationKind; 3],
        [&'a [fe2o3_kernel_ir::ValueDef]; 2],
        [&'a fe2o3_kernel_ir::ValueDef; 2],
        [&'a ValueId; 3],
        [&'a CanonicalKirDefinitionRefV1<'a>; 3],
        &'a CanonicalKirInventoryV18<'a>,
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
        &'a [fe2o3_kernel_ir::CanonicalKirDefinitionDescendantV1],
        std::slice::Iter<'a, fe2o3_kernel_ir::CanonicalKirDefinitionDescendantV1>,
        &'a fe2o3_kernel_ir::CanonicalKirDefinitionDescendantV1,
        bool,
        Option<Kind>,
        Kind,
        (
            Kind,
            &'a fe2o3_kernel_ir::CanonicalKirDefinitionDescendantV1,
        ),
        (SliceOperation, &'a SliceOperation),
    );
    fn h<T>() -> Result<usize, ArgumentResourceV1> {
        argument_sum_v1(&[
            size_of::<T>(),
            argument_product_v1(2, size_of::<SourceOwnedResultV18<T>>())?,
        ])
    }
    argument_sum_v1(&[
        h::<SelectFrame<'_>>()?,
        h::<DescendantFrame<'_>>()?,
        h::<LookupFrames<'_>>()?,
        h::<Locals<'_>>()?,
        h::<Option<(SliceOperation, bool)>>()?,
        h::<&CanonicalKirOperationRefV1<'_>>()?,
        h::<&CanonicalKirDefinitionRefV1<'_>>()?,
        h::<Option<&CanonicalKirDefinitionRefV1<'_>>>()?,
        size_of::<
            Result<
                Option<&CanonicalKirDefinitionRefV1<'_>>,
                fe2o3_kernel_analysis::CanonicalKirInventoryErrorV1,
            >,
        >(),
        h::<&[fe2o3_kernel_ir::CanonicalKirDefinitionDescendantV1]>()?,
        h::<Option<fe2o3_kernel_analysis::CanonicalKirOutputUseV1>>()?,
        h::<ProductionOptimizedSourceOperationV18>()?,
        h::<()>()?,
        h::<Result<(), ArgumentResourceV1>>()?,
        argument_product_v1(2, h::<&Type>()?)?,
        h::<[bool; 4]>()?,
        h::<[SliceOperation; 4]>()?,
        h::<[usize; 2]>()?,
        h::<std::array::IntoIter<usize, 2>>()?,
        h::<Option<usize>>()?,
        h::<usize>()?,
        h::<[(usize, IssuedMetadataKindV18, usize); 2]>()?,
        h::<(usize, IssuedMetadataKindV18, usize)>()?,
        h::<Option<(usize, IssuedMetadataKindV18, usize)>>()?,
        h::<std::array::IntoIter<(usize, IssuedMetadataKindV18, usize), 2>>()?,
    ])
}

#[cfg(test)]
include!("production_optimized_source_issued_metadata_v18_tests.rs");
