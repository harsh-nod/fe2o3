// A lexical source/currentness/physical composition, not native Memory policy.
// The source entry-value binder is an independent mandatory dependency.
include!("production_source_private_spill_v25.rs");
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
    SourceWrite,
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
    with_source_private_physical_profile_v25::<false, T, E>(
        original, optimized, limits, budget, consume,
    )
}

pub(super) fn with_source_private_physical_profile_v25<'work, const SPILLS: bool, T, E>(
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
    let mut consume = Some(consume);
    let capture = std::mem::size_of_val(&consume);
    let alignment = std::mem::align_of_val(&consume);
    let prepared = scoped_source_attempt_v29(original.source.cleanup, budget, floor, |budget| {
        let floor = budget.storage();
        original
            .retain_query((|| {
                let disposal = source_owned_finish_preflight_v26::<T, E>(budget)?;
                budget.reserve_storage(argument_sum_v1(&[
                    disposal,
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
                    if SPILLS {
                        source_private_spill_shape_headers_v25()?
                    } else {
                        0
                    },
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
                let allocations = source_private_allocations_profile_v25::<SPILLS>(
                    original, optimized, &physical, budget,
                )?;
                let retained = budget
                    .storage()
                    .checked_sub(floor)
                    .ok_or(ArgumentResourceV1::Accounting)?;
                Ok((physical, allocations, retained))
            })())
            .inspect_err(|_| {
                source_reference_discard_v29(consume.take());
            })
    });
    let (physical, allocations, retained) = match prepared {
        Ok(prepared) => prepared,
        Err(error) => {
            source_reference_discard_v29(consume);
            return Err(error.into());
        }
    };
    let consume = consume.expect("private preparation retained its callback");
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
    let block = source_block_row_v18(inventory, operation.block, budget)?;
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
    source_private_allocations_profile_v25::<false>(original, optimized, physical, budget)
}

fn source_private_allocations_profile_v25<'a, const SPILLS: bool>(
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
            let (bytes, alignment) = match backing.representation {
                ScopedSlotRepresentationV29::Object {
                    schema,
                    bytes,
                    alignment,
                } => {
                    if !matches!((backing.origin.identity, backing.origin.source),
                        (ScopedAllocationIdentityV29::OriginalObject { .. },
                            ScopedAllocationSourceV29::OriginalObject { schema: source_schema, .. }) if source_schema == schema)
                    {
                        return original
                            .source
                            .missing("source private allocation lost original object identity");
                    }
                    (
                        usize::try_from(bytes).map_err(|_| ArgumentResourceV1::Arithmetic)?,
                        alignment,
                    )
                }
                ScopedSlotRepresentationV29::ScalarArray(_) if SPILLS => {
                    let shape =
                        source_private_spill_shape_v25(original, root, slot, backing, budget)?;
                    (
                        usize::try_from(shape.element.size)
                            .map_err(|_| ArgumentResourceV1::Arithmetic)?,
                        shape.element.alignment,
                    )
                }
                _ => {
                    return original
                        .source
                        .missing("source private entry family requires typed scalar object slots");
                }
            };
            let input = source_slot_input_v18(original, root, slot, budget)?;
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

fn source_private_current_slot_v18<'a>(
    currentness: &'a CheckedOptimizedSourceMemoryV18<'_>,
    original: &ProductionSourceCorrespondenceV18<'_>,
    optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    root: usize,
    object: &OptimizedSourceObjectV18<'_>,
    before: ValueId,
    after: ValueId,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<&'a PendingSourceMemoryAccessV29> {
    source_private_current_occurrence_v25(
        currentness,
        original,
        optimized,
        root,
        object.original.instance,
        object.input,
        object.output,
        before,
        after,
        budget,
    )
}

fn source_private_current_occurrence_v25<'a>(
    currentness: &'a CheckedOptimizedSourceMemoryV18<'_>,
    original: &ProductionSourceCorrespondenceV18<'_>,
    optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    root: usize,
    instance: usize,
    input: fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1,
    output: fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1,
    before: ValueId,
    after: ValueId,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<&'a PendingSourceMemoryAccessV29> {
    if !currentness.retained_value_footprint_v18(
        original,
        optimized,
        root,
        instance,
        input,
        0,
        before,
        Some((output, 0, after)),
        budget,
    )? {
        return original
            .source
            .missing("source private access has no original activation/currentness");
    }
    // Reuse the existing sorted complete currentness index, never scan slots
    // or all original accesses for each actual operation.
    let key = [
        input.block.function.0 as usize,
        input.block.block as usize,
        input.operation as usize,
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
    currentness
        .original
        .pending
        .accesses
        .get(row.original)
        .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
            "source private currentness original row",
        ))
}

fn source_private_safe_allocation_v18(
    currentness: &CheckedOptimizedSourceMemoryV18<'_>,
    allocation: &SourcePrivateAllocationV18<'_>,
    object: &OptimizedSourceObjectV18<'_>,
    access: &PendingSourceMemoryAccessV29,
    safe: SourceSafeObjectOriginV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<()> {
    budget.charge_work(26)?;
    let endpoint = match object.original.source.role {
        ScopedObjectRoleV29::ReadValue { source, .. }
            if safe.key.access == SourceReferenceAccessV29::Read =>
        {
            source
        }
        ScopedObjectRoleV29::WriteValue { destination, .. }
            if safe.key.access == SourceReferenceAccessV29::Write =>
        {
            destination
        }
        _ => {
            return Err(ProductionSourceOwnedViewErrorV18::Binding(
                "source private safe access changed original role",
            ));
        }
    };
    let ScopedSlotRepresentationV29::Object { schema, .. } = allocation.original.representation
    else {
        return Err(ProductionSourceOwnedViewErrorV18::Binding(
            "source private safe allocation schema",
        ));
    };
    let alternatives = currentness
        .original
        .pending
        .alternatives
        .get(access.alternatives.clone())
        .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
            "source private safe activation range",
        ))?;
    let [alternative] = alternatives else {
        return Err(ProductionSourceOwnedViewErrorV18::Binding(
            "source private safe activation is not exact",
        ));
    };
    if access.instance.index() != object.original.instance
        || access.anchor != object.original.row
        || safe.key.site.instance != access.instance
        || object.original.anchor.source != Some(safe.frame)
        || safe.referent.generation != 0
        || allocation.original.instance != safe.referent.instance
        || allocation.original.origin.identity
            != (ScopedAllocationIdentityV29::OriginalObject {
                local: safe.referent.local.index(),
                generation: 0,
            })
        || allocation.original.origin.semantic_type != safe.referent.ty
        || endpoint.root_type != safe.referent.ty
        || endpoint.projected_type != safe.referent.ty
        || endpoint.root_schema != schema
        || endpoint.projected_schema != schema
        || endpoint.path.count != 0
        || !matches!(endpoint.object, ScopedObjectIdentityV29::Reference { instance, site, role, dereference_prefix }
            if instance == access.instance && site == safe.frame.site
                && safe.frame.role == Some(ScopedMemoryRoleV29::Operand(role)) && dereference_prefix != 0)
        || alternative.instance != safe.referent.instance
        || alternative.local != safe.referent.local
        || alternative.slot != allocation.slot
        || alternative.activation != SourceMemoryActivationV29::Invocation
        || alternative.formation != Some(safe.formation)
    {
        return Err(ProductionSourceOwnedViewErrorV18::Binding(
            "source private safe access differs from original referent and activation",
        ));
    }
    Ok(())
}

fn source_private_raw_allocation_v26(
    original: &ProductionSourceCorrespondenceV18<'_>,
    currentness: &CheckedOptimizedSourceMemoryV18<'_>,
    allocation: &SourcePrivateAllocationV18<'_>,
    object: &OptimizedSourceObjectV18<'_>,
    access: &PendingSourceMemoryAccessV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<()> {
    // Fixed role/place/type/schema comparisons and the bounded original-place
    // selector are prepaid separately from the formation-alternative walk.
    budget.charge_work(128)?;
    let (endpoint, writing) = match object.original.source.role {
        ScopedObjectRoleV29::ReadValue { source, .. } => (source, false),
        ScopedObjectRoleV29::WriteValue { destination, .. } => (destination, true),
        _ => {
            return original
                .source
                .missing("source private raw access changed its role");
        }
    };
    let ScopedAllocationIdentityV29::OriginalObject {
        local,
        generation: 0,
    } = allocation.original.origin.identity
    else {
        return original
            .source
            .missing("source private raw allocation is not an original invocation object");
    };
    let ScopedSlotRepresentationV29::Object { schema, .. } = allocation.original.representation
    else {
        return original
            .source
            .missing("source private raw allocation schema absent");
    };
    let ScopedObjectIdentityV29::Reference {
        instance,
        site,
        role,
        dereference_prefix,
    } = endpoint.object
    else {
        return original
            .source
            .missing("source private indirect access has no original reference endpoint");
    };
    let semantic = original.source.source_semantic(budget)?;
    let (function, _) =
        original
            .source
            .instance(allocation.root, access.instance.index(), budget)?;
    let function = semantic.functions().get(function.index() as usize).ok_or(
        ProductionSourceOwnedViewErrorV18::Binding("source private raw access original function"),
    )?;
    let place = scoped_object_original_place_v29(function, site, role).ok_or(
        ProductionSourceOwnedViewErrorV18::Binding("source private raw access original place"),
    )?;
    let pointer = function
        .locals()
        .get(place.local().index() as usize)
        .and_then(|local| semantic.types().get(local.ty().index() as usize))
        .map(SemanticTypeDeclV1::shape);
    if access.instance.index() != object.original.instance
        || access.anchor != object.original.row
        || instance != access.instance
        || object.original.anchor.source != Some(ScopedMemoryFrameV29::operand(site, Some(role)))
        || dereference_prefix != 1
        || !matches!(place.projections(), [projection]
            if projection.kind() == SemanticProjectionKindV1::Dereference
                && projection.result_type() == place.ty())
        || !matches!(pointer, Some(SemanticTypeShapeV1::Pointer(pointer))
            if pointer.kind() == SemanticPointerKindV1::Raw
                && pointer.metadata() == SemanticPointerMetadataV1::None
                && pointer.pointee() == place.ty()
                && (!writing || pointer.mutability() == SemanticMutabilityV1::Mutable))
        || endpoint.source
            != (ScopedObjectSourceV29::Place {
                site,
                role,
                local: place.local(),
                prefix: 1,
            })
        || endpoint.root_type != allocation.original.origin.semantic_type
        || endpoint.projected_type != endpoint.root_type
        || place.ty() != endpoint.root_type
        || endpoint.root_schema != schema
        || endpoint.projected_schema != schema
        || endpoint.path.count != 0
        || endpoint.source_path.count != 1
        || access.safe_object.is_some()
    {
        return original.source.missing(
            "source private raw access differs from its original whole-object dereference",
        );
    }
    let alternatives = currentness
        .original
        .pending
        .alternatives
        .get(access.alternatives.clone())
        .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
            "source private raw activation range",
        ))?;
    budget.charge_work(1)?;
    if alternatives.is_empty() {
        return original
            .source
            .missing("source private raw activation is absent");
    }
    // These alternatives were issued by exact original raw-origin replay, not
    // inferred from equal physical addresses. The caller independently checks
    // the complete original/output currentness equations for this occurrence.
    for alternative in alternatives {
        budget.charge_work(6)?;
        if alternative.instance != allocation.original.instance
            || alternative.local.index() != local
            || alternative.slot != allocation.slot
            || alternative.activation != SourceMemoryActivationV29::Invocation
            || alternative.formation.is_none()
        {
            return original.source.missing(
                "source private raw access differs from its original referent and formation",
            );
        }
    }
    budget.charge_work(1)?;
    Ok(())
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

fn source_private_write_destination_v22(
    allocation: &SourcePrivateAllocationV18<'_>,
    object: &OptimizedSourceObjectV18<'_>,
    source_writes: bool,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<bool> {
    if matches!(
        object.original.source.role,
        ScopedObjectRoleV29::WriteValue {
            value: ScopedObjectValueOriginV29::Original(
                ScopedMemoryStoreSourceV29::EntryArgument { .. }
            ),
            ..
        }
    ) || !source_writes
    {
        source_private_entry_destination_v18(allocation, object, budget)?;
        return Ok(false);
    }
    budget.charge_work(8)?;
    let ScopedSlotRepresentationV29::Object { schema, .. } = allocation.original.representation
    else {
        return Err(ProductionSourceOwnedViewErrorV18::Binding(
            "private source write allocation schema",
        ));
    };
    let ScopedObjectRoleV29::WriteValue {
        destination,
        value:
            ScopedObjectValueOriginV29::Original(
                ScopedMemoryStoreSourceV29::Assignment { ty, .. }
                | ScopedMemoryStoreSourceV29::Operand { ty, .. },
            ),
    } = object.original.source.role
    else {
        return Err(ProductionSourceOwnedViewErrorV18::Binding(
            "private source write original value role",
        ));
    };
    if ty != allocation.original.origin.semantic_type
        || destination.root_type != ty
        || destination.projected_type != ty
        || destination.root_schema != schema
        || destination.projected_schema != schema
        || destination.path.count != 0
        || object.original.anchor.source.is_none()
    {
        return Err(ProductionSourceOwnedViewErrorV18::Binding(
            "private source write destination differs from current slot",
        ));
    }
    Ok(true)
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
    let access = source_private_current_slot_v18(
        currentness,
        original,
        core.optimized,
        root,
        object,
        before,
        after,
        budget,
    )?;
    budget.charge_work(12)?;
    if !core.physical.operation(ordinal)
        || allocation.root != root
        || allocation.slot != access.physical.slot
        || address.length() != 1
        || address.offset() != 0
        || output
            .operations()
            .get(address.allocation())
            .is_none_or(|operation| {
                operation.coordinate != allocation.output
                    || !matches!(operation.operation.results.as_slice(), [result]
                    if result.id == allocation.pointer)
            })
    {
        return original
            .source
            .missing("source private access source slot and physical allocation differ");
    }
    if let Some(safe) = access.safe_object {
        // `after` was independently resolved through the physical definition
        // table above. This authenticated safe origin permits a distinct
        // pointer/access instance; equal allocation numbers alone do not.
        source_private_safe_allocation_v18(currentness, allocation, object, access, safe, budget)?;
    } else if matches!(
        object.original.source.role,
        ScopedObjectRoleV29::ReadValue {
            source: ScopedObjectEndpointV29 {
                object: ScopedObjectIdentityV29::Reference { .. },
                ..
            },
            ..
        } | ScopedObjectRoleV29::WriteValue {
            destination: ScopedObjectEndpointV29 {
                object: ScopedObjectIdentityV29::Reference { .. },
                ..
            },
            ..
        }
    ) {
        source_private_raw_allocation_v26(
            original,
            currentness,
            allocation,
            object,
            access,
            budget,
        )?;
    } else if allocation.original.instance.index() != object.original.instance
        || allocation.pointer != after
    {
        return original
            .source
            .missing("source private direct access pointer or source instance differs");
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
            let source_write = source_private_write_destination_v22(
                allocation,
                object,
                entries.source_writes,
                budget,
            )?;
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
            if source_write {
                SourcePrivateOperationKindV18::SourceWrite
            } else {
                SourcePrivateOperationKindV18::EntryWrite
            }
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
    source_private_root_rows_with_v25(core, currentness, entries, root, budget, |_, _, _| Ok(()))
}

fn source_private_root_rows_with_v25<'work>(
    core: &CheckedSourcePrivatePhysicalV18<'_>,
    currentness: &CheckedOptimizedSourceMemoryV18<'_>,
    entries: &ProductionCheckedSourceEntryWritesV18<'_>,
    root: usize,
    budget: &mut ArgumentBudgetV1<'work>,
    extend: impl FnOnce(
        usize,
        &mut [Option<SourcePrivateOperationV18>],
        &mut ArgumentBudgetV1<'work>,
    ) -> SourceOwnedResultV18<()>,
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
    extend(first, &mut rows, budget)?;
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
            if !matches!(
                writer.kind,
                SourcePrivateOperationKindV18::EntryWrite
                    | SourcePrivateOperationKindV18::SourceWrite
            ) || writer.root != root
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
        source_private_header_v18::<&PendingSourceMemoryAccessV29>()?,
        source_private_header_v18::<SourceSafeObjectOriginV29>()?,
        source_private_header_v18::<(
            &ProductionSourceCorrespondenceV18<'_>,
            &CheckedOptimizedSourceMemoryV18<'_>,
            &SourcePrivateAllocationV18<'_>,
            &OptimizedSourceObjectV18<'_>,
            &PendingSourceMemoryAccessV29,
            &mut ArgumentBudgetV1<'_>,
        )>()?,
        source_private_header_v18::<(ScopedObjectEndpointV29, bool)>()?,
        source_private_header_v18::<&AdmittedInertSemanticMirV1>()?,
        source_private_header_v18::<SemanticFunctionIdV1>()?,
        source_private_header_v18::<(SemanticFunctionIdV1, Option<(usize, SemanticBlockIdV1)>)>()?,
        source_private_header_v18::<&SemanticFunctionDeclV1>()?,
        source_private_header_v18::<Option<&SemanticFunctionDeclV1>>()?,
        source_private_header_v18::<&SemanticPlaceV1>()?,
        source_private_header_v18::<Option<&SemanticPlaceV1>>()?,
        source_private_header_v18::<Option<&SemanticTypeShapeV1>>()?,
        source_private_header_v18::<Option<&SemanticTypeDeclV1>>()?,
        source_private_header_v18::<Option<&fe2o3_mir_model::semantic_mir_v1::SemanticLocalDeclV1>>(
        )?,
        source_private_header_v18::<&fe2o3_mir_model::semantic_mir_v1::SemanticPointerTypeV1>()?,
        source_private_header_v18::<&SemanticProjectionV1>()?,
        source_private_header_v18::<ScopedMemoryFrameV29>()?,
        source_private_header_v18::<SemanticLocalIdV1>()?,
        source_private_header_v18::<std::slice::Iter<'_, PendingSourceMemoryAlternativeV29>>()?,
        source_private_header_v18::<Option<SourceSafeObjectOriginV29>>()?,
        source_private_header_v18::<SourceReferenceAccessV29>()?,
        source_private_header_v18::<SourceMemoryActivationV29>()?,
        source_private_header_v18::<Option<SourceReferenceSiteV29>>()?,
        source_private_header_v18::<Option<ScopedMemoryFrameV29>>()?,
        source_private_header_v18::<Option<ScopedMemoryRoleV29>>()?,
        source_private_header_v18::<Option<&[PendingSourceMemoryAlternativeV29]>>()?,
        source_private_header_v18::<&[PendingSourceMemoryAlternativeV29]>()?,
        source_private_header_v18::<&PendingSourceMemoryAlternativeV29>()?,
        source_private_header_v18::<std::ops::Range<usize>>()?,
        source_private_header_v18::<ScopedObjectEndpointV29>()?,
        source_private_header_v18::<ScopedSlotRepresentationV29>()?,
        source_private_header_v18::<(
            &CheckedOptimizedSourceMemoryV18<'_>,
            &SourcePrivateAllocationV18<'_>,
            &OptimizedSourceObjectV18<'_>,
            &PendingSourceMemoryAccessV29,
            SourceSafeObjectOriginV29,
            &mut ArgumentBudgetV1<'_>,
        )>()?,
        source_private_header_v18::<ProductionCallInstanceIdV1>()?,
        source_private_header_v18::<ExecutionSiteV29>()?,
        source_private_header_v18::<ExecutionOperandV29>()?,
        source_private_header_v18::<u32>()?,
        source_private_header_v18::<fe2o3_kernel_ir::StorageLayoutIdV1>()?,
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

#[cfg(test)]
impl CheckedSourcePrivateMemoryV18<'_> {
    pub(super) fn test_safe_pointer_substitution_v18(
        &self,
        other_allocation: bool,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<()> {
        self.test_reference_pointer_substitution_v26(true, other_allocation, budget)
    }

    pub(super) fn test_raw_pointer_substitution_v26(
        &self,
        other_allocation: bool,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<()> {
        self.test_reference_pointer_substitution_v26(false, other_allocation, budget)
    }

    fn test_reference_pointer_substitution_v26(
        &self,
        safe: bool,
        other_allocation: bool,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<()> {
        self.check(budget)?;
        // These are actual-view query substitutions, not fabricated canonical
        // owners. The full unchanged composition must precede each refusal.
        budget.reserve_storage(argument_sum_v1(&[
            source_private_root_headers_v18()?,
            source_private_header_v18::<std::slice::Iter<'_, OptimizedSourceMemoryOccurrenceV18>>(
            )?,
            source_private_header_v18::<Option<&OptimizedSourceMemoryOccurrenceV18>>()?,
            source_private_header_v18::<
                std::slice::Iter<'_, Option<SourcePrivateAllocationV18<'_>>>,
            >()?,
            source_private_header_v18::<Option<&Option<SourcePrivateAllocationV18<'_>>>>()?,
            source_private_header_v18::<(bool, bool, &Self, &mut ArgumentBudgetV1<'_>)>()?,
            source_private_header_v18::<&[PendingSourceMemoryAlternativeV29]>()?,
            source_private_header_v18::<std::slice::Iter<'_, PendingSourceMemoryAlternativeV29>>()?,
        ])?)?;
        for row in self.currentness.accesses {
            budget.charge_work(5)?;
            let original = &self.currentness.original.pending.accesses[row.original];
            if original.safe_object.is_some() != safe {
                continue;
            }
            if let Some(safe) = original.safe_object {
                if safe.key.access != SourceReferenceAccessV29::Read {
                    continue;
                }
            } else {
                let alternatives =
                    &self.currentness.original.pending.alternatives[original.alternatives.clone()];
                budget.charge_work(argument_sum_v1(&[alternatives.len(), 1])?)?;
                if alternatives.is_empty() || alternatives.iter().any(|row| row.formation.is_none())
                {
                    continue;
                }
            }
            let Some((output, access, pointer)) = row.output else {
                panic!("retained safe read");
            };
            let (input, input_pointer) =
                immutable_memory_access_v29(self.physical.original, self.root, original, budget)?;
            assert!(self.currentness.retained_value_footprint_v18(
                self.physical.original,
                self.physical.optimized,
                self.root,
                original.instance.index(),
                input,
                row.input_access,
                input_pointer,
                Some((output, access, pointer)),
                budget
            )?);
            let mut replacement = None;
            for candidate in self.physical.allocations {
                budget.charge_work(2)?;
                if let Some(candidate) = candidate
                    && candidate.root == self.root
                    && (candidate.slot != original.physical.slot) == other_allocation
                    && candidate.pointer != pointer
                {
                    replacement = Some(candidate.pointer);
                    break;
                }
            }
            let replacement = replacement.expect("genuine direct or other same-schema allocation");
            return self
                .currentness
                .retained_value_footprint_v18(
                    self.physical.original,
                    self.physical.optimized,
                    self.root,
                    original.instance.index(),
                    input,
                    row.input_access,
                    input_pointer,
                    Some((output, access, replacement)),
                    budget,
                )
                .map(|_| ());
        }
        panic!("original reference read required before hostile query")
    }

    pub(super) fn test_safe_counts_v18(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<[usize; 3]> {
        self.check(budget)?;
        let floor = budget.storage();
        let mut counts = [0; 3];
        scoped_source_attempt_v29(
            self.physical.original.source.cleanup,
            budget,
            floor,
            |budget| {
                budget.reserve_storage(argument_sum_v1(&[source_private_root_headers_v18()?,
                source_private_header_v18::<[usize; 3]>()?,
                source_private_header_v18::<(&Self, &mut [usize; 3], &mut ArgumentBudgetV1<'_>)>()?,
            ])?)?;
                let output = self.physical.optimized.output_inventory(budget)?;
                for (relative, row) in self.rows.iter().enumerate() {
                    budget.charge_work(6)?;
                    let Some(row) = row else {
                        continue;
                    };
                    if !matches!(row.kind, SourcePrivateOperationKindV18::Read { .. }) {
                        continue;
                    }
                    let key = [
                        row.input.block.function.0 as usize,
                        row.input.block.block as usize,
                        row.input.operation as usize,
                        0,
                    ];
                    let at = private_array_partition_v1(
                        self.currentness.accesses,
                        optimized_currentness_occurrence_key_v18,
                        key,
                        false,
                        &mut SourceCorrespondenceWorkV18(budget),
                    )?;
                    let checked = &self.currentness.accesses[at];
                    assert_eq!(optimized_currentness_occurrence_key_v18(checked), key);
                    let original = &self.currentness.original.pending.accesses[checked.original];
                    let Some(safe) = original.safe_object else {
                        continue;
                    };
                    let allocation = self.physical.allocations[row.allocation].as_ref().unwrap();
                    let OperationKind::Storage(ScopedObjectOperationV29::ReadValue {
                        address, ..
                    }) = output.operations()[self.first + relative].operation.kind
                    else {
                        panic!("genuine safe typed read");
                    };
                    assert_eq!(original.instance.index(), row.instance);
                    assert_eq!(safe.referent.instance, allocation.original.instance);
                    assert_eq!(original.physical.slot, allocation.slot);
                    counts[0] += 1;
                    counts[1] += usize::from(original.instance != safe.referent.instance);
                    counts[2] += usize::from(address != allocation.pointer);
                }
                Ok::<_, ProductionSourceOwnedViewErrorV18>(())
            },
        )?;
        budget.release_storage(
            budget
                .storage()
                .checked_sub(floor)
                .ok_or(ArgumentResourceV1::Accounting)?,
        )?;
        Ok(counts)
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
        self.with_root_memory_extend_v25(
            root,
            currentness,
            entries,
            budget,
            |_, _, _| Ok(()),
            consume,
        )
    }

    fn with_root_memory_extend_v25<'work, T, E>(
        &self,
        root: usize,
        currentness: &CheckedOptimizedSourceMemoryV18<'_>,
        entries: &ProductionCheckedSourceEntryWritesV18<'_>,
        budget: &mut ArgumentBudgetV1<'work>,
        extend: impl FnOnce(
            usize,
            &mut [Option<SourcePrivateOperationV18>],
            &mut ArgumentBudgetV1<'work>,
        ) -> SourceOwnedResultV18<()>,
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
        let mut consume = Some(consume);
        let capture = std::mem::size_of_val(&consume);
        let alignment = std::mem::align_of_val(&consume);
        let mut extend = Some(extend);
        let extend_bytes = std::mem::size_of_val(&extend);
        let extend_alignment = if extend_bytes == 0 {
            0
        } else {
            std::mem::align_of_val(&extend)
        };
        let prepared =
            scoped_source_attempt_v29(self.original.source.cleanup, budget, floor, |budget| {
                let floor = budget.storage();
                self.original
                    .retain_query((|| {
                        let disposal = source_owned_finish_preflight_v26::<T, E>(budget)?;
                        budget.reserve_storage(argument_sum_v1(&[
                            disposal,
                            capture,
                            alignment,
                            extend_bytes,
                            extend_alignment,
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
                        let (first, rows) = source_private_root_rows_with_v25(
                            self,
                            currentness,
                            entries,
                            root,
                            budget,
                            extend.take().ok_or(ArgumentResourceV1::Accounting)?,
                        )?;
                        let retained = budget
                            .storage()
                            .checked_sub(floor)
                            .ok_or(ArgumentResourceV1::Accounting)?;
                        Ok((first, rows, retained))
                    })())
                    .inspect_err(|_| {
                        source_reference_discard_v29(extend.take());
                        source_reference_discard_v29(consume.take());
                    })
            });
        let (first, rows, retained) = match prepared {
            Ok(prepared) => prepared,
            Err(error) => {
                source_reference_discard_v29(extend);
                source_reference_discard_v29(consume);
                return Err(error.into());
            }
        };
        let consume = consume.expect("private root preparation retained its callback");
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
                SourcePrivateOperationKindV18::EntryWrite
                | SourcePrivateOperationKindV18::SourceWrite => 1,
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
