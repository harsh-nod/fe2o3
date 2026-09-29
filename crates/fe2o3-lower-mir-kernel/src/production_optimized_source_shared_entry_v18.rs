// The source argument contract is not a runtime allocation certificate.
struct PendingSharedEntryRegionsV18<'s, 'a> {
    source: &'s PendingGlobalSourceAccessesV18<'s>,
    arguments: ArgumentViewDataV18<'a>,
    profile: kernel_argument_abi_v18::SourceDescriptorRootAbiV29<'s>,
    original_function: SemanticFunctionIdV1,
    physical_function: usize,
}

struct PendingSharedEntryRegionV18<'s, 'g> {
    root: usize,
    source: ProductionArgumentNodeV1<'s>,
    abi: &'s kernel_argument_abi_v18::SourceSharedEntryAbiV18<'s>,
    read: &'s PendingGlobalReadConditionsV18<'s, 'g>,
}

impl PendingSharedEntryRegionV18<'_, '_> {
    fn root(&self) -> usize {
        self.root
    }
    fn original_argument(&self) -> u32 {
        self.source.source_argument()
    }
    fn original_function(&self) -> SemanticFunctionIdV1 {
        self.abi.function()
    }
    fn original_type(&self) -> SemanticTypeIdV1 {
        self.abi.ty()
    }
    fn original_type_identity(&self) -> fe2o3_mir_model::semantic_mir_v1::SemanticTypeIdentityV1 {
        self.abi.identity()
    }
    fn source_sha256(&self) -> &[u8; 32] {
        self.abi.source()
    }
    fn root_binding(&self) -> &[u8; 32] {
        self.abi.binding()
    }
    fn original_parameter(&self) -> SliceDefinition {
        self.read.pair.input.logical.root
    }
    fn optimized_parameter(&self) -> SliceDefinition {
        self.read.pair.output.logical.root
    }
    fn allocation(&self) -> fe2o3_kernel_ir::FormalAllocationIdentity {
        self.read.fact.domain().allocation()
    }
    fn element(&self) -> ScalarType {
        self.abi.scalar()
    }
    const fn requires_runtime_initialized_extent(&self) -> bool {
        true
    }
    const fn requires_runtime_allocation_binding(&self) -> bool {
        true
    }
    const fn requires_runtime_allocation_lifetime(&self) -> bool {
        true
    }
    const fn requires_runtime_base_alignment(&self) -> bool {
        true
    }
    const fn alias_and_concurrency_are_proved(&self) -> bool {
        false
    }
    const fn grants_memory_or_launch_authority(&self) -> bool {
        false
    }
}

// ArgumentView owns unit-query scratch. Observe the external consumer's exact
// floor before any walker can refund that scratch and hide retained growth.
fn shared_entry_consume_v18<'work>(
    original: &ProductionSourceCorrespondenceV18<'_>,
    budget: &mut ArgumentBudgetV1<'work>,
    consume: impl FnOnce(&mut ArgumentBudgetV1<'work>) -> SourceOwnedResultV18<()>,
) -> SourceOwnedResultV18<()> {
    original.check(budget)?;
    let prepared = original.retain_query((|| {
        let storage =
            source_owned_finish_preflight_v26::<(), ProductionSourceOwnedViewErrorV18>(budget)?;
        budget.reserve_storage(storage)?;
        Ok(storage)
    })());
    let storage = match prepared {
        Ok(storage) => storage,
        Err(error) => {
            source_reference_discard_v29(consume);
            return Err(error);
        }
    };
    let required = budget.storage();
    let slot = std::ptr::from_ref(&*budget) as usize;
    let ledger = budget.work_ledger_identity_v1();
    let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        original.retain_query(consume(budget))
    }));
    let first = original.source.guard.first.get();
    let custody = slot == std::ptr::from_ref(&*budget) as usize
        && ledger == budget.work_ledger_identity_v1()
        && budget.storage() == required;
    let postflight = if custody {
        original.check(budget)
    } else {
        original.source.cleanup.deny_refund();
        original.retain_query(Err(ArgumentResourceV1::Accounting.into()))
    };
    source_owned_finish_callback_v18(
        caught,
        first,
        postflight,
        original.source.cleanup,
        budget,
        storage,
    )
}

fn shared_entry_node_matches_v18(
    node: &ProductionArgumentNodeV1<'_>,
    slot: usize,
    value: ValueId,
    ty: &Type,
) -> bool {
    shared_entry_node_parts_match_v18(
        node.source_path().is_empty(),
        node.local_binding()
            .is_some_and(|(_, path)| path.is_empty()),
        node.coverage(),
        slot,
        value,
        ty,
    )
}

fn shared_entry_node_parts_match_v18(
    source_root: bool,
    local_root: bool,
    coverage: ProductionArgumentCoverageV1<'_>,
    slot: usize,
    value: ValueId,
    ty: &Type,
) -> bool {
    source_root
        && local_root
        && matches!(coverage, ProductionArgumentCoverageV1::Parameter(parameter)
            if parameter.slot() == slot && parameter.value() == value && parameter.ty() == ty)
}

fn shared_entry_headers_v18() -> Result<usize, ArgumentResourceV1> {
    use kernel_argument_abi_v18::{SourceDescriptorRootAbiV29, SourceSharedEntryAbiV18};
    type Join<'a> = (
        &'a PendingGlobalSourceAccessesV18<'a>,
        &'a PendingGlobalReadConditionsV18<'a, 'a>,
        &'a mut ArgumentBudgetV1<'a>,
        &'a mut (),
        &'a ProductionSourceCorrespondenceV18<'a>,
        &'a fe2o3_kernel_analysis::CanonicalKirInventoryV18<'a>,
        &'a CanonicalKirDefinitionRefV1<'a>,
        SourceDescriptorRootAbiV29<'a>,
        Option<SourceDescriptorRootAbiV29<'a>>,
        SourceSharedEntryAbiV18<'a>,
        Option<SourceSharedEntryAbiV18<'a>>,
        &'a SourceSharedEntryAbiV18<'a>,
        ArgumentViewDataV18<'a>,
        &'a ArgumentViewDataV18<'a>,
        ProductionArgumentNodeV1<'a>,
        &'a ProductionArgumentNodeV1<'a>,
        ProductionArgumentCoverageV1<'a>,
        ProductionPhysicalArgumentV1<'a>,
        Option<(SemanticLocalIdV1, &'a [ProductionArgumentProjectionV1])>,
        &'a [ProductionArgumentProjectionV1],
        &'a Type,
        Type,
        [SliceDefinition; 2],
        fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1,
        SemanticFunctionIdV1,
        SemanticTypeIdV1,
        (SemanticFunctionIdV1, usize),
        Option<(u32, SemanticTypeIdV1)>,
        &'a mut Option<(u32, SemanticTypeIdV1)>,
        (u32, SemanticTypeIdV1),
        [usize; 4],
        Option<usize>,
        u32,
        ValueId,
        Option<ValueId>,
        bool,
        Result<usize, std::num::TryFromIntError>,
        ScalarType,
        fe2o3_kernel_ir::FormalAllocationIdentity,
        fe2o3_kernel_ir::FormalRuntimeSliceReadDomainV1,
        &'a [u8; 32],
        fe2o3_mir_model::semantic_mir_v1::SemanticTypeIdentityV1,
    );
    type Nodes<'a> = (
        &'a PendingGlobalReadConditionsV18<'a, 'a>,
        &'a CanonicalKirDefinitionRefV1<'a>,
        &'a SourceSharedEntryAbiV18<'a>,
        &'a PendingGlobalSourceAccessesV18<'a>,
        &'a mut (),
        &'a mut usize,
        &'a mut Option<(u32, SemanticTypeIdV1)>,
        usize,
        (u32, SemanticTypeIdV1),
    );
    type BorrowedConsumer<'a> = (&'a PendingSharedEntryRegionV18<'a, 'a>, &'a mut ());
    type RootBuild<'a> = (
        &'a PendingGlobalSourceAccessesV18<'a>,
        &'a ProductionSourceCorrespondenceV18<'a>,
        &'a mut (),
        &'a mut ArgumentBudgetV1<'a>,
        &'a SemanticFunctionIdV1,
        &'a usize,
        &'a SourceDescriptorRootAbiV29<'a>,
        (&'a PendingSharedEntryRegionsV18<'a, 'a>, &'a mut ()),
        [usize; 3],
    );
    type Walk<'a> = (
        &'a PendingSharedEntryRegionsV18<'a, 'a>,
        &'a ProductionSourceCorrespondenceV18<'a>,
        &'a PendingGlobalReadConditionsV18<'a, 'a>,
        &'a SourceSharedEntryAbiV18<'a>,
        &'a mut (),
        &'a mut usize,
        &'a mut Option<(u32, SemanticTypeIdV1)>,
        &'a usize,
        &'a ValueId,
        &'a Type,
        &'a u32,
        &'a SemanticTypeIdV1,
        ProductionArgumentNodeV1<'a>,
        &'a mut ArgumentBudgetV1<'a>,
    );
    type Finish<'a> = (
        std::thread::Result<SourceOwnedResultV18<()>>,
        Option<SourceOwnedQueryFailureV18>,
        SourceOwnedResultV18<()>,
        &'a ScopedSourceCleanupV29,
        &'a mut ArgumentBudgetV1<'a>,
        usize,
    );
    type RootCatch<'a> = (
        &'a PendingGlobalSourceAccessesV18<'a>,
        &'a ProductionSourceCorrespondenceV18<'a>,
        &'a mut ArgumentBudgetV1<'a>,
    );
    type Consumer<'a> = (
        &'a ProductionSourceCorrespondenceV18<'a>,
        &'a mut ArgumentBudgetV1<'a>,
        &'a PendingSharedEntryRegionV18<'a, 'a>,
        &'a mut (),
        BorrowedConsumer<'a>,
        (
            &'a ProductionSourceCorrespondenceV18<'a>,
            &'a mut ArgumentBudgetV1<'a>,
            BorrowedConsumer<'a>,
        ),
        std::panic::AssertUnwindSafe<(
            &'a ProductionSourceCorrespondenceV18<'a>,
            &'a mut ArgumentBudgetV1<'a>,
            BorrowedConsumer<'a>,
        )>,
        [usize; 2],
        fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
        Option<SourceOwnedQueryFailureV18>,
        SourceOwnedQueryFailureV18,
        bool,
        SourceOwnedResultV18<()>,
        std::thread::Result<SourceOwnedResultV18<()>>,
        SourceOwnedResultV18<()>,
    );
    fn h<T>() -> Result<usize, ArgumentResourceV1> {
        argument_sum_v1(&[
            size_of::<T>(),
            argument_product_v1(2, size_of::<Result<T, ProductionSourceOwnedViewErrorV18>>())?,
        ])
    }
    argument_sum_v1(&[
        kernel_argument_abi_v18::shared_entry_abi_headers_v18()?,
        h::<Join<'_>>()?,
        h::<Nodes<'_>>()?,
        h::<Nodes<'_>>()?,
        h::<Consumer<'_>>()?,
        h::<RootBuild<'_>>()?,
        h::<RootBuild<'_>>()?,
        h::<Walk<'_>>()?,
        h::<Walk<'_>>()?,
        h::<Finish<'_>>()?,
        h::<RootCatch<'_>>()?,
        h::<std::panic::AssertUnwindSafe<RootCatch<'_>>>()?,
        h::<(
            usize,
            usize,
            fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
            bool,
        )>()?,
        h::<PendingSharedEntryRegionsV18<'_, '_>>()?,
        h::<&PendingSharedEntryRegionsV18<'_, '_>>()?,
        h::<(
            &PendingSharedEntryRegionsV18<'_, '_>,
            &mut ArgumentBudgetV1<'_>,
        )>()?,
        h::<PendingSharedEntryRegionV18<'_, '_>>()?,
        h::<(
            &PendingSharedEntryRegionV18<'_, '_>,
            &mut ArgumentBudgetV1<'_>,
        )>()?,
        h::<(&ProductionArgumentNodeV1<'_>, usize, ValueId, &Type)>()?,
        h::<Result<(), ProductionSemanticKirErrorV1>>()?,
        h::<Result<(), ArgumentResourceV1>>()?,
        h::<ProductionSemanticKirErrorV1>()?,
        h::<ProductionSourceOwnedViewErrorV18>()?,
        h::<Result<&CanonicalKirDefinitionRefV1<'_>, ProductionSourceOwnedViewErrorV18>>()?,
        h::<Result<Option<SourceSharedEntryAbiV18<'_>>, ProductionSemanticKirErrorV1>>()?,
        h::<Result<Option<SourceDescriptorRootAbiV29<'_>>, ProductionSourceOwnedViewErrorV18>>()?,
        h::<Result<(SemanticFunctionIdV1, usize), ProductionSourceOwnedViewErrorV18>>()?,
        h::<Option<ValueId>>()?,
        h::<fe2o3_mir_model::SemanticSourceArgumentV1<'_>>()?,
        h::<(SemanticLocalIdV1, &[ProductionArgumentProjectionV1])>()?,
        h::<(
            &PendingGlobalSourceAccessesV18<'_>,
            &fe2o3_pliron::PendingCanonicalGlobalAccessesV18<'_, '_>,
            &GlobalReadFactsV18<'_, '_>,
            SliceOperation,
            &mut ArgumentBudgetV1<'_>,
        )>()?,
        h::<(&ScopedSourceCleanupV29, &mut ArgumentBudgetV1<'_>, usize)>()?,
        h::<[usize; 3]>()?,
        h::<Result<(), PendingGlobalReadConditionErrorV18>>()?,
        h::<std::thread::Result<Result<(), PendingGlobalReadConditionErrorV18>>>()?,
        h::<std::thread::Result<Result<(), PendingGlobalReadConditionErrorV18>>>()?,
    ])
}

impl PendingGlobalSourceAccessesV18<'_> {
    // Complete captured ABI authentication and root ArgumentView construction
    // happen once here. Repeated reads borrow this same immutable root scope.
    fn with_shared_entry_regions_v18<'work>(
        &self,
        budget: &mut ArgumentBudgetV1<'work>,
        mut consume: impl for<'s, 'a> FnMut(
            &PendingSharedEntryRegionsV18<'s, 'a>,
            &mut ArgumentBudgetV1<'work>,
        ) -> SourceOwnedResultV18<()>,
    ) -> SourceOwnedResultV18<()> {
        let original = self.original();
        original.check(budget)?;
        original.retain_query(self.roles.observe_custody(budget))?;
        let capture = std::mem::size_of_val(&consume);
        let alignment = std::mem::align_of_val(&consume);
        let prepared = original.retain_query((|| {
            let storage = shared_entry_headers_v18().and_then(|fixed| {
                argument_sum_v1(&[
                    fixed,
                    capture,
                    alignment,
                    source_owned_finish_preflight_v26::<(), ProductionSourceOwnedViewErrorV18>(
                        budget,
                    )?,
                ])
            })?;
            budget.reserve_storage(storage)?;
            Ok(storage)
        })());
        let storage = match prepared {
            Ok(storage) => storage,
            Err(error) => {
                source_reference_discard_v29(consume);
                return Err(error);
            }
        };
        let required = budget.storage();
        let slot = std::ptr::from_ref(&*budget) as usize;
        let ledger = budget.work_ledger_identity_v1();
        let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let result = source_scalar_normalization_scratch_v18(
                original.source.cleanup,
                budget,
                0,
                |budget| {
                    let (original_function, physical_function) =
                        original.source.root(self.root(), budget)?;
                    let profile = original
                        .source
                        .descriptor_root_abi_v29(self.root(), budget)?
                        .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                            "shared entry region lacks captured ABI",
                        ))?;
                    original.with_root_argument_data_v18(
                        self.root(),
                        budget,
                        |arguments, budget| {
                            if arguments.semantic_function(budget)? != original_function {
                                return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                            }
                            let scope = PendingSharedEntryRegionsV18 {
                                source: self,
                                arguments,
                                profile,
                                original_function,
                                physical_function,
                            };
                            shared_entry_consume_v18(original, budget, |budget| {
                                consume(&scope, budget)
                            })
                            .map_err(source_slice_query_error_v18)
                        },
                    )
                },
            );
            // Both the complete query result and earlier retained failures are
            // settled before the still-prepaid external callback is destroyed.
            let result = original.retain_query(result);
            drop(consume);
            result
        }));
        let first = original.source.guard.first.get();
        let custody = slot == std::ptr::from_ref(&*budget) as usize
            && ledger == budget.work_ledger_identity_v1()
            && budget.storage() == required;
        let postflight = if custody {
            original.check(budget)
        } else {
            original.source.cleanup.deny_refund();
            original.retain_query(Err(ArgumentResourceV1::Accounting.into()))
        };
        source_owned_finish_callback_v18(
            caught,
            first,
            postflight,
            original.source.cleanup,
            budget,
            storage,
        )
    }
}

impl PendingSharedEntryRegionsV18<'_, '_> {
    fn original(&self) -> &ProductionSourceCorrespondenceV18<'_> {
        self.source.original()
    }

    fn root(&self) -> usize {
        self.source.root()
    }

    // Only the authentic local-read constructor can supply `read`. The owned
    // consumer remains in its catch/latch/drop boundary, not in a node walker.
    fn with_shared_entry_region_v18<'work>(
        &self,
        native: &fe2o3_pliron::PendingCanonicalGlobalAccessesV18<'_, '_>,
        facts: &GlobalReadFactsV18<'_, '_>,
        operation: SliceOperation,
        budget: &mut ArgumentBudgetV1<'work>,
        mut consume: impl for<'s, 'g> FnMut(
            &PendingSharedEntryRegionV18<'s, 'g>,
            &mut ArgumentBudgetV1<'work>,
        ) -> SourceOwnedResultV18<()>,
    ) -> Result<(), PendingGlobalReadConditionErrorV18> {
        let original = self.original();
        original.check(budget)?;
        original.retain_query(self.source.roles.observe_custody(budget))?;
        original.retain_query(
            self.arguments
                .check(budget)
                .map_err(source_argument_error_v18),
        )?;
        let cleanup = original.source.cleanup;
        let floor = budget.storage();
        let run = move |budget: &mut ArgumentBudgetV1<'work>| {
            self.source.with_local_read_conditions_v18(native, facts, operation, budget, move |read, budget| {
            let original = self.original();
            let result = (|| {
                source_scalar_normalization_scratch_v18(original.source.cleanup, budget, 0, |budget| {
                    let read = read.ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                        "shared entry region requires an ordinary read"))?;
                    budget.charge_work(24)?;
                    if !matches!(read.pair.origin, GlobalSourceAccessOriginV18::Assertion(_)) {
                        return original.source.missing("shared entry region requires original SharedSlice");
                    }
                    let SliceDefinition::FunctionArgument { function, argument } = read.pair.input.logical.root else {
                        return original.source.missing("shared entry region has no original input parameter");
                    };
                    let root_function = self.original_function;
                    if function.0 as usize != self.physical_function {
                        return original.source.missing("shared entry region changed original root");
                    }
                    let inventory = original.inventory;
                    let input = optimized_source_definition_row_v18(inventory, read.pair.input.logical.root, budget)?;
                    let value = input.value.ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                        "shared entry region original parameter has no value"))?;
                    let slot = usize::try_from(argument).map_err(|_| ArgumentResourceV1::Arithmetic)?;
                    let profile = self.profile;
                    let arguments = &self.arguments;
                    (|| -> Result<(), ProductionSemanticKirErrorV1> {
                        if arguments.semantic_function(budget)? != root_function {
                            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                        }
                        let mut selected = None;
                        let mut matches = 0usize;
                        arguments.visit_nodes_scoped(budget, |node, budget| {
                            budget.charge_work(8)?;
                            if shared_entry_node_matches_v18(&node, slot, value, input.ty) {
                                matches = matches.checked_add(1).ok_or(ArgumentResourceV1::Arithmetic)?;
                                selected = Some((node.source_argument(), node.semantic_type()));
                            }
                            Ok(())
                        })?;
                        if matches != 1 { return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch); }
                        let (ordinal, ty) = selected.ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
                        let entry = profile.shared_entry_v18(ordinal, ty, budget)?
                            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
                        budget.charge_work(8)?;
                        if entry.function() != root_function || entry.argument() != ordinal || entry.ty() != ty
                            || entry.scalar() != read.pair.input.scalar || entry.scalar() != read.pair.output.scalar
                        { return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch); }
                        let mut invoked = 0usize;
                        arguments.visit_nodes_scoped(budget, |node, budget| {
                            budget.charge_work(8)?;
                            if shared_entry_node_matches_v18(&node, slot, value, input.ty) {
                                if invoked != 0 || node.source_argument() != ordinal || node.semantic_type() != ty {
                                    return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                                }
                                invoked = 1;
                                let view = PendingSharedEntryRegionV18 { root: self.root(), source: node, abi: &entry, read };
                                shared_entry_consume_v18(original, budget, |budget| consume(&view, budget))
                                    .map_err(source_slice_query_error_v18)?;
                            }
                            Ok(())
                        })?;
                        if invoked != 1 { return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch); }
                        Ok(())
                    })().map_err(source_argument_error_v18)
                })
            })();
            original.retain_query(result).map_err(Into::into)
        })
        };
        let attempt = move |budget: &mut ArgumentBudgetV1<'work>| {
            let required = budget.storage();
            scoped_source_attempt_v29(cleanup, budget, required, run)
        };
        let storage = original.retain_query(
            shared_entry_headers_v18()
                .and_then(|fixed| {
                    argument_sum_v1(&[
                        fixed,
                        std::mem::size_of_val(&attempt),
                        std::mem::align_of_val(&attempt),
                    ])
                })
                .map_err(Into::into),
        )?;
        original.retain_query(budget.reserve_storage(storage).map_err(Into::into))?;
        // The inner attempt protects the complete fixed frame; the outer one
        // drops the unit attempt and all query borrows before error/unwind refund.
        scoped_source_attempt_v29(cleanup, budget, floor, attempt)?;
        original.retain_query(budget.release_storage(storage).map_err(Into::into))?;
        Ok(())
    }
}

#[cfg(test)]
include!("production_optimized_source_shared_entry_commands_v18_tests.rs");

include!("production_optimized_source_slice_entry_v25.rs");
