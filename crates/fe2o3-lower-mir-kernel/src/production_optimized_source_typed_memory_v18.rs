#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum OptimizedMemoryBoundaryRoleV18 {
    Constant,
    Ordered,
    TypedSourcePending,
}

// Exhaustive classification is deliberately separate from scalar_memory_access.
// A missing scalar access is never permission to omit an ordered typed event.
fn optimized_memory_boundary_role_v18(kind: &OperationKind) -> OptimizedMemoryBoundaryRoleV18 {
    use OptimizedMemoryBoundaryRoleV18 as Role;
    match kind {
        OperationKind::Constant(_) => Role::Constant,
        OperationKind::Storage(operation) => match operation {
            fe2o3_kernel_ir::StorageOperationV1::Project { .. }
            | fe2o3_kernel_ir::StorageOperationV1::ReadValue { .. }
            | fe2o3_kernel_ir::StorageOperationV1::ReadDiscriminant { .. }
            | fe2o3_kernel_ir::StorageOperationV1::WriteValue { .. }
            | fe2o3_kernel_ir::StorageOperationV1::CopyObject { .. }
            | fe2o3_kernel_ir::StorageOperationV1::SetDiscriminant { .. } => {
                Role::TypedSourcePending
            }
        },
        OperationKind::Execution(operation) => match operation {
            fe2o3_kernel_ir::ExecutionOperationV15::ContextIssue
            | fe2o3_kernel_ir::ExecutionOperationV15::WorkgroupDerive { .. }
            | fe2o3_kernel_ir::ExecutionOperationV15::ScopeEnd { .. }
            | fe2o3_kernel_ir::ExecutionOperationV15::MaskedTileLoadU32 { .. }
            | fe2o3_kernel_ir::ExecutionOperationV15::TileIntoFragmentU32 { .. }
            | fe2o3_kernel_ir::ExecutionOperationV15::FragmentIntoPartsU32 { .. } => Role::Ordered,
        },
        OperationKind::VerificationContract(_)
        | OperationKind::VectorLoad(_)
        | OperationKind::VectorStore(_)
        | OperationKind::VectorLayoutConvert(_)
        | OperationKind::Intrinsic(_)
        | OperationKind::MemoryIntrinsic(_)
        | OperationKind::Unary { .. }
        | OperationKind::Binary { .. }
        | OperationKind::Compare { .. }
        | OperationKind::Cast { .. }
        | OperationKind::Select { .. }
        | OperationKind::Call { .. }
        | OperationKind::Alloca { .. }
        | OperationKind::SliceLength { .. }
        | OperationKind::SliceData { .. }
        | OperationKind::GetElementPointer { .. }
        | OperationKind::Load { .. }
        | OperationKind::GuardedLoad { .. }
        | OperationKind::GuardedStore { .. }
        | OperationKind::Store { .. }
        | OperationKind::Barrier(_)
        | OperationKind::Atomic(_)
        | OperationKind::Fence(_)
        | OperationKind::WorkgroupBarrier(_)
        | OperationKind::WorkgroupMemory(_)
        | OperationKind::Matrix(_)
        | OperationKind::Gfx950LdsTranspose(_)
        | OperationKind::Wave(_)
        | OperationKind::InlineAssembly(_)
        | OperationKind::Gfx942OrderedRegion(_)
        | OperationKind::Gfx942OrderedProgram(_)
        | OperationKind::Gfx942CompleteBodyDeclaration(_)
        | OperationKind::Gfx942CompleteBodyStep(_)
        | OperationKind::Gfx942PhysicalEntryDeclaration(_)
        | OperationKind::Gfx942PhysicalEntryStep(_)
        | OperationKind::Gfx942PhysicalGlobalCopyDeclaration(_)
        | OperationKind::Gfx942PhysicalGlobalCopyStep(_)
        | OperationKind::Gfx942PhysicalLdsExchangeDeclaration(_)
        | OperationKind::Gfx942PhysicalLdsExchangeStep(_) => Role::Ordered,
    }
}

struct OptimizedMemoryGapPrefixV18 {
    // One prefix entry per output operation plus one per block. The inventory's
    // dense operation range and block ordinal select a prefix in constant time.
    nonconstant: Vec<usize>,
}

impl OptimizedMemoryGapPrefixV18 {
    fn build(
        original: &ProductionSourceCorrespondenceV18<'_>,
        optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<Self> {
        optimized_source_endpoints_v18(original, optimized, budget)?;
        let output = optimized.output_inventory(budget)?;
        let length = argument_sum_v1(&[output.operations().len(), output.blocks().len()])?;
        let mut nonconstant =
            emission_vec_v1(length, budget).map_err(immutable_memory_error_v29)?;
        for block in output.blocks() {
            budget.charge_work(1)?;
            let mut count = 0;
            nonconstant.push(count);
            for row in &output.operations()[block.operations.clone()] {
                budget.charge_work(2)?;
                if optimized_memory_boundary_role_v18(&row.operation.kind)
                    != OptimizedMemoryBoundaryRoleV18::Constant
                {
                    count = argument_sum_v1(&[count, 1])?;
                }
                nonconstant.push(count);
            }
        }
        Ok(Self { nonconstant })
    }

    fn boundary(
        &self,
        original: &ProductionSourceCorrespondenceV18<'_>,
        optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
        root: usize,
        block: BlockId,
        original_gap: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<Option<(BlockId, usize)>> {
        Ok(self
            .boundary_with_source_v45(original, optimized, root, block, original_gap, budget)?
            .2)
    }

    fn boundary_with_source_v45(
        &self,
        original: &ProductionSourceCorrespondenceV18<'_>,
        optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
        root: usize,
        block: BlockId,
        original_gap: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<(
        fe2o3_kernel_ir::CanonicalKirBlockCoordinateV1,
        usize,
        Option<(BlockId, usize)>,
    )> {
        optimized_source_endpoints_v18(original, optimized, budget)?;
        let owner = original.source.root_row(root)?;
        let input_function = fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1(
            u32::try_from(owner.function_ordinal).map_err(|_| ArgumentResourceV1::Arithmetic)?,
        );
        let input_block = original
            .inventory
            .block_for_id(input_function, block, budget)
            .map_err(|error| {
                ProductionSourceOwnedViewErrorV18::from(
                    fe2o3_pliron::CanonicalAnalysisScopeErrorV1::Inventory(error),
                )
            })?
            .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                "optimized lifetime original block",
            ))?;
        let gap = immutable_memory_gap_v29(owner, block, original_gap, true, budget)?;
        let actual =
            self.canonical_boundary(original, optimized, input_block.coordinate, gap, budget)?;
        Ok((input_block.coordinate, gap, actual))
    }

    fn canonical_boundary(
        &self,
        original: &ProductionSourceCorrespondenceV18<'_>,
        optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
        block: fe2o3_kernel_ir::CanonicalKirBlockCoordinateV1,
        gap: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<Option<(BlockId, usize)>> {
        optimized_source_endpoints_v18(original, optimized, budget)?;
        let interval = match optimized.physical_gap(block, gap, budget)? {
            ProductionOptimizedSourceGapV18::Reachable(interval) => interval,
            ProductionOptimizedSourceGapV18::Unreachable { .. } => return Ok(None),
        };
        let output = optimized.output_inventory(budget)?;
        let row = source_block_row_v18(output, interval.block, budget)?;
        let function = &output.functions()[interval.block.function.0 as usize];
        let dense_block = argument_sum_v1(&[function.blocks.start, interval.block.block as usize])?;
        let start = argument_sum_v1(&[row.operations.start, dense_block])?;
        let first = argument_sum_v1(&[start, interval.first as usize])?;
        let last = argument_sum_v1(&[start, interval.last as usize])?;
        budget.charge_work(4)?;
        if interval.first > interval.last
            || interval.last as usize > row.operations.len()
            || self
                .nonconstant
                .get(first)
                .zip(self.nonconstant.get(last))
                .is_none_or(|(a, b)| a != b)
        {
            return original
                .source
                .missing("optimized lifetime gap crosses a state-changing or trapping operation");
        }
        // Every interposed occurrence is a checked Constant with no memory,
        // pointer birth, execution, or trap transition. Thus both boundary
        // states are identical; the first is an equivalent representative.
        Ok(Some((row.block.id, interval.first as usize)))
    }
}

fn require_optimized_source_typed_roles_v18(
    original: &ProductionSourceCorrespondenceV18<'_>,
    optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    root: usize,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<OptimizedSourceObjectCensusV18> {
    // This validates the complete original/actual typed-operation census, not
    // merely matching endpoint schemas. The physical solver must separately
    // account for every returned occurrence before exposing currentness.
    visit_optimized_source_objects_v18(original, optimized, root, budget, |row, budget| {
        budget.charge_work(1)?;
        if !matches!(
            row.actual.operation,
            ScopedObjectOperationV29::ReadValue { .. }
                | ScopedObjectOperationV29::WriteValue { .. }
                | ScopedObjectOperationV29::Project {
                    step: ScopedObjectProjectionV29::Field(_)
                        | ScopedObjectProjectionV29::ArrayIndex(_),
                    ..
                }
        ) {
            return original
                .source
                .missing("optimized typed tag/copy/variant currentness remains pending");
        }
        Ok(())
    })
}

include!("production_optimized_source_projects_v33.rs");

#[cfg(test)]
pub(super) fn test_optimized_whole_value_roles_v18(
    original: &ProductionSourceCorrespondenceV18<'_>,
    optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    root: usize,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<(usize, usize)> {
    let census = require_optimized_source_typed_roles_v18(original, optimized, root, budget)?;
    Ok((census.retained, census.unreachable))
}

// Borrowed original metadata and the exact mapped output payload. This is an
// input to C2 currentness, never a replacement source/object/schema owner.
pub(super) struct OptimizedSourceObjectV18<'a> {
    pub(super) original: SourcePhysicalObjectV18<'a>,
    pub(super) input: fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1,
    pub(super) output: fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1,
    pub(super) actual: ScopedObjectPayloadV29,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct OptimizedSourceObjectCensusV18 {
    retained: usize,
    unreachable: usize,
    retained_footprints: usize,
    unreachable_footprints: usize,
}

pub(super) fn visit_optimized_source_objects_v18(
    original: &ProductionSourceCorrespondenceV18<'_>,
    optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    root: usize,
    budget: &mut ArgumentBudgetV1<'_>,
    mut visit: impl FnMut(
        OptimizedSourceObjectV18<'_>,
        &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<()>,
) -> SourceOwnedResultV18<OptimizedSourceObjectCensusV18> {
    original.retain_query((|| {
        optimized_source_endpoints_v18(original, optimized, budget)?;
        let root_row = original.source.root_row(root)?;
        let input = original
            .inventory
            .functions()
            .get(root_row.function_ordinal)
            .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                "typed census original function",
            ))?;
        let output = optimized_source_root_function_v18(original, optimized, root, budget)?;
        let floor = budget.storage();
        let visitor_header = std::mem::size_of_val(&visit);
        let run = |budget: &mut ArgumentBudgetV1<'_>| {
            let mut input_seen = emission_vec_v1(input.operations.len(), budget)
                .map_err(source_emission_error_v18)?;
            let mut output_seen = emission_vec_v1(output.operations.len(), budget)
                .map_err(source_emission_error_v18)?;
            budget.charge_work(argument_sum_v1(&[
                input.operations.len(),
                output.operations.len(),
            ])?)?;
            input_seen.resize(input.operations.len(), 0u8);
            output_seen.resize(output.operations.len(), 0u8);
            let mut census = OptimizedSourceObjectCensusV18::default();
            for source in &root_row.coordinates.sources.rows {
                budget.charge_work(1)?;
                let instance = source.instance.index();
                let Some(sidecar) = original.source.optional_sidecar(root, instance, budget)?
                else {
                    continue;
                };
                let anchors = sidecar.scoped_memory_anchors.as_ref().ok_or(
                    ProductionSourceOwnedViewErrorV18::Binding("typed census original anchors"),
                )?;
                for (row, anchor) in anchors.rows.iter().enumerate() {
                    budget.charge_work(1)?;
                    if !matches!(anchor.kind, ScopedMemoryAnchorKindV29::Object(_)) {
                        continue;
                    }
                    let key = TileAttachmentKeyV29 {
                        root,
                        family: TileAttachmentFamilyV29::MemoryAnchor,
                        instance,
                        row,
                        field: TileAttachmentFieldV29::MemoryPosition,
                        component: 0,
                        part: 0,
                    };
                    let [position] = original.attachment_range(key, budget)? else {
                        return original
                            .source
                            .missing("typed census exact original position");
                    };
                    let ProductionSourceOperationV18::Operation(input_operation) =
                        original.mapped_source_operation(position.location, budget)?
                    else {
                        return original
                            .source
                            .missing("typed census position is not an operation");
                    };
                    let payload = original.retained_object_payload_at_v29(
                        root,
                        instance,
                        row,
                        input_operation,
                        budget,
                    )?;
                    let mut footprints = 0;
                    payload
                        .actual
                        .operation
                        .try_visit_memory_accesses(|_, _, _| {
                            budget.charge_work(1)?;
                            footprints = argument_sum_v1(&[footprints, 1])?;
                            Ok::<_, ProductionSourceOwnedViewErrorV18>(())
                        })?;
                    mark_source_object_operation_v18(
                        original.inventory,
                        input,
                        input_operation,
                        &mut input_seen,
                        budget,
                    )?;
                    match optimized.operation(input_operation, budget)? {
                        ProductionOptimizedSourceOperationV18::RemovedUnreachable { .. } => {
                            census.unreachable_footprints =
                                argument_sum_v1(&[census.unreachable_footprints, footprints])?;
                            census.unreachable = census
                                .unreachable
                                .checked_add(1)
                                .ok_or(ArgumentResourceV1::Arithmetic)?;
                        }
                        ProductionOptimizedSourceOperationV18::Rewritten { .. } => {
                            return original
                                .source
                                .missing("typed source operation has no exact retained output");
                        }
                        ProductionOptimizedSourceOperationV18::Retained {
                            output: coordinate,
                            ..
                        } => {
                            let inventory = optimized.output_inventory(budget)?;
                            mark_source_object_operation_v18(
                                inventory,
                                output,
                                coordinate,
                                &mut output_seen,
                                budget,
                            )?;
                            let actual = optimized_source_object_payload_v18(
                                original,
                                optimized,
                                input_operation,
                                coordinate,
                                &payload.actual,
                                budget,
                            )?;
                            let callback_floor = budget.storage();
                            let selected =
                                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                                    visit(
                                        OptimizedSourceObjectV18 {
                                            original: payload,
                                            input: input_operation,
                                            output: coordinate,
                                            actual,
                                        },
                                        budget,
                                    )
                                }));
                            let balanced = budget.storage() == callback_floor;
                            if !balanced {
                                original.source.cleanup.deny_refund();
                            }
                            match selected {
                                Ok(Err(error)) => return Err(error),
                                Err(payload) => std::panic::resume_unwind(payload),
                                Ok(Ok(())) if !balanced => {
                                    return Err(ArgumentResourceV1::Accounting.into());
                                }
                                Ok(Ok(())) => {}
                            }
                            optimized_source_endpoints_v18(original, optimized, budget)?;
                            census.retained = census
                                .retained
                                .checked_add(1)
                                .ok_or(ArgumentResourceV1::Arithmetic)?;
                            census.retained_footprints =
                                argument_sum_v1(&[census.retained_footprints, footprints])?;
                        }
                    }
                }
            }
            // Compiler holders are disjoint from source objects. Their exact
            // source-owned role and closed operand census must account for
            // typed stores before the complete operation census below.
            if let Some(pending) = &root_row.source_slots.pending_memory {
                if pending
                    .compiler_enum
                    .has_typed_allocations_v57(budget)
                    .map_err(immutable_memory_error_v29)?
                {
                    let compiler = immutable_compiler_enum_memory_v55(
                        original,
                        root,
                        &pending.compiler_enum,
                        budget,
                    )?;
                    check_compiler_enum_closed_memory_v55(
                        input.function,
                        &original.inventory.owner().module().storage_layouts,
                        &compiler,
                        budget,
                    )
                    .map_err(immutable_memory_error_v29)?;
                    let mapped =
                        map_compiler_enum_memory_v57(original, root, &compiler, optimized, budget)?;
                    let inventory = optimized.output_inventory(budget)?;
                    check_compiler_enum_closed_memory_v55(
                        output.function,
                        &inventory.owner().module().storage_layouts,
                        &mapped,
                        budget,
                    )
                    .map_err(immutable_memory_error_v29)?;
                    for (inventory, function, memory, marks) in [
                        (
                            original.inventory,
                            input,
                            &compiler,
                            input_seen.as_mut_slice(),
                        ),
                        (inventory, output, &mapped, output_seen.as_mut_slice()),
                    ] {
                        for access in &memory.accesses {
                            budget.charge_work(1)?;
                            let allocation = memory
                                .allocation(access.record.pointer, budget)
                                .map_err(immutable_memory_error_v29)?
                                .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                                    "typed compiler holder missing",
                                ))?;
                            if allocation.origin.storage.is_none() {
                                continue;
                            }
                            let block = inventory
                                .block_for_id(function.coordinate, access.block, budget)
                                .map_err(source_pointer_inventory_error_v18)?
                                .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                                    "typed compiler block missing",
                                ))?;
                            let coordinate = fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1 {
                                block: block.coordinate,
                                operation: u32::try_from(access.operation)
                                    .map_err(|_| ArgumentResourceV1::Arithmetic)?,
                            };
                            mark_source_object_operation_v18(
                                inventory, function, coordinate, marks, budget,
                            )?;
                        }
                    }
                }
            }
            for (inventory, function, marks) in [
                (original.inventory, input, input_seen.as_slice()),
                (
                    optimized.output_inventory(budget)?,
                    output,
                    output_seen.as_slice(),
                ),
            ] {
                let operations = inventory
                    .operations()
                    .get(function.operations.clone())
                    .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                        "typed census complete operation range",
                    ))?;
                if operations.len() != marks.len() {
                    return original
                        .source
                        .missing("typed census changed operation count");
                }
                for (operation, mark) in operations.iter().zip(marks) {
                    budget.charge_work(1)?;
                    if matches!(operation.operation.kind, OperationKind::Storage(_)) != (*mark == 1)
                    {
                        return original
                            .source
                            .missing("typed census duplicate, orphan or scalar substitution");
                    }
                }
            }
            Ok::<_, ProductionSourceOwnedViewErrorV18>(census)
        };
        type Result = SourceOwnedResultV18<OptimizedSourceObjectCensusV18>;
        budget.reserve_storage(argument_sum_v1(&[
            std::mem::size_of_val(&run),
            size_of::<Result>(),
            size_of::<std::thread::Result<Result>>(),
            visitor_header,
            size_of::<SourceOwnedResultV18<()>>(),
            size_of::<std::thread::Result<SourceOwnedResultV18<()>>>(),
            size_of::<std::panic::AssertUnwindSafe<SourceOwnedResultV18<()>>>(),
            size_of::<Box<dyn std::any::Any + Send>>(),
            size_of::<std::panic::AssertUnwindSafe<Result>>(),
            size_of::<OptimizedSourceObjectCensusV18>(),
            size_of::<usize>(),
            size_of::<SourcePhysicalObjectV18<'_>>(),
            size_of::<OptimizedSourceObjectV18<'_>>(),
            size_of::<ScopedObjectPayloadV29>(),
            size_of::<[Option<ValueId>; 2]>(),
            argument_product_v1(2, size_of::<Vec<u8>>())?,
        ])?)?;
        let result = scoped_source_attempt_v29(original.source.cleanup, budget, floor, run)?;
        budget.release_storage(
            budget
                .storage()
                .checked_sub(floor)
                .ok_or(ArgumentResourceV1::Accounting)?,
        )?;
        Ok(result)
    })())
}

fn mark_source_object_operation_v18(
    inventory: &fe2o3_kernel_analysis::CanonicalKirInventoryV18<'_>,
    function: &fe2o3_kernel_analysis::CanonicalKirFunctionRefV1<'_>,
    coordinate: fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1,
    seen: &mut [u8],
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<()> {
    budget.charge_work(4)?;
    if coordinate.block.function != function.coordinate {
        return Err(ProductionSourceOwnedViewErrorV18::Binding(
            "typed census foreign function",
        ));
    }
    let block = source_block_row_v18(inventory, coordinate.block, budget)?;
    let dense = block
        .operations
        .start
        .checked_add(coordinate.operation as usize)
        .filter(|row| *row < block.operations.end && function.operations.contains(row))
        .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
            "typed census operation index",
        ))?;
    let operation = inventory
        .operations()
        .get(dense)
        .filter(|operation| {
            operation.coordinate == coordinate
                && matches!(operation.operation.kind, OperationKind::Storage(_))
        })
        .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
            "typed census actual Storage operation",
        ))?;
    let mark = seen.get_mut(dense - function.operations.start).ok_or(
        ProductionSourceOwnedViewErrorV18::Binding("typed census dense mark"),
    )?;
    if *mark != 0 || operation.coordinate != coordinate {
        return Err(ProductionSourceOwnedViewErrorV18::Binding(
            "typed census repeats an operation",
        ));
    }
    *mark = 1;
    Ok(())
}

pub(super) fn optimized_source_object_payload_v18(
    original: &ProductionSourceCorrespondenceV18<'_>,
    optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    input: fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1,
    output: fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1,
    source: &ScopedObjectPayloadV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<ScopedObjectPayloadV29> {
    use fe2o3_kernel_ir::CanonicalKirUseCoordinateV1 as Use;
    let mut values = [None; 2];
    let mut count = 0usize;
    for (operand, value) in source.operands().into_iter().enumerate() {
        let Some(value) = value else {
            continue;
        };
        let operand = u32::try_from(operand).map_err(|_| ArgumentResourceV1::Arithmetic)?;
        let input_use = Use::OperationOperand {
            operation: input,
            operand,
        };
        let (actual_use, actual_value) =
            optimized_source_actual_operand_v18(original, optimized, input_use, output, budget)?;
        budget.charge_work(2)?;
        if actual_use.coordinate
            != (Use::OperationOperand {
                operation: output,
                operand,
            })
            || !optimized_source_value_descends_v18(
                original,
                optimized,
                input.block.function,
                value,
                output.block.function,
                actual_value,
                budget,
            )?
        {
            return original
                .source
                .missing("typed payload changed exact output operand role");
        }
        values[operand as usize] = Some(actual_value);
        count += 1;
    }
    let inventory = optimized.output_inventory(budget)?;
    let operation = source_operation_row_v18(inventory, output, budget)?;
    let result = match (source.result, operation.operation.results.as_slice()) {
        (None, []) => None,
        (Some(input_value), [result])
            if optimized_source_value_descends_v18(
                original,
                optimized,
                input.block.function,
                input_value,
                output.block.function,
                result.id,
                budget,
            )? =>
        {
            Some(result.id)
        }
        _ => {
            return original
                .source
                .missing("typed payload changed actual result role");
        }
    };
    let mut mapped = *source;
    let mut ordinal = 0usize;
    mapped
        .try_map_values(|_| {
            let value = if ordinal < count {
                values[ordinal]
            } else {
                result
            };
            ordinal += 1;
            value.ok_or_else(scoped_object_error_v29)
        })
        .map_err(|error| source_attachment_error_v18(error.into()))?;
    mapped
        .check_operation(operation.operation, budget)
        .map_err(|error| source_attachment_error_v18(error.into()))?;
    Ok(mapped)
}
