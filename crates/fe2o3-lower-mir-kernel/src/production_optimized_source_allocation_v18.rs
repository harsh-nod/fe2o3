fn optimized_source_input_operation_v18(
    original: &ProductionSourceCorrespondenceV18<'_>,
    root: usize,
    block: BlockId,
    operation: usize,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1> {
    original.query(budget)?;
    let owner = original.source.root_row(root)?;
    let function = fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1(
        u32::try_from(owner.function_ordinal).map_err(|_| ArgumentResourceV1::Arithmetic)?,
    );
    let block_row = original
        .inventory
        .block_for_id(function, block, budget)
        .map_err(|error| {
            ProductionSourceOwnedViewErrorV18::from(
                fe2o3_pliron::CanonicalAnalysisScopeErrorV1::Inventory(error),
            )
        })?
        .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
            "optimized original allocation block",
        ))?;
    let operation = immutable_memory_gap_v29(owner, block, operation, false, budget)?;
    let coordinate = fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1 {
        block: block_row.coordinate,
        operation: u32::try_from(operation).map_err(|_| ArgumentResourceV1::Arithmetic)?,
    };
    optimized_source_operation_row_v18(original.inventory, coordinate, budget)?;
    Ok(coordinate)
}

// Slot locations are local to their original invocation. The source attachment
// producer already accounts for expansion, relocation and synthetic insertions.
pub(super) fn optimized_source_slot_input_v18(
    original: &ProductionSourceCorrespondenceV18<'_>,
    root: usize,
    slot_ordinal: usize,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1> {
    original.retain_query((|| {
        original.query(budget)?;
        let owner = original.source.root_row(root)?;
        budget.charge_work(3)?;
        let slot = owner.source_slots.slots.get(slot_ordinal).ok_or(
            ProductionSourceOwnedViewErrorV18::Binding("optimized original slot ordinal"),
        )?;
        let instance = slot.instance.index();
        let active = original.source.active_ordinal(root, instance, budget)?.ok_or(
            ProductionSourceOwnedViewErrorV18::Binding("optimized original slot inactive instance"),
        )?;
        let instance_slots = owner.source_slots.instances.get(active).ok_or(
            ProductionSourceOwnedViewErrorV18::Binding("optimized original slot instance interval"),
        )?;
        if instance_slots.instance != slot.instance || !instance_slots.slots.contains(&slot_ordinal) {
            return original.source.missing("optimized original slot changed instance interval");
        }
        let sidecar = original.source.sidecar(root, instance, budget)?;
        let first = sidecar.scoped_slot_origins.as_ref().map_or(0, |rows| rows.len());
        let row = argument_sum_v1(&[first, slot_ordinal - instance_slots.slots.start])?;
        let key = TileAttachmentKeyV29 {
            root, family: TileAttachmentFamilyV29::SourceSlot, instance, row,
            field: TileAttachmentFieldV29::SlotAllocation, component: 0, part: 0,
        };
        let [attachment] = original.attachment_range(key, budget)? else {
            return original.source.missing("optimized original slot allocation attachment census");
        };
        if source_attachment_key_v18(attachment.key) != source_attachment_key_v18(key) {
            return original.source.missing("optimized original slot allocation attachment part");
        }
        let TileAttachmentLocationV29::Origin(TileScalarSourceV29::Operation(point)) = attachment.location else {
            return original.source.missing("optimized original slot allocation attachment role");
        };
        if point.function != owner.function_ordinal {
            return original.source.missing("optimized original slot allocation root");
        }
        let input = fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1 {
            block: fe2o3_kernel_ir::CanonicalKirBlockCoordinateV1 {
                function: fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1(
                    u32::try_from(point.function).map_err(|_| ArgumentResourceV1::Arithmetic)?),
                block: u32::try_from(point.block).map_err(|_| ArgumentResourceV1::Arithmetic)?,
            },
            operation: u32::try_from(point.operation).map_err(|_| ArgumentResourceV1::Arithmetic)?,
        };
        let actual = optimized_source_operation_row_v18(original.inventory, input, budget)?.operation;
        if !matches!(actual.kind, OperationKind::Alloca { address_space: AddressSpace::Private, .. })
            || !matches!(actual.results.as_slice(), [result] if result.id == slot.origin.pointer)
        {
            return original.source.missing("optimized original slot allocation changed backing");
        }
        Ok(input)
    })())
}

// The copied rows are hypotheses for the unchanged physical solver. Their
// source identities and shape stay original; only exactly checked physical
// occurrences change. The original slot table is never mutated or rebound.
fn optimized_source_slots_v18(
    original: &CheckedSourceMemoryV29<'_>,
    optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<Vec<ScopedSourceSlotV29>> {
    original.check(budget)?;
    let relation = original.correspondence;
    optimized_source_endpoints_v18(relation, optimized, budget)?;
    let owner = relation.source.root_row(original.root)?;
    let output = optimized.output_inventory(budget)?;
    let output_function =
        optimized_source_root_function_v18(relation, optimized, original.root, budget)?;
    let headers = optimized_source_slot_transport_headers_v18()?;
    budget.reserve_storage(headers)?;
    let mut slots = emission_vec_v1(owner.source_slots.slots.len(), budget)
        .map_err(immutable_memory_error_v29)?;
    for (ordinal, slot) in owner.source_slots.slots.iter().enumerate() {
        budget.charge_work(2)?;
        let input = optimized_source_slot_input_v18(relation, original.root, ordinal, budget)?;
        let allocation = optimized.allocation(original.root, input, budget)?.ok_or(
            ProductionSourceOwnedViewErrorV18::Binding("optimized allocation lost original slot"),
        )?;
        if allocation.instance() != slot.instance.index() {
            return relation
                .source
                .missing("optimized allocation changed original instance");
        }
        let operation = allocation
            .output()
            .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                "removed source allocation needs explicit slot-domain transport",
            ))?;
        if operation.block.function != output_function.coordinate {
            return relation
                .source
                .missing("optimized allocation changed output function");
        }
        let pointer = allocation
            .pointer()
            .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                "optimized allocation missing actual pointer",
            ))?;
        let pointer = optimized_source_definition_row_v18(output, pointer, budget)?
            .value
            .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                "optimized allocation has no actual result",
            ))?;
        let block = optimized_source_block_row_v18(output, operation.block, budget)?
            .block
            .id;
        let mut actual = *slot;
        actual.origin.pointer = pointer;
        actual.allocation = PrivateArrayPhysicalLocationV1 {
            block_ordinal: operation.block.block as usize,
            block,
            operation: operation.operation as usize,
        };
        actual.representation = match slot.representation {
            ScopedSlotRepresentationV29::ScalarArray(mut scalar) => {
                scalar.count = optimized_source_scalar_count_v18(
                    relation, output, scalar, allocation.count(), budget,
                )?;
                ScopedSlotRepresentationV29::ScalarArray(scalar)
            }
            ScopedSlotRepresentationV29::Object { .. } => {
                let actual_operation = optimized_source_operation_row_v18(output, operation, budget)?;
                optimized_source_object_representation_v18(slot.representation,
                    actual_operation.operation, pointer, allocation.count().is_some(), budget)?
            }
        };
        slots.push(actual);
    }
    budget.release_storage(headers)?;
    Ok(slots)
}

// Closed representation checking only. Original allocation/result custody is
// established by the caller before this inert copied representation is used.
fn optimized_source_object_representation_v18(
    original: ScopedSlotRepresentationV29,
    operation: &Operation,
    pointer: ValueId,
    has_count: bool,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<ScopedSlotRepresentationV29> {
    budget.charge_work(1)?;
    let ScopedSlotRepresentationV29::Object { schema, alignment, .. } = original else {
        return Err(ProductionSourceOwnedViewErrorV18::Binding("scalar slot is not an object representation"));
    };
    if has_count {
        return Err(ProductionSourceOwnedViewErrorV18::Binding("optimized object allocation acquired a count"));
    }
    check_scoped_object_alloca_v29(operation, pointer, schema, alignment, budget)
        .map_err(immutable_memory_error_v29)?;
    Ok(original)
}

fn optimized_source_slot_transport_headers_v18() -> Result<usize, ArgumentResourceV1> {
    argument_sum_v1(&[
        std::mem::size_of::<ScopedSlotRepresentationV29>(),
        std::mem::size_of::<ScopedScalarArraySlotV29>(),
        std::mem::size_of::<Option<fe2o3_kernel_analysis::CanonicalKirOutputUseV1>>(),
        std::mem::size_of::<SourceOwnedResultV18<Option<(ValueId, PrivateArrayPhysicalLocationV1)>>>(),
        std::mem::size_of::<Result<(), ProductionSemanticKirErrorV1>>(),
        std::mem::size_of::<SourceOwnedResultV18<ScopedSlotRepresentationV29>>(),
    ])
}

#[cfg(test)]
mod optimized_source_c1_transport_tests {
    use super::*;

    #[test]
    fn copied_object_recipe_checks_count_schema_alignment_and_actual_pointer() {
        // Primitive representation testing only, not prepared-source C2 proof.
        let schema = fe2o3_kernel_ir::StorageLayoutIdV1(17);
        let pointer = ValueId(91);
        let representation = ScopedSlotRepresentationV29::Object { schema, bytes: 48, alignment: 16 };
        for fault in 0..6 {
            let mut operation = Operation::effect_free(
                ValueDef::new(pointer, Type::pointer(Type::StorageObject(schema),
                    AddressSpace::Private, AccessMode::ReadWrite)),
                OperationKind::Alloca { element: Type::StorageObject(schema), count: None,
                    address_space: AddressSpace::Private, alignment: 16 });
            match fault {
                0 | 1 => {},
                2 => if let OperationKind::Alloca { count, .. } = &mut operation.kind { *count = Some(ValueId(2)); },
                3 => if let OperationKind::Alloca { element, .. } = &mut operation.kind { *element = Type::StorageObject(fe2o3_kernel_ir::StorageLayoutIdV1(18)); },
                4 => if let OperationKind::Alloca { alignment, .. } = &mut operation.kind { *alignment = 8; },
                5 => operation.results[0].id = ValueId(92),
                _ => unreachable!(),
            }
            let mut work = CanonicalKernelIrWorkBudgetV1::new(10);
            let mut budget = ArgumentBudgetV1::new(&mut work, 73);
            budget.reserve_storage(73).unwrap();
            let result = optimized_source_object_representation_v18(
                representation, &operation, pointer, fault == 1, &mut budget);
            match result {
                Ok(actual) => { assert_eq!(fault, 0); assert_eq!(actual, representation); },
                Err(_) => assert_ne!(fault, 0),
            }
            assert_eq!(budget.work(), if fault == 1 { 1 } else { 10 });
            assert_eq!(budget.storage(), 73);
        }
    }

    #[test]
    fn slot_transport_transient_headers_have_an_independent_exact_and_short_oracle() {
        let expected = std::mem::size_of::<ScopedSlotRepresentationV29>()
            + std::mem::size_of::<ScopedScalarArraySlotV29>()
            + std::mem::size_of::<Option<fe2o3_kernel_analysis::CanonicalKirOutputUseV1>>()
            + std::mem::size_of::<SourceOwnedResultV18<Option<(ValueId, PrivateArrayPhysicalLocationV1)>>>()
            + std::mem::size_of::<Result<(), ProductionSemanticKirErrorV1>>()
            + std::mem::size_of::<SourceOwnedResultV18<ScopedSlotRepresentationV29>>();
        assert_eq!(optimized_source_slot_transport_headers_v18().unwrap(), expected);
        for limit in [expected, expected - 1] {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(0);
            let mut budget = ArgumentBudgetV1::new(&mut work, limit);
            let result = budget.reserve_storage(optimized_source_slot_transport_headers_v18().unwrap());
            assert_eq!(result.is_ok(), limit == expected);
            if result.is_err() {
                assert_eq!(budget.failed_storage(), Some(expected));
                assert!(matches!(result, Err(ArgumentResourceV1::Storage(error))
                    if (error.actual(), error.limit()) == (expected, limit)));
                assert_eq!(budget.storage(), 0);
            }
            assert_eq!(budget.work(), 0);
        }
    }
}

// The scalar arm retains the old count obligation. Object storage never enters
// this reader, and a runtime count is not replaced with a static source length.
fn optimized_source_scalar_count_v18(
    relation: &ProductionSourceCorrespondenceV18<'_>,
    output: &fe2o3_kernel_analysis::CanonicalKirInventoryV18<'_>,
    scalar: ScopedScalarArraySlotV29,
    count: Option<fe2o3_kernel_analysis::CanonicalKirOutputUseV1>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<Option<(ValueId, PrivateArrayPhysicalLocationV1)>> {
    use fe2o3_kernel_ir::CanonicalKirDefinitionCoordinateV1 as Definition;
    Ok(match (scalar.count, count) {
            (None, None) => None,
            (Some(_), Some(count)) => {
                let value = optimized_source_definition_row_v18(output, count.definition, budget)?
                    .value
                    .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                        "optimized allocation count has no value",
                    ))?;
                // Existing source slots own static extents. A genuine runtime
                // count needs its own extent obligation, not a guessed length.
                let Definition::Result {
                    operation: count_operation,
                    result: 0,
                } = count.definition
                else {
                    return relation
                        .source
                        .missing("runtime allocation extent remains pending");
                };
                let count_row =
                    optimized_source_operation_row_v18(output, count_operation, budget)?;
                if !matches!(count_row.operation.kind, OperationKind::Constant(Constant::Index(length)) if length == scalar.length)
                {
                    return relation
                        .source
                        .missing("runtime or changed allocation extent remains pending");
                }
                let block = optimized_source_block_row_v18(output, count_operation.block, budget)?
                    .block
                    .id;
                Some((
                    value,
                    PrivateArrayPhysicalLocationV1 {
                        block_ordinal: count_operation.block.block as usize,
                        block,
                        operation: count_operation.operation as usize,
                    },
                ))
            }
            (None, Some(_)) | (Some(_), None) => {
                return relation
                    .source
                    .missing("optimized allocation count shape differs");
            }
        })
}
