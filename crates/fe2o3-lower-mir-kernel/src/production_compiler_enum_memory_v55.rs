// Compiler-private storage is disjoint from original source slots. This
// retained table is replay data; only the closed physical check below can
// produce the borrowed argument accepted by the source address solver.
#[cfg_attr(test, derive(Clone))]
struct PendingCompilerEnumMemoryV55 {
    allocations: Vec<SourceEnumSpillRowV48>,
    accesses: Vec<PendingCompilerEnumAccessV55>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PendingCompilerEnumAccessV55 {
    instance: ProductionCallInstanceIdV1,
    record: ScopedCompilerEnumAccessV55,
    block: BlockId,
    operation: usize,
    reference: Option<SourceCompilerEnumReferenceV55>,
}

pub(super) struct CheckedCompilerEnumMemoryV55<'a> {
    function: &'a Function,
    pending: &'a PendingCompilerEnumMemoryV55,
}

impl CheckedCompilerEnumMemoryV55<'_> {
    pub(super) fn belongs_to(&self, function: &Function) -> bool {
        std::ptr::eq(self.function, function)
    }

    pub(super) fn contains_allocation(
        &self,
        pointer: ValueId,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<bool, ProductionSemanticKirErrorV1> {
        self.pending
            .allocation(pointer, budget)
            .map(|row| row.is_some())
    }

    pub(super) fn reference_uses(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Vec<SourceCompilerEnumReferenceUseV55>, ProductionSemanticKirErrorV1> {
        source_reference_emission_prepay_v29::<Vec<SourceCompilerEnumReferenceUseV55>>(budget)?;
        source_reference_emission_prepay_v29::<SourceCompilerEnumReferenceUseV55>(budget)?;
        let mut output = emission_vec_v1(self.pending.accesses.len(), budget)?;
        for access in &self.pending.accesses {
            budget.charge_work(3)?;
            let Some(custody) = access.reference else {
                continue;
            };
            let ScopedCompilerEnumRoleV55::Store { value, .. } = access.record.role else {
                return Err(scoped_compiler_enum_error_v55());
            };
            output.push(SourceCompilerEnumReferenceUseV55 {
                block: access.block,
                operation: access.operation,
                value,
                custody,
            });
        }
        Ok(output)
    }
}

fn compiler_enum_element_owned_storage_v55(ty: &Type) -> Result<usize, ArgumentResourceV1> {
    match ty {
        Type::Scalar(_) | Type::Vector(_) => Ok(0),
        Type::Pointer(pointer)
            if matches!(
                pointer.pointee.as_ref(),
                Type::Scalar(_) | Type::StorageObject(_)
            ) =>
        {
            Ok(size_of::<Type>())
        }
        _ => Err(ArgumentResourceV1::Accounting),
    }
}

include!("production_optimized_compiler_enum_memory_v55.rs");
#[cfg(test)]
include!("production_compiler_enum_memory_v55_tests.rs");
#[cfg(test)]
include!("production_compiler_enum_reference_memory_v55_tests.rs");

impl PendingCompilerEnumMemoryV55 {
    fn retained_storage(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<usize, ArgumentResourceV1> {
        let mut owned_types = 0;
        for row in &self.allocations {
            budget.charge_work(2)?;
            owned_types = argument_sum_v1(&[
                owned_types,
                compiler_enum_element_owned_storage_v55(&row.origin.element)?,
            ])?;
        }
        argument_sum_v1(&[
            owned_types,
            argument_product_v1(
                self.allocations.capacity(),
                size_of::<SourceEnumSpillRowV48>(),
            )?,
            argument_product_v1(
                self.accesses.capacity(),
                size_of::<PendingCompilerEnumAccessV55>(),
            )?,
        ])
    }

    fn allocation(
        &self,
        pointer: ValueId,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Option<&SourceEnumSpillRowV48>, ProductionSemanticKirErrorV1> {
        charge_execution_cfg_lookup_v29(self.allocations.len(), budget)?;
        Ok(self
            .allocations
            .binary_search_by_key(&pointer, |row| row.origin.pointer)
            .ok()
            .map(|ordinal| &self.allocations[ordinal]))
    }

    fn access(
        &self,
        block: BlockId,
        operation: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Option<&PendingCompilerEnumAccessV55>, ProductionSemanticKirErrorV1> {
        charge_execution_cfg_lookup_v29(self.accesses.len(), budget)?;
        Ok(self
            .accesses
            .binary_search_by_key(&(block, operation), |row| (row.block, row.operation))
            .ok()
            .map(|ordinal| &self.accesses[ordinal]))
    }
}

fn compiler_enum_memory_equal_v55(
    left: &PendingCompilerEnumMemoryV55,
    right: &PendingCompilerEnumMemoryV55,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<bool, ProductionSemanticKirErrorV1> {
    if !source_enum_spills_equal_v48(&left.allocations, &right.allocations, budget)? {
        return Ok(false);
    }
    budget.charge_work(argument_sum_v1(&[
        1,
        argument_product_v1(
            left.accesses.len(),
            size_of::<PendingCompilerEnumAccessV55>(),
        )?,
    ])?)?;
    Ok(left.accesses == right.accesses)
}

pub(super) fn source_address_compiler_enum_access_v55(
    instances: &ExecutionInstancesV29<'_>,
    references: &SourceReferenceEmissionV29<'_, '_>,
    slots: &OwnedScopedSourceSlotsV29,
    source: &SourceAddressSourceIndexV29<'_>,
    instance: ProductionCallInstanceIdV1,
    anchor: usize,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<Option<Option<SourceCompilerEnumReferenceV55>>, ProductionSemanticKirErrorV1> {
    references.check(budget)?;
    if !std::ptr::eq(references.plan.instances, instances) {
        return Err(scoped_compiler_enum_error_v55());
    }
    let sidecar = source.sidecar(instance, budget)?;
    let anchors = sidecar
        .scoped_memory_anchors
        .as_ref()
        .ok_or_else(scoped_compiler_enum_error_v55)?;
    charge_execution_cfg_lookup_v29(anchors.compiler_enum.len(), budget)?;
    let Ok(ordinal) = anchors
        .compiler_enum
        .binary_search_by_key(&anchor, |row| row.anchor)
    else {
        return Ok(None);
    };
    let record = &anchors.compiler_enum[ordinal];
    let row = anchors
        .rows
        .get(anchor)
        .ok_or_else(scoped_compiler_enum_error_v55)?;
    let archive = sidecar
        .execution_observation
        .as_ref()
        .ok_or_else(scoped_compiler_enum_error_v55)?;
    let operation = source
        .emitted
        .operation(instance, row.block, row.position, budget)?;
    let checked = check_scoped_compiler_enum_access_v55(
        instances, instance, anchors, archive, record, operation, budget,
    )?;
    source.frame_gap(
        instance,
        row.source.ok_or_else(scoped_compiler_enum_error_v55)?,
        row.block,
        row.position,
        budget,
    )?;
    let reference =
        retain_compiler_enum_reference_v55(references, instance, archive, &checked, slots, budget)?;
    if reference.is_none() {
        check_scoped_compiler_enum_scalar_value_v55(&checked, budget)?;
    }
    Ok(Some(reference))
}

fn pending_compiler_enum_memory_v55(
    instances: &ExecutionInstancesV29<'_>,
    references: &SourceReferenceEmissionV29<'_, '_>,
    slots: &OwnedScopedSourceSlotsV29,
    source: &SourceAddressSourceIndexV29<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<PendingCompilerEnumMemoryV55, ProductionSemanticKirErrorV1> {
    let mut allocations = 0;
    let mut accesses = 0;
    for sidecar in &source.pending.sidecars.rows {
        budget.charge_work(3)?;
        let archive = sidecar
            .execution_observation
            .as_ref()
            .ok_or_else(scoped_compiler_enum_error_v55)?;
        let anchors = sidecar
            .scoped_memory_anchors
            .as_ref()
            .ok_or_else(scoped_compiler_enum_error_v55)?;
        allocations = argument_sum_v1(&[allocations, archive.enum_spills.len()])?;
        accesses = argument_sum_v1(&[accesses, anchors.compiler_enum.len()])?;
    }
    let mut output = PendingCompilerEnumMemoryV55 {
        allocations: emission_vec_v1(allocations, budget)?,
        accesses: emission_vec_v1(accesses, budget)?,
    };
    for sidecar in &source.pending.sidecars.rows {
        budget.charge_work(4)?;
        let instance = sidecar
            .source_call_instance
            .ok_or_else(scoped_compiler_enum_error_v55)?;
        let archive = sidecar
            .execution_observation
            .as_ref()
            .ok_or_else(scoped_compiler_enum_error_v55)?;
        archive.check_original_v29(instances, instance, budget)?;
        let anchors = sidecar
            .scoped_memory_anchors
            .as_ref()
            .ok_or_else(scoped_compiler_enum_error_v55)?;
        let mut previous = None;
        for spill in &archive.enum_spills {
            budget.charge_work(4)?;
            let key = scoped_compiler_enum_key_v55(spill);
            if previous.is_some_and(|old| old >= key) {
                return Err(scoped_compiler_enum_error_v55());
            }
            compiler_enum_element_owned_storage_v55(&spill.element)?;
            previous = Some(key);
            retain_source_enum_spill_v48(instance.index(), spill, &mut output.allocations, budget)?;
        }
        let mut previous = None;
        for record in &anchors.compiler_enum {
            budget.charge_work(3)?;
            if previous.is_some_and(|old| old >= record.anchor) {
                return Err(scoped_compiler_enum_error_v55());
            }
            previous = Some(record.anchor);
            let reference = source_address_compiler_enum_access_v55(
                instances,
                references,
                slots,
                source,
                instance,
                record.anchor,
                budget,
            )?
            .ok_or_else(scoped_compiler_enum_error_v55)?;
            let row = anchors
                .rows
                .get(record.anchor)
                .ok_or_else(scoped_compiler_enum_error_v55)?;
            let (block, operation) = source
                .emitted
                .point(
                    instance,
                    row.block,
                    u32::try_from(row.position).map_err(|_| ArgumentResourceV1::Arithmetic)?,
                    budget,
                )?
                .ok_or_else(scoped_compiler_enum_error_v55)?;
            if output.accesses.len() == output.accesses.capacity() {
                return Err(ArgumentResourceV1::Accounting.into());
            }
            output.accesses.push(PendingCompilerEnumAccessV55 {
                instance,
                record: *record,
                block,
                operation: operation as usize,
                reference,
            });
        }
    }
    if output.allocations.len() != allocations || output.accesses.len() != accesses {
        return Err(ArgumentResourceV1::Accounting.into());
    }
    call_splice_sort_work_v1(output.allocations.len(), budget)
        .map_err(source_address_call_error_v29)?;
    output
        .allocations
        .sort_unstable_by_key(|row| row.origin.pointer);
    call_splice_sort_work_v1(output.accesses.len(), budget)
        .map_err(source_address_call_error_v29)?;
    output
        .accesses
        .sort_unstable_by_key(|row| (row.block, row.operation));
    Ok(output)
}

fn check_compiler_enum_closed_memory_v55<'a>(
    function: &'a Function,
    pending: &'a PendingCompilerEnumMemoryV55,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<CheckedCompilerEnumMemoryV55<'a>, ProductionSemanticKirErrorV1> {
    let body = function
        .body
        .as_ref()
        .ok_or_else(scoped_compiler_enum_error_v55)?;
    budget.charge_work(argument_sum_v1(&[
        pending.allocations.len(),
        pending.accesses.len(),
    ])?)?;
    if pending
        .allocations
        .windows(2)
        .any(|rows| rows[0].origin.pointer >= rows[1].origin.pointer)
        || pending
            .accesses
            .windows(2)
            .any(|rows| (rows[0].block, rows[0].operation) >= (rows[1].block, rows[1].operation))
    {
        return Err(scoped_compiler_enum_error_v55());
    }
    if pending.allocations.is_empty() {
        if !pending.accesses.is_empty() {
            return Err(scoped_compiler_enum_error_v55());
        }
        return Ok(CheckedCompilerEnumMemoryV55 { function, pending });
    }
    let mut allocations = 0;
    let mut accesses = 0;
    for block in &body.blocks {
        budget.charge_work(1)?;
        for (position, operation) in block.operations.iter().enumerate() {
            budget.charge_work(1)?;
            if matches!(operation.kind, OperationKind::Alloca { .. }) {
                let [result] = operation.results.as_slice() else {
                    return Err(scoped_compiler_enum_error_v55());
                };
                if let Some(row) = pending.allocation(result.id, budget)? {
                    if body.blocks.first().map(|block| block.id) != Some(block.id) {
                        return Err(scoped_compiler_enum_error_v55());
                    }
                    compiler_enum_element_owned_storage_v55(&row.origin.element)?;
                    check_enum_spill_alloca_v55(&row.origin, operation, budget)?;
                    allocations = argument_sum_v1(&[allocations, 1])?;
                }
            }
            let access = pending.access(block.id, position, budget)?;
            if let Some(access) = access {
                let row = pending
                    .allocation(access.record.pointer, budget)?
                    .ok_or_else(scoped_compiler_enum_error_v55)?;
                let ScopedCompilerEnumRoleV55::Store { value, source, .. } = access.record.role
                else {
                    return Err(scoped_compiler_enum_error_v55());
                };
                match (access.reference, source, &row.origin.element) {
                    (None, None, Type::Scalar(_) | Type::Vector(_)) => {}
                    (
                        Some(reference),
                        Some(ScopedMemoryStoreSourceV29::Operand { .. }),
                        Type::Pointer(pointer),
                    ) => {
                        let expected = match reference.backing {
                            SourceCompilerEnumReferenceBackingV55::Scalar => {
                                matches!(pointer.pointee.as_ref(), Type::Scalar(_))
                            }
                            SourceCompilerEnumReferenceBackingV55::Object(schema) => {
                                pointer.pointee.as_ref() == &Type::StorageObject(schema)
                            }
                        };
                        if !expected
                            || reference.loan.carrier
                                != ProductionSourceReferenceCarrierV38::MemoryPointer
                        {
                            return Err(scoped_compiler_enum_error_v55());
                        }
                    }
                    _ => return Err(scoped_compiler_enum_error_v55()),
                }
                if row.instance != access.instance.index()
                    || scoped_compiler_enum_key_v55(&row.origin)
                        != (
                            access.record.local.index(),
                            access.record.variant,
                            access.record.field,
                            access.record.component as usize,
                        )
                    || !operation.results.is_empty()
                    || !matches!(operation.kind, OperationKind::Store { pointer, value: actual, access: actual_access }
                        if pointer == access.record.pointer && actual == value && actual_access == MemoryAccess::new(AddressSpace::Private, row.origin.alignment))
                {
                    return Err(scoped_compiler_enum_error_v55());
                }
                accesses = argument_sum_v1(&[accesses, 1])?;
            }
            let mut operand = 0;
            operation.kind.try_visit_operands(
                |value| -> Result<(), ProductionSemanticKirErrorV1> {
                    budget.charge_work(1)?;
                    let ordinal = operand;
                    operand = argument_sum_v1(&[operand, 1])?;
                    if pending.allocation(value, budget)?.is_some()
                        && (ordinal != 0
                            || !access.is_some_and(|access| access.record.pointer == value))
                    {
                        return Err(scoped_compiler_enum_error_v55());
                    }
                    Ok(())
                },
            )?;
        }
        block
            .terminator
            .as_ref()
            .ok_or_else(scoped_compiler_enum_error_v55)?
            .try_visit_operands(|value| -> Result<(), ProductionSemanticKirErrorV1> {
                budget.charge_work(1)?;
                if pending.allocation(value, budget)?.is_some() {
                    return Err(scoped_compiler_enum_error_v55());
                }
                Ok(())
            })?;
    }
    if allocations != pending.allocations.len() || accesses != pending.accesses.len() {
        return Err(scoped_compiler_enum_error_v55());
    }
    Ok(CheckedCompilerEnumMemoryV55 { function, pending })
}

fn immutable_compiler_enum_memory_v55(
    correspondence: &ProductionSourceCorrespondenceV18<'_>,
    root: usize,
    pending: &PendingCompilerEnumMemoryV55,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<PendingCompilerEnumMemoryV55> {
    correspondence.query(budget)?;
    let owner = correspondence.source.root_row(root)?;
    let function = fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1(
        u32::try_from(owner.function_ordinal).map_err(|_| ArgumentResourceV1::Arithmetic)?,
    );
    let mut output = PendingCompilerEnumMemoryV55 {
        allocations: emission_vec_v1(pending.allocations.len(), budget)
            .map_err(immutable_memory_error_v29)?,
        accesses: emission_vec_v1(pending.accesses.len(), budget)
            .map_err(immutable_memory_error_v29)?,
    };
    for row in &pending.allocations {
        retain_source_enum_spill_v48(row.instance, &row.origin, &mut output.allocations, budget)
            .map_err(immutable_memory_error_v29)?;
    }
    for access in &pending.accesses {
        budget.charge_work(8)?;
        let sidecar = correspondence
            .source
            .sidecar(root, access.instance.index(), budget)?;
        let anchors = sidecar.scoped_memory_anchors.as_ref().ok_or(
            ProductionSourceOwnedViewErrorV18::Binding("compiler access source anchors"),
        )?;
        charge_execution_cfg_lookup_v29(anchors.compiler_enum.len(), budget)
            .map_err(immutable_memory_error_v29)?;
        let ordinal = anchors
            .compiler_enum
            .binary_search_by_key(&access.record.anchor, |row| row.anchor)
            .map_err(|_| {
                ProductionSourceOwnedViewErrorV18::Binding("compiler access original role")
            })?;
        if anchors.subject.instance != access.instance
            || anchors.compiler_enum[ordinal] != access.record
        {
            return correspondence
                .source
                .missing("compiler access role substitution");
        }
        let anchor = anchors.rows.get(access.record.anchor).ok_or(
            ProductionSourceOwnedViewErrorV18::Binding("compiler access original anchor"),
        )?;
        if anchor.kind
            != (ScopedMemoryAnchorKindV29::Access {
                pointer: access.record.pointer,
                payload: None,
            })
        {
            return correspondence
                .source
                .missing("compiler access anchor substitution");
        }
        let key = TileAttachmentKeyV29 {
            root,
            family: TileAttachmentFamilyV29::MemoryAnchor,
            instance: access.instance.index(),
            row: access.record.anchor,
            field: TileAttachmentFieldV29::MemoryPosition,
            component: 0,
            part: 0,
        };
        let [position] = correspondence.attachment_range(key, budget)? else {
            return correspondence
                .source
                .missing("compiler access position cardinality");
        };
        let ProductionSourceOperationV18::Operation(operation) =
            correspondence.mapped_source_operation(position.location, budget)?
        else {
            return correspondence
                .source
                .missing("compiler access mapped operation");
        };
        let block = correspondence
            .inventory
            .block_for_id(function, access.block, budget)
            .map_err(|error| {
                ProductionSourceOwnedViewErrorV18::from(
                    fe2o3_pliron::CanonicalAnalysisScopeErrorV1::Inventory(error),
                )
            })?
            .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                "compiler access original block",
            ))?;
        let expected =
            immutable_memory_gap_v29(owner, access.block, access.operation, false, budget)?;
        if operation.block != block.coordinate || operation.operation as usize != expected {
            return correspondence
                .source
                .missing("compiler access lifecycle mapping");
        }
        let [pointer] = correspondence.attachment_range(
            TileAttachmentKeyV29 {
                field: TileAttachmentFieldV29::MemoryPointer,
                ..key
            },
            budget,
        )?
        else {
            return correspondence
                .source
                .missing("compiler access pointer cardinality");
        };
        if correspondence.attachment_value(root, pointer.location, budget)? != access.record.pointer
        {
            return correspondence
                .source
                .missing("compiler access pointer substitution");
        }
        if output.accesses.len() == output.accesses.capacity() {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        output.accesses.push(PendingCompilerEnumAccessV55 {
            operation: expected,
            ..*access
        });
    }
    // Lifecycle insertion preserves source order but may interleave new ops.
    // The closed check rejects duplicate mappings and every unclaimed use.
    Ok(output)
}
