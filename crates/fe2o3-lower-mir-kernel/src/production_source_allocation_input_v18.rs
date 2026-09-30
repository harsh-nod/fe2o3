fn source_input_operation_v18(
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
    source_operation_row_v18(original.inventory, coordinate, budget)?;
    Ok(coordinate)
}

// Slot locations are local to their original invocation. The source attachment
// producer already accounts for expansion, relocation and synthetic insertions.
pub(super) fn source_slot_input_v18(
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
        let active = original
            .source
            .active_ordinal(root, instance, budget)?
            .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                "optimized original slot inactive instance",
            ))?;
        let instance_slots = owner.source_slots.instances.get(active).ok_or(
            ProductionSourceOwnedViewErrorV18::Binding("optimized original slot instance interval"),
        )?;
        if instance_slots.instance != slot.instance || !instance_slots.slots.contains(&slot_ordinal)
        {
            return original
                .source
                .missing("optimized original slot changed instance interval");
        }
        let sidecar = original.source.sidecar(root, instance, budget)?;
        let first = sidecar
            .scoped_slot_origins
            .as_ref()
            .map_or(0, |rows| rows.len());
        let row = argument_sum_v1(&[first, slot_ordinal - instance_slots.slots.start])?;
        let key = TileAttachmentKeyV29 {
            root,
            family: TileAttachmentFamilyV29::SourceSlot,
            instance,
            row,
            field: TileAttachmentFieldV29::SlotAllocation,
            component: 0,
            part: 0,
        };
        let [attachment] = original.attachment_range(key, budget)? else {
            return original
                .source
                .missing("optimized original slot allocation attachment census");
        };
        if source_attachment_key_v18(attachment.key) != source_attachment_key_v18(key) {
            return original
                .source
                .missing("optimized original slot allocation attachment part");
        }
        let TileAttachmentLocationV29::Origin(TileScalarSourceV29::Operation(point)) =
            attachment.location
        else {
            return original
                .source
                .missing("optimized original slot allocation attachment role");
        };
        if point.function != owner.function_ordinal {
            return original
                .source
                .missing("optimized original slot allocation root");
        }
        let input = fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1 {
            block: fe2o3_kernel_ir::CanonicalKirBlockCoordinateV1 {
                function: fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1(
                    u32::try_from(point.function).map_err(|_| ArgumentResourceV1::Arithmetic)?,
                ),
                block: u32::try_from(point.block).map_err(|_| ArgumentResourceV1::Arithmetic)?,
            },
            operation: u32::try_from(point.operation)
                .map_err(|_| ArgumentResourceV1::Arithmetic)?,
        };
        let actual = source_operation_row_v18(original.inventory, input, budget)?.operation;
        if !matches!(
            actual.kind,
            OperationKind::Alloca {
                address_space: AddressSpace::Private,
                ..
            }
        ) || !matches!(actual.results.as_slice(), [result] if result.id == slot.origin.pointer)
        {
            return original
                .source
                .missing("optimized original slot allocation changed backing");
        }
        Ok(input)
    })())
}

// The copied rows are hypotheses for the unchanged physical solver. Their
// source identities and shape stay original; only exactly checked physical
// occurrences change. The original slot table is never mutated or rebound.
