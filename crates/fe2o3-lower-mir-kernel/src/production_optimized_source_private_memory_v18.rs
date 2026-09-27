// A lexical source/currentness/physical composition, not native Memory policy.
// The source entry-value binder is an independent mandatory dependency.
#[derive(Clone, Copy)]
struct SourcePrivateAllocationV18<'a> {
    root: usize,
    slot: usize,
    original: &'a ScopedSourceSlotV29,
    input: fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1,
    output: fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1,
    pointer: ValueId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SourcePrivateOperationKindV18 {
    Allocation,
    EntryWrite,
    Read { writer: usize },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SourcePrivateOperationV18 {
    root: usize,
    instance: usize,
    anchor: Option<usize>,
    input: fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1,
    output: fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1,
    allocation: usize,
    kind: SourcePrivateOperationKindV18,
    input_value: Option<ValueId>,
    output_value: Option<ValueId>,
}

pub(super) struct CheckedSourcePrivateMemoryV18<'scope> {
    physical: &'scope CheckedSourcePrivatePhysicalV18<'scope>,
    currentness: &'scope CheckedOptimizedSourceMemoryV18<'scope>,
    entries: &'scope ProductionCheckedSourceEntryWritesV18<'scope>,
    root: usize,
    first: usize,
    rows: &'scope [Option<SourcePrivateOperationV18>],
    floor: usize,
}

// This scope can lend physical/source allocation identities, but has no
// completed memory rows until a root also supplies currentness and checked RHSs.
pub(super) struct CheckedSourcePrivatePhysicalV18<'scope> {
    original: &'scope ProductionSourceCorrespondenceV18<'scope>,
    optimized: &'scope ProductionOptimizedSourceCorrespondenceV18<'scope>,
    physical: &'scope fe2o3_kernel_analysis::CheckedCanonicalKirPrivateMemoryV18<'scope, 'scope>,
    allocations: &'scope [Option<SourcePrivateAllocationV18<'scope>>],
    slot: usize,
    ledger: fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
    floor: usize,
}

fn source_private_header_v18<T>() -> Result<usize, ArgumentResourceV1> {
    argument_sum_v1(&[size_of::<T>(), size_of::<SourceOwnedResultV18<T>>()])
}

fn source_private_allocation_headers_v18() -> Result<usize, ArgumentResourceV1> {
    use fe2o3_kernel_analysis::{
        CanonicalKirDefinitionRefV1, CanonicalKirOperationRefV1, CanonicalKirPrivateMemoryAddressV1,
    };
    use fe2o3_kernel_ir::{CanonicalKirDefinitionCoordinateV1, CanonicalKirOperationCoordinateV1};
    argument_sum_v1(&[
        source_private_header_v18::<Vec<Option<SourcePrivateAllocationV18<'_>>>>()?,
        source_private_header_v18::<&fe2o3_kernel_analysis::CanonicalKirInventoryV18<'_>>()?,
        source_private_header_v18::<&ScopedModuleRootV29>()?,
        source_private_header_v18::<std::ops::Range<usize>>()?,
        source_private_header_v18::<std::iter::Enumerate<std::slice::Iter<'_, ScopedSourceSlotV29>>>(
        )?,
        source_private_header_v18::<Option<(usize, &ScopedSourceSlotV29)>>()?,
        source_private_header_v18::<(usize, &ScopedSourceSlotV29)>()?,
        source_private_header_v18::<&ScopedSourceSlotV29>()?,
        source_private_header_v18::<ScopedSlotRepresentationV29>()?,
        source_private_header_v18::<Result<usize, std::num::TryFromIntError>>()?,
        source_private_header_v18::<usize>()?,
        source_private_header_v18::<(ScopedAllocationIdentityV29, ScopedAllocationSourceV29)>()?,
        source_private_header_v18::<CanonicalKirOperationCoordinateV1>()?,
        source_private_header_v18::<CanonicalKirOperationCoordinateV1>()?,
        source_private_header_v18::<Option<CanonicalKirOperationCoordinateV1>>()?,
        source_private_header_v18::<ProductionOptimizedSourceAllocationV18<'_>>()?,
        source_private_header_v18::<Option<fe2o3_kernel_analysis::CanonicalKirOutputUseV1>>()?,
        source_private_header_v18::<CanonicalKirDefinitionCoordinateV1>()?,
        source_private_header_v18::<Option<CanonicalKirDefinitionCoordinateV1>>()?,
        source_private_header_v18::<&CanonicalKirDefinitionRefV1<'_>>()?,
        source_private_header_v18::<Option<ValueId>>()?,
        source_private_header_v18::<ValueId>()?,
        source_private_header_v18::<Option<usize>>()?,
        source_private_header_v18::<
            Result<Option<usize>, fe2o3_kernel_analysis::CanonicalKirInventoryErrorV1>,
        >()?,
        source_private_header_v18::<usize>()?,
        source_private_header_v18::<usize>()?,
        source_private_header_v18::<Option<&CanonicalKirPrivateMemoryAddressV1>>()?,
        source_private_header_v18::<&CanonicalKirPrivateMemoryAddressV1>()?,
        source_private_header_v18::<Option<&mut Option<SourcePrivateAllocationV18<'_>>>>()?,
        source_private_header_v18::<&mut Option<SourcePrivateAllocationV18<'_>>>()?,
        source_private_header_v18::<SourcePrivateAllocationV18<'_>>()?,
        source_private_header_v18::<Option<SourcePrivateAllocationV18<'_>>>()?,
        source_private_header_v18::<
            std::iter::Zip<
                std::slice::Iter<'_, CanonicalKirOperationRefV1<'_>>,
                std::slice::Iter<'_, Option<SourcePrivateAllocationV18<'_>>>,
            >,
        >()?,
        source_private_header_v18::<
            Option<(
                &CanonicalKirOperationRefV1<'_>,
                &Option<SourcePrivateAllocationV18<'_>>,
            )>,
        >()?,
        source_private_header_v18::<(
            &CanonicalKirOperationRefV1<'_>,
            &Option<SourcePrivateAllocationV18<'_>>,
        )>()?,
        source_private_header_v18::<bool>()?,
        source_private_header_v18::<()>()?,
        source_private_header_v18::<()>()?,
    ])
}

fn source_private_operation_headers_v18() -> Result<usize, ArgumentResourceV1> {
    argument_sum_v1(&[
        source_private_header_v18::<&fe2o3_kernel_analysis::CanonicalKirBlockRefV1<'_>>()?,
        source_private_header_v18::<Option<usize>>()?,
        source_private_header_v18::<usize>()?,
        source_private_header_v18::<Option<&fe2o3_kernel_analysis::CanonicalKirOperationRefV1<'_>>>(
        )?,
        source_private_header_v18::<&fe2o3_kernel_analysis::CanonicalKirOperationRefV1<'_>>()?,
        source_private_header_v18::<bool>()?,
    ])
}

fn source_private_physical_capture_headers_v18() -> Result<usize, ArgumentResourceV1> {
    type Build<'a> = (
        &'a ProductionSourceCorrespondenceV18<'a>,
        &'a ProductionOptimizedSourceCorrespondenceV18<'a>,
        fe2o3_kernel_analysis::CanonicalKirPrivateMemoryLimitsV1,
        usize,
        usize,
        usize,
    );
    argument_sum_v1(&[
        source_private_header_v18::<Build<'_>>()?,
        source_private_header_v18::<(&mut ArgumentBudgetV1<'_>, Build<'_>)>()?,
        source_private_header_v18::<(
            &CheckedSourcePrivatePhysicalV18<'_>,
            &mut ArgumentBudgetV1<'_>,
        )>()?,
        source_private_header_v18::<
            std::panic::AssertUnwindSafe<(
                &CheckedSourcePrivatePhysicalV18<'_>,
                &mut ArgumentBudgetV1<'_>,
            )>,
        >()?,
    ])
}

fn source_private_root_capture_headers_v18() -> Result<usize, ArgumentResourceV1> {
    type Build<'a> = (
        &'a CheckedSourcePrivatePhysicalV18<'a>,
        &'a CheckedOptimizedSourceMemoryV18<'a>,
        &'a ProductionCheckedSourceEntryWritesV18<'a>,
        usize,
        usize,
        usize,
        usize,
    );
    type Visit<'a> = (
        &'a CheckedSourcePrivatePhysicalV18<'a>,
        &'a CheckedOptimizedSourceMemoryV18<'a>,
        &'a ProductionCheckedSourceEntryWritesV18<'a>,
        usize,
        usize,
        &'a mut Vec<Option<SourcePrivateOperationV18>>,
    );
    argument_sum_v1(&[
        source_private_header_v18::<Build<'_>>()?,
        source_private_header_v18::<(&mut ArgumentBudgetV1<'_>, Build<'_>)>()?,
        source_private_header_v18::<(
            &CheckedSourcePrivateMemoryV18<'_>,
            &mut ArgumentBudgetV1<'_>,
        )>()?,
        source_private_header_v18::<
            std::panic::AssertUnwindSafe<(
                &CheckedSourcePrivateMemoryV18<'_>,
                &mut ArgumentBudgetV1<'_>,
            )>,
        >()?,
        // The outer visitor and its closed per-object unit attempt both
        // retain independent capture slots; neither cleanup helper pays them.
        source_private_header_v18::<Visit<'_>>()?,
        source_private_header_v18::<(Visit<'_>, &OptimizedSourceObjectV18<'_>)>()?,
        source_private_header_v18::<&OptimizedSourceObjectV18<'_>>()?,
        source_private_header_v18::<&CheckedSourcePrivatePhysicalV18<'_>>()?,
        source_private_header_v18::<&CheckedOptimizedSourceMemoryV18<'_>>()?,
        source_private_header_v18::<&ProductionCheckedSourceEntryWritesV18<'_>>()?,
    ])
}

impl CheckedSourcePrivatePhysicalV18<'_> {
    fn observe_custody(&self, budget: &ArgumentBudgetV1<'_>) -> SourceOwnedResultV18<()> {
        self.original.observe_custody(budget)?;
        if self.slot != std::ptr::from_ref(budget) as usize
            || self.ledger != budget.work_ledger_identity_v1()
            || budget.storage() < self.floor
        {
            self.original.source.cleanup.deny_refund();
            return Err(ArgumentResourceV1::Accounting.into());
        }
        Ok(())
    }

    fn check(&self, budget: &mut ArgumentBudgetV1<'_>) -> SourceOwnedResultV18<()> {
        self.original.retain_query((|| {
            self.observe_custody(budget)?;
            optimized_source_endpoints_v18(self.original, self.optimized, budget)?;
            let output = self.optimized.output_inventory(budget)?;
            budget.charge_work(2)?;
            if !self.physical.is_for(output) || self.allocations.len() != output.operations().len()
            {
                return self
                    .original
                    .source
                    .missing("source private physical scope changed its owner or roster");
            }
            Ok(())
        })())
    }
}

pub(super) fn with_source_private_physical_v18<'work, T, E>(
    original: &ProductionSourceCorrespondenceV18<'_>,
    optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    limits: fe2o3_kernel_analysis::CanonicalKirPrivateMemoryLimitsV1,
    budget: &mut ArgumentBudgetV1<'work>,
    consume: impl for<'scope> FnOnce(
        &CheckedSourcePrivatePhysicalV18<'scope>,
        &mut ArgumentBudgetV1<'work>,
    ) -> Result<T, E>,
) -> Result<T, E>
where
    E: From<ProductionSourceOwnedViewErrorV18>,
{
    optimized_source_endpoints_v18(original, optimized, budget)?;
    let floor = budget.storage();
    let capture = std::mem::size_of_val(&consume);
    let alignment = std::mem::align_of_val(&consume);
    let (physical, allocations, retained) =
        scoped_source_attempt_v29(original.source.cleanup, budget, floor, |budget| {
            original.retain_query((|| {
                budget.reserve_storage(argument_sum_v1(&[
                    capture,
                    alignment,
                    size_of::<CheckedSourcePrivatePhysicalV18<'_>>(),
                    source_private_header_v18::<
                        fe2o3_kernel_analysis::CheckedCanonicalKirPrivateMemoryV18<'_, '_>,
                    >()?,
                    source_private_header_v18::<
                        fe2o3_kernel_analysis::CanonicalKirPrivateMemoryStorageV1,
                    >()?,
                    source_private_header_v18::<(
                        fe2o3_kernel_analysis::CheckedCanonicalKirPrivateMemoryV18<'_, '_>,
                        fe2o3_kernel_analysis::CanonicalKirPrivateMemoryStorageV1,
                    )>()?,
                    source_private_header_v18::<(
                        fe2o3_kernel_analysis::CheckedCanonicalKirPrivateMemoryV18<'_, '_>,
                        Vec<Option<SourcePrivateAllocationV18<'_>>>,
                        usize,
                    )>()?,
                    size_of::<
                        std::thread::Result<
                            SourceOwnedResultV18<(
                                fe2o3_kernel_analysis::CheckedCanonicalKirPrivateMemoryV18<'_, '_>,
                                Vec<Option<SourcePrivateAllocationV18<'_>>>,
                                usize,
                            )>,
                        >,
                    >(),
                    size_of::<
                        Result<
                            (
                                fe2o3_kernel_analysis::CheckedCanonicalKirPrivateMemoryV18<'_, '_>,
                                fe2o3_kernel_analysis::CanonicalKirPrivateMemoryStorageV1,
                            ),
                            fe2o3_kernel_analysis::CanonicalKirPrivateMemoryErrorV1,
                        >,
                    >(),
                    source_private_allocation_headers_v18()?,
                    source_private_operation_headers_v18()?,
                    source_private_physical_capture_headers_v18()?,
                    size_of::<Result<T, E>>(),
                    size_of::<std::thread::Result<Result<T, E>>>(),
                    source_private_header_v18::<Option<SourceOwnedQueryFailureV18>>()?,
                    source_private_header_v18::<usize>()?,
                    source_private_header_v18::<()>()?,
                    source_private_header_v18::<()>()?,
                    physical_discard_headers_v29::<T, E>()?,
                    source_reference_cleanup_headers_v29()?,
                ])?)?;
                budget.charge_work(1 + SOURCE_REFERENCE_PAYLOAD_ATTEMPTS_V29)?;
                let output = optimized.output_inventory(budget)?;
                let (physical, receipt) =
                    fe2o3_kernel_analysis::check_canonical_kir_private_memory_v18(
                        output, limits, budget,
                    )
                    .map_err(source_private_memory_error_v18)?;
                // The analysis transfers unreserved backing. No query or caller
                // callback occurs before its receipt is reserved in this ledger.
                budget.reserve_storage(receipt.retained_storage())?;
                let allocations =
                    source_private_allocations_v18(original, optimized, &physical, budget)?;
                let retained = budget
                    .storage()
                    .checked_sub(floor)
                    .ok_or(ArgumentResourceV1::Accounting)?;
                Ok((physical, allocations, retained))
            })())
        })?;
    let view = CheckedSourcePrivatePhysicalV18 {
        original,
        optimized,
        physical: &physical,
        allocations: &allocations,
        slot: std::ptr::from_ref(budget) as usize,
        ledger: budget.work_ledger_identity_v1(),
        floor: budget.storage(),
    };
    let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| consume(&view, budget)));
    let first = original.source.guard.first.get();
    let postflight = if matches!(&caught, Ok(Ok(_))) {
        view.check(budget)
    } else {
        original.retain_query(view.observe_custody(budget))
    };
    drop(view);
    drop(allocations);
    drop(physical);
    source_owned_finish_callback_v18(
        caught,
        first,
        postflight,
        original.source.cleanup,
        budget,
        retained,
    )
}

fn source_private_memory_error_v18(
    error: fe2o3_kernel_analysis::CanonicalKirPrivateMemoryErrorV1,
) -> ProductionSourceOwnedViewErrorV18 {
    error.into()
}

fn source_private_operation_index_v18(
    inventory: &fe2o3_kernel_analysis::CanonicalKirInventoryV18<'_>,
    operation: fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<usize> {
    let block = optimized_source_block_row_v18(inventory, operation.block, budget)?;
    budget.charge_work(2)?;
    let ordinal = block
        .operations
        .start
        .checked_add(operation.operation as usize)
        .filter(|ordinal| *ordinal < block.operations.end)
        .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
            "source private operation interval",
        ))?;
    if inventory
        .operations()
        .get(ordinal)
        .is_none_or(|row| row.coordinate != operation)
    {
        return Err(ProductionSourceOwnedViewErrorV18::Binding(
            "source private operation coordinate",
        ));
    }
    Ok(ordinal)
}

fn source_private_allocations_v18<'a>(
    original: &'a ProductionSourceCorrespondenceV18<'a>,
    optimized: &'a ProductionOptimizedSourceCorrespondenceV18<'a>,
    physical: &fe2o3_kernel_analysis::CheckedCanonicalKirPrivateMemoryV18<'a, 'a>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<Vec<Option<SourcePrivateAllocationV18<'a>>>> {
    optimized_source_endpoints_v18(original, optimized, budget)?;
    let output = optimized.output_inventory(budget)?;
    budget.charge_work(1)?;
    if !physical.is_for(output) {
        return original
            .source
            .missing("source private physical owner differs");
    }
    let mut rows =
        emission_vec_v1(output.operations().len(), budget).map_err(immutable_memory_error_v29)?;
    budget.charge_work(output.operations().len())?;
    rows.resize(output.operations().len(), None);
    for root in 0..original.source.root_count(budget)? {
        let owner = original.source.root_row(root)?;
        for (slot, backing) in owner.source_slots.slots.iter().enumerate() {
            budget.charge_work(6)?;
            let ScopedSlotRepresentationV29::Object {
                schema,
                bytes,
                alignment,
            } = backing.representation
            else {
                return original
                    .source
                    .missing("source private entry family requires typed scalar object slots");
            };
            let bytes = usize::try_from(bytes).map_err(|_| ArgumentResourceV1::Arithmetic)?;
            if !matches!((backing.origin.identity, backing.origin.source),
                (ScopedAllocationIdentityV29::OriginalObject { .. },
                    ScopedAllocationSourceV29::OriginalObject { schema: source_schema, .. }) if source_schema == schema)
            {
                return original
                    .source
                    .missing("source private allocation lost original object identity");
            }
            let input = optimized_source_slot_input_v18(original, root, slot, budget)?;
            let allocation = optimized.allocation_for_slot_v18(root, slot, input, budget)?;
            let actual = allocation
                .output()
                .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                    "source private entry family requires retained allocation",
                ))?;
            if allocation.instance() != backing.instance.index() || allocation.count().is_some() {
                return original
                    .source
                    .missing("source private allocation changed instance or whole-object extent");
            }
            let pointer =
                allocation
                    .pointer()
                    .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                        "source private allocation output pointer is absent",
                    ))?;
            let definition = optimized_source_definition_row_v18(output, pointer, budget)?;
            let value = definition
                .value
                .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                    "source private allocation output pointer has no value",
                ))?;
            let definition_index = output
                .definition_index_for_value(actual.block.function, value, budget)
                .map_err(|error| {
                    ProductionSourceOwnedViewErrorV18::from(
                        fe2o3_pliron::CanonicalAnalysisScopeErrorV1::Inventory(error),
                    )
                })?
                .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                    "source private allocation result index",
                ))?;
            let ordinal = source_private_operation_index_v18(output, actual, budget)?;
            let address = physical.address(definition_index).ok_or(
                ProductionSourceOwnedViewErrorV18::Binding(
                    "source private allocation has no physical address",
                ),
            )?;
            budget.charge_work(7)?;
            if !physical.operation(ordinal)
                || address.allocation() != ordinal
                || address.length() != 1
                || address.offset() != 0
                || address.stride() != bytes
                || address.alignment() != alignment
            {
                return original
                    .source
                    .missing("source private allocation shape or physical origin differs");
            }
            let row = rows
                .get_mut(ordinal)
                .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                    "source private allocation census interval",
                ))?;
            if row
                .replace(SourcePrivateAllocationV18 {
                    root,
                    slot,
                    original: backing,
                    input,
                    output: actual,
                    pointer: value,
                })
                .is_some()
            {
                return original
                    .source
                    .missing("source private allocation has repeated original backing");
            }
        }
    }
    for (operation, row) in output.operations().iter().zip(&rows) {
        budget.charge_work(1)?;
        if matches!(operation.operation.kind, OperationKind::Alloca { .. }) != row.is_some() {
            return original
                .source
                .missing("source private allocation census is incomplete");
        }
    }
    Ok(rows)
}

fn source_private_current_slot_v18(
    currentness: &CheckedOptimizedSourceMemoryV18<'_>,
    original: &ProductionSourceCorrespondenceV18<'_>,
    optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    root: usize,
    object: &OptimizedSourceObjectV18<'_>,
    before: ValueId,
    after: ValueId,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<usize> {
    if !currentness.retained_value_footprint_v18(
        original,
        optimized,
        root,
        object.original.instance,
        object.input,
        0,
        before,
        Some((object.output, 0, after)),
        budget,
    )? {
        return original
            .source
            .missing("source private access has no original activation/currentness");
    }
    // Reuse the existing sorted complete currentness index, never scan slots
    // or all original accesses for each actual operation.
    let key = [
        object.input.block.function.0 as usize,
        object.input.block.block as usize,
        object.input.operation as usize,
        0,
    ];
    let ordinal = private_array_partition_v1(
        currentness.accesses,
        optimized_currentness_occurrence_key_v18,
        key,
        false,
        &mut SourceCorrespondenceWorkV18(budget),
    )?;
    budget.charge_work(2)?;
    let row = currentness
        .accesses
        .get(ordinal)
        .filter(|row| optimized_currentness_occurrence_key_v18(row) == key)
        .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
            "source private currentness exact occurrence",
        ))?;
    Ok(currentness
        .original
        .pending
        .accesses
        .get(row.original)
        .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
            "source private currentness original row",
        ))?
        .physical
        .slot)
}

fn source_private_entry_destination_v18(
    allocation: &SourcePrivateAllocationV18<'_>,
    object: &OptimizedSourceObjectV18<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<()> {
    budget.charge_work(12)?;
    let ScopedAllocationIdentityV29::OriginalObject {
        local,
        generation: 0,
    } = allocation.original.origin.identity
    else {
        return Err(ProductionSourceOwnedViewErrorV18::Binding(
            "source private entry requires original generation zero",
        ));
    };
    let ScopedSlotRepresentationV29::Object { schema, .. } = allocation.original.representation
    else {
        return Err(ProductionSourceOwnedViewErrorV18::Binding(
            "source private entry has no object schema",
        ));
    };
    let ScopedObjectRoleV29::WriteValue {
        destination,
        value:
            ScopedObjectValueOriginV29::Original(ScopedMemoryStoreSourceV29::EntryArgument {
                local: source_local,
                ty,
            }),
    } = object.original.source.role
    else {
        return Err(ProductionSourceOwnedViewErrorV18::Binding(
            "source private non-entry write remains unresolved",
        ));
    };
    if source_local.index() != local
        || ty != allocation.original.origin.semantic_type
        || object.original.instance != allocation.original.instance.index()
        || object.original.anchor.source.is_some()
        || destination.object
            != (ScopedObjectIdentityV29::Local {
                instance: allocation.original.instance,
                local: source_local,
                generation: 0,
            })
        || destination.source
            != (ScopedObjectSourceV29::EntryArgument {
                local: source_local,
            })
        || destination.root_type != ty
        || destination.projected_type != ty
        || destination.root_schema != schema
        || destination.projected_schema != schema
        || destination.path.count != 0
        || destination.source_path.count != 0
    {
        return Err(ProductionSourceOwnedViewErrorV18::Binding(
            "source private entry destination differs from original slot",
        ));
    }
    Ok(())
}

fn source_private_access_row_v18(
    core: &CheckedSourcePrivatePhysicalV18<'_>,
    currentness: &CheckedOptimizedSourceMemoryV18<'_>,
    entries: &ProductionCheckedSourceEntryWritesV18<'_>,
    root: usize,
    object: &OptimizedSourceObjectV18<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<(usize, SourcePrivateOperationV18)> {
    let original = core.original;
    let output = core.optimized.output_inventory(budget)?;
    // Source roles retain original semantic identities, but pointer/RHS IDs
    // must come from the authenticated relocated input payload, not sidecar IDs.
    let (before, after, values) = match (object.original.actual.operation, object.actual.operation)
    {
        (
            ScopedObjectOperationV29::ReadValue {
                address: before, ..
            },
            ScopedObjectOperationV29::ReadValue { address: after, .. },
        ) => (
            before,
            after,
            (object.original.actual.result, object.actual.result),
        ),
        (
            ScopedObjectOperationV29::WriteValue {
                address: before,
                value: input_rhs,
                ..
            },
            ScopedObjectOperationV29::WriteValue {
                address: after,
                value: output_rhs,
                ..
            },
        ) => (before, after, (Some(input_rhs), Some(output_rhs))),
        _ => {
            return original
                .source
                .missing("source private whole scalar access family remains unresolved");
        }
    };
    let ordinal = source_private_operation_index_v18(output, object.output, budget)?;
    let definition = output
        .definition_index_for_value(object.output.block.function, after, budget)
        .map_err(|error| {
            ProductionSourceOwnedViewErrorV18::from(
                fe2o3_pliron::CanonicalAnalysisScopeErrorV1::Inventory(error),
            )
        })?
        .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
            "source private access pointer definition",
        ))?;
    let address =
        core.physical
            .address(definition)
            .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                "source private access pointer has no exact physical allocation",
            ))?;
    let allocation = core
        .allocations
        .get(address.allocation())
        .and_then(Option::as_ref)
        .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
            "source private access allocation lacks original backing",
        ))?;
    let slot = source_private_current_slot_v18(
        currentness,
        original,
        core.optimized,
        root,
        object,
        before,
        after,
        budget,
    )?;
    budget.charge_work(7)?;
    if !core.physical.operation(ordinal)
        || allocation.root != root
        || allocation.slot != slot
        || allocation.original.instance.index() != object.original.instance
        || allocation.pointer != after
        || address.length() != 1
        || address.offset() != 0
    {
        return original
            .source
            .missing("source private access source slot and physical allocation differ");
    }
    let kind = match (object.original.actual.operation, object.actual.operation) {
        (
            ScopedObjectOperationV29::ReadValue { .. },
            ScopedObjectOperationV29::ReadValue { .. },
        ) => {
            let writer = core
                .physical
                .latest_stores()
                .get(ordinal)
                .copied()
                .flatten()
                .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                    "source private read lacks an exact intersected Store",
                ))?;
            SourcePrivateOperationKindV18::Read { writer }
        }
        (
            ScopedObjectOperationV29::WriteValue {
                value: input_rhs, ..
            },
            ScopedObjectOperationV29::WriteValue {
                value: output_rhs, ..
            },
        ) => {
            source_private_entry_destination_v18(allocation, object, budget)?;
            let definition = output
                .definition_for_value(object.output.block.function, output_rhs, budget)
                .map_err(|error| {
                    ProductionSourceOwnedViewErrorV18::from(
                        fe2o3_pliron::CanonicalAnalysisScopeErrorV1::Inventory(error),
                    )
                })?
                .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                    "source private entry RHS definition",
                ))?;
            let scalar = kir_semantic_scalar_v1(definition.ty).ok_or(
                ProductionSourceOwnedViewErrorV18::Binding(
                    "source private entry RHS is not scalar",
                ),
            )?;
            let ScopedSlotRepresentationV29::Object { schema, .. } =
                allocation.original.representation
            else {
                return original
                    .source
                    .missing("source private entry allocation schema absent");
            };
            entries.require(
                object.original.instance,
                object.original.row,
                object.input,
                object.output,
                input_rhs,
                output_rhs,
                scalar,
                schema,
                budget,
            )?;
            SourcePrivateOperationKindV18::EntryWrite
        }
        _ => unreachable!(),
    };
    Ok((
        ordinal,
        SourcePrivateOperationV18 {
            root,
            instance: object.original.instance,
            anchor: Some(object.original.row),
            input: object.input,
            output: object.output,
            allocation: address.allocation(),
            kind,
            input_value: values.0,
            output_value: values.1,
        },
    ))
}

fn source_private_root_rows_v18(
    core: &CheckedSourcePrivatePhysicalV18<'_>,
    currentness: &CheckedOptimizedSourceMemoryV18<'_>,
    entries: &ProductionCheckedSourceEntryWritesV18<'_>,
    root: usize,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<(usize, Vec<Option<SourcePrivateOperationV18>>)> {
    core.check(budget)?;
    currentness.check_scope_v18(core.original, core.optimized, root, budget)?;
    entries.check_for(core.original, core.optimized, root, budget)?;
    let function = optimized_source_root_function_v18(core.original, core.optimized, root, budget)?;
    let first = function.operations.start;
    let output = core.optimized.output_inventory(budget)?;
    let actual = output.operations().get(function.operations.clone()).ok_or(
        ProductionSourceOwnedViewErrorV18::Binding("source private root operation interval"),
    )?;
    let mut rows = emission_vec_v1(actual.len(), budget).map_err(source_emission_error_v18)?;
    budget.charge_work(actual.len())?;
    rows.resize(actual.len(), None);
    for (relative, row) in rows.iter_mut().enumerate() {
        budget.charge_work(2)?;
        let ordinal = first
            .checked_add(relative)
            .ok_or(ArgumentResourceV1::Arithmetic)?;
        if let Some(allocation) = core.allocations.get(ordinal).and_then(Option::as_ref) {
            if allocation.root != root {
                return core
                    .original
                    .source
                    .missing("source private root contains a foreign allocation");
            }
            *row = Some(SourcePrivateOperationV18 {
                root,
                instance: allocation.original.instance.index(),
                anchor: None,
                input: allocation.input,
                output: allocation.output,
                allocation: ordinal,
                kind: SourcePrivateOperationKindV18::Allocation,
                input_value: None,
                output_value: None,
            });
        }
    }
    visit_optimized_source_objects_v18(
        core.original,
        core.optimized,
        root,
        budget,
        |object, budget| {
            // The census callback must leave its exact input floor intact. Queries
            // retain no object outside this closed unit scratch attempt; only the
            // prepaid dense operation row is filled before the scratch is dropped.
            let floor = budget.storage();
            scoped_source_attempt_v29(core.original.source.cleanup, budget, floor, |budget| {
                let (ordinal, row) = source_private_access_row_v18(
                    core,
                    currentness,
                    entries,
                    root,
                    &object,
                    budget,
                )?;
                let relative = ordinal
                    .checked_sub(first)
                    .ok_or(ArgumentResourceV1::Arithmetic)?;
                budget.charge_work(2)?;
                let output =
                    rows.get_mut(relative)
                        .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                            "source private access belongs to another root",
                        ))?;
                if output.replace(row).is_some() {
                    return core
                        .original
                        .source
                        .missing("source private operation has duplicate source roles");
                }
                Ok::<(), ProductionSourceOwnedViewErrorV18>(())
            })?;
            budget.release_storage(
                budget
                    .storage()
                    .checked_sub(floor)
                    .ok_or(ArgumentResourceV1::Accounting)?,
            )?;
            Ok(())
        },
    )?;
    for (relative, (operation, row)) in actual.iter().zip(&rows).enumerate() {
        budget.charge_work(3)?;
        let ordinal = first
            .checked_add(relative)
            .ok_or(ArgumentResourceV1::Arithmetic)?;
        if core.physical.operation(ordinal) != row.is_some() {
            return core
                .original
                .source
                .missing("source private physical/source operation census is incomplete");
        }
        let Some(row) = row else {
            continue;
        };
        if row.root != root || row.output != operation.coordinate {
            return core
                .original
                .source
                .missing("source private operation moved to a different output");
        }
        if let SourcePrivateOperationKindV18::Read { writer } = row.kind {
            let writer = writer
                .checked_sub(first)
                .and_then(|index| rows.get(index))
                .and_then(Option::as_ref)
                .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                    "source private read-from Store lacks a completed source entry",
                ))?;
            budget.charge_work(3)?;
            if writer.kind != SourcePrivateOperationKindV18::EntryWrite
                || writer.root != root
                || writer.allocation != row.allocation
            {
                return core.original.source.missing(
                    "source private read-from Store has another source allocation or role",
                );
            }
        }
    }
    Ok((first, rows))
}

fn source_private_root_headers_v18() -> Result<usize, ArgumentResourceV1> {
    use fe2o3_kernel_analysis::{
        CanonicalKirDefinitionRefV1, CanonicalKirOperationRefV1, CanonicalKirPrivateMemoryAddressV1,
    };
    argument_sum_v1(&[
        source_private_header_v18::<Vec<Option<SourcePrivateOperationV18>>>()?,
        source_private_header_v18::<(usize, Vec<Option<SourcePrivateOperationV18>>)>()?,
        source_private_header_v18::<&fe2o3_kernel_analysis::CanonicalKirFunctionRefV1<'_>>()?,
        source_private_header_v18::<&fe2o3_kernel_analysis::CanonicalKirInventoryV18<'_>>()?,
        source_private_header_v18::<&fe2o3_kernel_analysis::CanonicalKirInventoryV18<'_>>()?,
        source_private_header_v18::<Option<&[CanonicalKirOperationRefV1<'_>]>>()?,
        source_private_header_v18::<&[CanonicalKirOperationRefV1<'_>]>()?,
        source_private_header_v18::<std::ops::Range<usize>>()?,
        source_private_header_v18::<
            std::iter::Enumerate<std::slice::IterMut<'_, Option<SourcePrivateOperationV18>>>,
        >()?,
        source_private_header_v18::<Option<(usize, &mut Option<SourcePrivateOperationV18>)>>()?,
        source_private_header_v18::<(usize, &mut Option<SourcePrivateOperationV18>)>()?,
        source_private_header_v18::<Option<&Option<SourcePrivateAllocationV18<'_>>>>()?,
        source_private_header_v18::<Option<&SourcePrivateAllocationV18<'_>>>()?,
        source_private_header_v18::<&SourcePrivateAllocationV18<'_>>()?,
        source_private_header_v18::<SourcePrivateOperationV18>()?,
        source_private_header_v18::<Option<SourcePrivateOperationV18>>()?,
        source_private_header_v18::<(usize, SourcePrivateOperationV18)>()?,
        source_private_header_v18::<(ScopedObjectOperationV29, ScopedObjectOperationV29)>()?,
        source_private_header_v18::<(ValueId, ValueId, (Option<ValueId>, Option<ValueId>))>()?,
        source_private_header_v18::<(Option<ValueId>, Option<ValueId>)>()?,
        source_private_header_v18::<ValueId>()?,
        source_private_header_v18::<ValueId>()?,
        source_private_header_v18::<ValueId>()?,
        source_private_header_v18::<ValueId>()?,
        source_private_header_v18::<Option<usize>>()?,
        source_private_header_v18::<usize>()?,
        source_private_header_v18::<Option<usize>>()?,
        source_private_header_v18::<usize>()?,
        source_private_header_v18::<Option<usize>>()?,
        source_private_header_v18::<usize>()?,
        source_private_header_v18::<
            Result<Option<usize>, fe2o3_kernel_analysis::CanonicalKirInventoryErrorV1>,
        >()?,
        source_private_header_v18::<Option<&CanonicalKirPrivateMemoryAddressV1>>()?,
        source_private_header_v18::<&CanonicalKirPrivateMemoryAddressV1>()?,
        source_private_header_v18::<Option<&Option<usize>>>()?,
        source_private_header_v18::<Option<Option<usize>>>()?,
        source_private_header_v18::<SourcePrivateOperationKindV18>()?,
        source_private_header_v18::<
            Result<
                Option<&CanonicalKirDefinitionRefV1<'_>>,
                fe2o3_kernel_analysis::CanonicalKirInventoryErrorV1,
            >,
        >()?,
        source_private_header_v18::<Option<&CanonicalKirDefinitionRefV1<'_>>>()?,
        source_private_header_v18::<&CanonicalKirDefinitionRefV1<'_>>()?,
        source_private_header_v18::<Option<ProductionSemanticScalarTypeV2>>()?,
        source_private_header_v18::<ProductionSemanticScalarTypeV2>()?,
        source_private_header_v18::<ScopedSlotRepresentationV29>()?,
        source_private_header_v18::<ScopedAllocationIdentityV29>()?,
        source_private_header_v18::<ScopedObjectRoleV29>()?,
        source_private_header_v18::<(SemanticLocalIdV1, SemanticTypeIdV1)>()?,
        source_private_header_v18::<ScopedObjectEndpointV29>()?,
        source_private_header_v18::<ScopedObjectIdentityV29>()?,
        source_private_header_v18::<ScopedObjectSourceV29>()?,
        source_private_header_v18::<[usize; 4]>()?,
        source_private_header_v18::<SourceCorrespondenceWorkV18<'_, '_>>()?,
        source_private_header_v18::<Option<&OptimizedSourceMemoryOccurrenceV18>>()?,
        source_private_header_v18::<&OptimizedSourceMemoryOccurrenceV18>()?,
        source_private_header_v18::<Option<&PendingSourceMemoryAccessV29>>()?,
        source_private_header_v18::<&PendingSourceMemoryAccessV29>()?,
        source_private_header_v18::<Option<&mut Option<SourcePrivateOperationV18>>>()?,
        source_private_header_v18::<&mut Option<SourcePrivateOperationV18>>()?,
        source_private_header_v18::<
            std::iter::Enumerate<
                std::iter::Zip<
                    std::slice::Iter<'_, CanonicalKirOperationRefV1<'_>>,
                    std::slice::Iter<'_, Option<SourcePrivateOperationV18>>,
                >,
            >,
        >()?,
        source_private_header_v18::<
            Option<(
                usize,
                (
                    &CanonicalKirOperationRefV1<'_>,
                    &Option<SourcePrivateOperationV18>,
                ),
            )>,
        >()?,
        source_private_header_v18::<(
            usize,
            (
                &CanonicalKirOperationRefV1<'_>,
                &Option<SourcePrivateOperationV18>,
            ),
        )>()?,
        source_private_header_v18::<Option<&Option<SourcePrivateOperationV18>>>()?,
        source_private_header_v18::<Option<&SourcePrivateOperationV18>>()?,
        source_private_header_v18::<&SourcePrivateOperationV18>()?,
        source_private_header_v18::<&SourcePrivateOperationV18>()?,
        source_private_header_v18::<()>()?,
        source_private_header_v18::<()>()?,
    ])
}

impl CheckedSourcePrivateMemoryV18<'_> {
    fn observe_custody(&self, budget: &ArgumentBudgetV1<'_>) -> SourceOwnedResultV18<()> {
        self.physical.observe_custody(budget)?;
        self.currentness.observe_custody(budget)?;
        // This higher floor was captured after authenticating the entry
        // context. Its immutable lower floor and ledger cannot change here.
        if budget.storage() < self.floor {
            self.physical.original.source.cleanup.deny_refund();
            return Err(ArgumentResourceV1::Accounting.into());
        }
        Ok(())
    }

    fn check(&self, budget: &mut ArgumentBudgetV1<'_>) -> SourceOwnedResultV18<()> {
        self.physical.original.retain_query((|| {
            self.observe_custody(budget)?;
            self.physical.check(budget)?;
            self.currentness.check_scope_v18(
                self.physical.original,
                self.physical.optimized,
                self.root,
                budget,
            )?;
            self.entries.check_for(
                self.physical.original,
                self.physical.optimized,
                self.root,
                budget,
            )
        })())
    }
}

#[cfg(test)]
impl CheckedSourcePrivatePhysicalV18<'_> {
    pub(super) fn test_replaced_allocation_v18(
        &self,
        root: usize,
        currentness: &CheckedOptimizedSourceMemoryV18<'_>,
        entries: &ProductionCheckedSourceEntryWritesV18<'_>,
        fault: u8,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<()> {
        self.check(budget)?;
        let floor = budget.storage();
        let result =
            scoped_source_attempt_v29(self.original.source.cleanup, budget, floor, |budget| {
                budget.reserve_storage(argument_sum_v1(&[
                    source_private_header_v18::<Vec<Option<SourcePrivateAllocationV18<'_>>>>()?,
                    source_private_header_v18::<CheckedSourcePrivatePhysicalV18<'_>>()?,
                    source_private_header_v18::<Option<usize>>()?,
                    source_private_header_v18::<Option<&mut SourcePrivateAllocationV18<'_>>>()?,
                    source_private_header_v18::<&mut SourcePrivateAllocationV18<'_>>()?,
                    source_private_header_v18::<()>()?,
                ])?)?;
                let mut allocations = emission_vec_v1(self.allocations.len(), budget)
                    .map_err(source_emission_error_v18)?;
                budget.charge_work(argument_product_v1(2, self.allocations.len())?)?;
                allocations.extend_from_slice(self.allocations);
                let index = allocations
                    .iter()
                    .position(|row| row.is_some_and(|row| row.root == root))
                    .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                        "private mutation root has no allocation",
                    ))?;
                if fault == 3 {
                    allocations[index] = None;
                } else {
                    let row = allocations[index].as_mut().unwrap();
                    match fault {
                        0 => row.slot = usize::MAX,
                        1 => row.root = usize::MAX,
                        2 => row.pointer = ValueId(u32::MAX),
                        _ => panic!("unknown private allocation mutation"),
                    }
                }
                let copied = CheckedSourcePrivatePhysicalV18 {
                    original: self.original,
                    optimized: self.optimized,
                    physical: self.physical,
                    allocations: &allocations,
                    slot: self.slot,
                    ledger: self.ledger,
                    floor: budget.storage(),
                };
                // Copied internal rows are not an authentic published owner. This
                // tests the independent joins, not a new constructor or authority.
                copied.with_root_memory_v18(root, currentness, entries, budget, |_, _| {
                    panic!("substituted source allocation reached completed memory rows")
                })
            });
        if result.is_ok() {
            budget.release_storage(
                budget
                    .storage()
                    .checked_sub(floor)
                    .ok_or(ArgumentResourceV1::Accounting)?,
            )?;
        }
        result
    }
}

impl CheckedSourcePrivatePhysicalV18<'_> {
    pub(super) fn with_root_memory_v18<'work, T, E>(
        &self,
        root: usize,
        currentness: &CheckedOptimizedSourceMemoryV18<'_>,
        entries: &ProductionCheckedSourceEntryWritesV18<'_>,
        budget: &mut ArgumentBudgetV1<'work>,
        consume: impl for<'scope> FnOnce(
            &CheckedSourcePrivateMemoryV18<'scope>,
            &mut ArgumentBudgetV1<'work>,
        ) -> Result<T, E>,
    ) -> Result<T, E>
    where
        E: From<ProductionSourceOwnedViewErrorV18>,
    {
        self.check(budget)?;
        currentness.check_scope_v18(self.original, self.optimized, root, budget)?;
        entries.check_for(self.original, self.optimized, root, budget)?;
        let floor = budget.storage();
        let capture = std::mem::size_of_val(&consume);
        let alignment = std::mem::align_of_val(&consume);
        let (first, rows, retained) =
            scoped_source_attempt_v29(self.original.source.cleanup, budget, floor, |budget| {
                self.original.retain_query((|| {
                    budget.reserve_storage(argument_sum_v1(&[
                        capture,
                        alignment,
                        size_of::<CheckedSourcePrivateMemoryV18<'_>>(),
                        source_private_root_headers_v18()?,
                        source_private_operation_headers_v18()?,
                        source_private_root_capture_headers_v18()?,
                        source_private_header_v18::<(
                            usize,
                            Vec<Option<SourcePrivateOperationV18>>,
                            usize,
                        )>()?,
                        size_of::<
                            std::thread::Result<
                                SourceOwnedResultV18<(
                                    usize,
                                    Vec<Option<SourcePrivateOperationV18>>,
                                    usize,
                                )>,
                            >,
                        >(),
                        size_of::<Result<T, E>>(),
                        size_of::<std::thread::Result<Result<T, E>>>(),
                        source_private_header_v18::<Option<SourceOwnedQueryFailureV18>>()?,
                        source_private_header_v18::<usize>()?,
                        source_private_header_v18::<()>()?,
                        source_private_header_v18::<()>()?,
                        physical_discard_headers_v29::<T, E>()?,
                        source_reference_cleanup_headers_v29()?,
                    ])?)?;
                    #[cfg(test)]
                    budget.reserve_storage(source_private_header_v18::<[usize; 3]>()?)?;
                    budget.charge_work(1 + SOURCE_REFERENCE_PAYLOAD_ATTEMPTS_V29)?;
                    let (first, rows) =
                        source_private_root_rows_v18(self, currentness, entries, root, budget)?;
                    let retained = budget
                        .storage()
                        .checked_sub(floor)
                        .ok_or(ArgumentResourceV1::Accounting)?;
                    Ok((first, rows, retained))
                })())
            })?;
        let view = CheckedSourcePrivateMemoryV18 {
            physical: self,
            currentness,
            entries,
            root,
            first,
            rows: &rows,
            floor: budget.storage(),
        };
        let caught =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| consume(&view, budget)));
        let first = self.original.source.guard.first.get();
        let postflight = if matches!(&caught, Ok(Ok(_))) {
            view.check(budget)
        } else {
            self.original.retain_query(view.observe_custody(budget))
        };
        drop(view);
        drop(rows);
        source_owned_finish_callback_v18(
            caught,
            first,
            postflight,
            self.original.source.cleanup,
            budget,
            retained,
        )
    }
}

#[cfg(test)]
impl CheckedSourcePrivateMemoryV18<'_> {
    pub(super) fn test_counts_v18(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<[usize; 3]> {
        self.check(budget)?;
        let mut counts = [0usize; 3];
        for row in self.rows.iter().flatten() {
            budget.charge_work(2)?;
            let kind = match row.kind {
                SourcePrivateOperationKindV18::Allocation => 0,
                SourcePrivateOperationKindV18::EntryWrite => 1,
                SourcePrivateOperationKindV18::Read { .. } => 2,
            };
            counts[kind] = counts[kind]
                .checked_add(1)
                .ok_or(ArgumentResourceV1::Arithmetic)?;
        }
        Ok(counts)
    }

    pub(super) fn test_check_v18(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<()> {
        self.check(budget)
    }

    pub(super) fn test_replaced_allocation_v18(
        &self,
        fault: u8,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<()> {
        self.physical.test_replaced_allocation_v18(
            self.root,
            self.currentness,
            self.entries,
            fault,
            budget,
        )
    }

    pub(super) fn test_nested_exit_v18(
        &self,
        mode: u8,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<()> {
        self.check(budget)?;
        let floor = budget.storage();
        let headers = argument_sum_v1(&[
            source_private_header_v18::<std::thread::Result<SourceOwnedResultV18<()>>>()?,
            source_private_header_v18::<Vec<u8>>()?,
            source_private_header_v18::<usize>()?,
            source_private_header_v18::<()>()?,
        ])?;
        budget.reserve_storage(headers)?;
        let held = budget.storage();
        if mode == 4 {
            let bytes = size_of::<Vec<u8>>() + 29;
            let result = self.physical.with_root_memory_v18(
                self.root,
                self.currentness,
                self.entries,
                budget,
                |_, budget| {
                    budget.reserve_storage(bytes)?;
                    Ok::<_, ProductionSourceOwnedViewErrorV18>(vec![7u8; 29])
                },
            )?;
            assert_eq!(result, vec![7u8; 29]);
            assert_eq!(
                budget.storage(),
                held + bytes,
                "consumer backing is not constructor scratch"
            );
            drop(result);
            budget.release_storage(bytes)?;
        } else {
            let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                self.physical.with_root_memory_v18(
                    self.root,
                    self.currentness,
                    self.entries,
                    budget,
                    |_, budget| -> SourceOwnedResultV18<()> {
                        if mode >= 2 {
                            budget.release_storage(1)?;
                        }
                        if mode % 2 == 1 {
                            std::panic::panic_any("private memory selected unwind");
                        }
                        Err(ProductionSourceOwnedViewErrorV18::Binding(
                            "private memory selected error",
                        ))
                    },
                )
            }));
            if mode % 2 == 1 {
                let payload = caught.expect_err("selected unwind must resume unchanged");
                assert_eq!(
                    payload.downcast_ref::<&'static str>(),
                    Some(&"private memory selected unwind")
                );
                drop(payload);
            } else {
                assert!(matches!(
                    caught,
                    Ok(Err(ProductionSourceOwnedViewErrorV18::Binding(
                        "private memory selected error"
                    )))
                ));
            }
            if mode >= 2 {
                let stopped = (budget.work(), budget.storage());
                assert!(matches!(
                    self.check(budget),
                    Err(ProductionSourceOwnedViewErrorV18::Resource(
                        ArgumentResourceV1::Accounting
                    ))
                ));
                assert_eq!((budget.work(), budget.storage()), stopped);
                assert!(
                    budget.storage() > held,
                    "lost child custody cannot refund its retained backing"
                );
                return Ok(());
            }
            assert_eq!(budget.storage(), held);
            self.check(budget)?;
        }
        budget.release_storage(headers)?;
        assert_eq!(budget.storage(), floor);
        Ok(())
    }
}

// Only the lower native-completion constructor consumes these private joins.
// Its fixed scope headers pay the borrowed query/census frames once.
impl CheckedSourcePrivatePhysicalV18<'_> {
    pub(super) fn native_physical_v18(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<&fe2o3_kernel_analysis::CheckedCanonicalKirPrivateMemoryV18<'_, '_>>
    {
        self.check(budget)?;
        Ok(self.physical)
    }

    pub(super) fn check_native_source_subject_v18(
        &self,
        original: &ProductionSourceCorrespondenceV18<'_>,
        optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<()> {
        self.original.retain_query((|| {
            self.check(budget)?;
            budget.charge_work(2)?;
            if !std::ptr::eq(original, self.original) || !std::ptr::eq(optimized, self.optimized) {
                return self
                    .original
                    .source
                    .missing("private native source subject changed");
            }
            Ok(())
        })())
    }

    pub(super) fn observe_native_source_custody_v18(
        &self,
        budget: &ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<()> {
        self.original.retain_query(self.observe_custody(budget))
    }
}

impl CheckedSourcePrivateMemoryV18<'_> {
    pub(super) fn mark_native_source_operations_v18(
        &self,
        original: &ProductionSourceCorrespondenceV18<'_>,
        optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
        root: usize,
        coverage: &mut [bool],
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<()> {
        self.physical.original.retain_query((|| {
            self.check(budget)?;
            self.physical
                .check_native_source_subject_v18(original, optimized, budget)?;
            budget.charge_work(3)?;
            if root != self.root || coverage.len() != self.physical.allocations.len() {
                return original
                    .source
                    .missing("private native root coverage subject changed");
            }
            for (offset, row) in self.rows.iter().enumerate() {
                budget.charge_work(3)?;
                let index = self
                    .first
                    .checked_add(offset)
                    .ok_or(ArgumentResourceV1::Arithmetic)?;
                let physical = self.physical.physical.operation(index);
                if physical != row.is_some() {
                    return original
                        .source
                        .missing("private native root coverage is incomplete");
                }
                let Some(row) = row else {
                    continue;
                };
                if row.root != root {
                    return original
                        .source
                        .missing("private native root coverage changed source root");
                }
                let seen =
                    coverage
                        .get_mut(index)
                        .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                            "private native operation outside output census",
                        ))?;
                if *seen {
                    return original
                        .source
                        .missing("private native operation completed twice");
                }
                *seen = true;
            }
            self.check(budget)
        })())
    }
}

pub(super) fn native_source_memory_bridge_headers_v18() -> Result<usize, ArgumentResourceV1> {
    type Mark<'a> = (
        &'a CheckedSourcePrivateMemoryV18<'a>,
        &'a ProductionSourceCorrespondenceV18<'a>,
        &'a ProductionOptimizedSourceCorrespondenceV18<'a>,
        usize,
        &'a mut [bool],
        &'a mut ArgumentBudgetV1<'a>,
    );
    argument_sum_v1(&[
        source_private_header_v18::<
            &fe2o3_kernel_analysis::CheckedCanonicalKirPrivateMemoryV18<'_, '_>,
        >()?,
        source_private_header_v18::<Mark<'_>>()?,
        source_private_header_v18::<
            std::iter::Enumerate<std::slice::Iter<'_, Option<SourcePrivateOperationV18>>>,
        >()?,
        source_private_header_v18::<Option<(usize, &Option<SourcePrivateOperationV18>)>>()?,
        source_private_header_v18::<(usize, &Option<SourcePrivateOperationV18>)>()?,
        source_private_header_v18::<&Option<SourcePrivateOperationV18>>()?,
        source_private_header_v18::<&SourcePrivateOperationV18>()?,
        source_private_header_v18::<Option<&mut bool>>()?,
        source_private_header_v18::<&mut bool>()?,
        source_private_header_v18::<Option<usize>>()?,
        source_private_header_v18::<usize>()?,
        source_private_header_v18::<bool>()?,
        source_private_header_v18::<()>()?,
    ])
}
