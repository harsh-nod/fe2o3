// Source recipes only. Native memory policies, initializedness/read-from,
// ranked effect equivalence and reference/launch authority remain independent.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum DescriptorSourceRoleV18 {
    Read,
    Write,
    Data,
    Length,
    Address,
}

#[derive(Clone, Copy)]
struct DescriptorSourceRoleRowV18 {
    output: SliceOperation,
    input: Option<SliceOperation>,
    instance: Option<usize>,
    site: Option<ProductionSliceAccessSiteV1>,
    role: Option<DescriptorSourceRoleV18>,
    write_recipe_pending: bool,
    global: Option<GlobalSourceAccessPairV18>,
}

#[derive(Clone, Copy)]
struct DescriptorAccessSummaryV18 {
    access: SliceOperation,
    address: SliceOperation,
    data: SliceOperation,
    length: SliceOperation,
    logical: GlobalSourceLogicalEndpointV18,
}

include!("production_optimized_source_global_access_v18.rs");

include!("production_optimized_source_issued_roles_v18.rs");

impl DescriptorAccessSummaryV18 {
    fn from_facts(facts: &SliceFacts<'_>) -> Self {
        Self {
            access: facts.access.operation,
            address: facts.address_operation,
            data: facts.data_operation,
            length: facts.length_operation,
            logical: GlobalSourceLogicalEndpointV18 {
                access: facts.access,
                root: facts.input,
                index: facts.index,
                data: facts.data_operation,
                length: facts.length_operation,
                address: facts.address_operation,
                guard_condition: facts.guard_condition,
                guard_edge: facts.guard_edge,
            },
        }
    }
}

struct DescriptorRoleScopeV18 {
    required: usize,
    slot: usize,
    ledger: fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
}

impl DescriptorRoleScopeV18 {
    fn new(budget: &ArgumentBudgetV1<'_>) -> Self {
        Self {
            required: budget.storage(),
            slot: std::ptr::from_ref(budget) as usize,
            ledger: budget.work_ledger_identity_v1(),
        }
    }

    fn observe(
        &self,
        original: &ProductionSourceCorrespondenceV18<'_>,
        budget: &ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<()> {
        original.observe_custody(budget)?;
        if self.slot != std::ptr::from_ref(budget) as usize
            || self.ledger != budget.work_ledger_identity_v1()
            || budget.storage() < self.required
        {
            original.source.cleanup.deny_refund();
            return Err(ArgumentResourceV1::Accounting.into());
        }
        Ok(())
    }
}

/// Complete actual operation roster with individually checked descriptor roles.
/// None is an unresolved role, including unused address operations. There is
/// deliberately no whole-profile completion or owned conversion operation.
/// Construction still requires the existing whole-root scalar namespace;
/// unsupported namespaces (including volatile scalar Loads) remain refused.
pub(super) struct CheckedDescriptorSourceRolesV18<'s> {
    original: &'s ProductionSourceCorrespondenceV18<'s>,
    optimized: &'s ProductionOptimizedSourceCorrespondenceV18<'s>,
    root: usize,
    rows: &'s [DescriptorSourceRoleRowV18],
    scope: DescriptorRoleScopeV18,
}

fn descriptor_role_key_v18(coordinate: SliceOperation) -> [usize; 3] {
    [
        coordinate.block.function.0 as usize,
        coordinate.block.block as usize,
        coordinate.operation as usize,
    ]
}

impl CheckedDescriptorSourceRolesV18<'_> {
    #[cfg(test)]
    pub(super) fn test_required_storage_v18(&self) -> usize {
        self.scope.required
    }

    #[cfg(test)]
    pub(super) fn test_operation_count_v18(&self) -> usize {
        self.rows.len()
    }

    fn observe_custody(&self, budget: &ArgumentBudgetV1<'_>) -> SourceOwnedResultV18<()> {
        self.scope.observe(self.original, budget)
    }

    fn check(&self, budget: &mut ArgumentBudgetV1<'_>) -> SourceOwnedResultV18<()> {
        self.original.retain_query(self.observe_custody(budget))?;
        self.optimized
            .check_exact_original_v18(self.original, budget)?;
        optimized_source_endpoints_v18(self.original, self.optimized, budget)?;
        let function =
            optimized_source_root_function_v18(self.original, self.optimized, self.root, budget)?;
        self.original.retain_query((|| {
            budget.charge_work(1)?;
            if function.operations.len() != self.rows.len() {
                return self
                    .original
                    .source
                    .missing("descriptor role operation census changed");
            }
            Ok(())
        })())
    }

    pub(super) fn role(
        &self,
        operation: SliceOperation,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<Option<DescriptorSourceRoleV18>> {
        self.check(budget)?;
        self.original.retain_query((|| {
            let index = descriptor_role_index_v18(self.rows, operation, budget)?;
            let row = self
                .rows
                .get(index)
                .filter(|row| row.output == operation)
                .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                    "descriptor role query changed actual occurrence",
                ))?;
            if row.write_recipe_pending {
                return Ok(None);
            }
            Ok(row.role)
        })())
    }
}

fn descriptor_role_index_v18(
    rows: &[DescriptorSourceRoleRowV18],
    operation: SliceOperation,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<usize> {
    private_array_partition_v1(
        rows,
        |row| descriptor_role_key_v18(row.output),
        descriptor_role_key_v18(operation),
        false,
        &mut SourceCorrespondenceWorkV18(budget),
    )
}

pub(super) fn descriptor_role_outer_headers_v18<T, E>(
    callback: usize,
    alignment: usize,
) -> Result<usize, ArgumentResourceV1> {
    argument_sum_v1(&[
        callback,
        argument_product_v1(2, alignment)?,
        size_of::<DescriptorRoleScopeV18>(),
        size_of::<[usize; 2]>(),
        argument_product_v1(2, size_of::<SourceOwnedResultV18<usize>>())?,
        size_of::<std::thread::Result<SourceOwnedResultV18<usize>>>(),
        size_of::<std::thread::Result<Result<T, E>>>(),
        size_of::<Result<T, E>>(),
        size_of::<Option<SourceOwnedQueryFailureV18>>(),
        size_of::<SourceOwnedResultV18<()>>(),
        size_of::<Option<&fe2o3_pliron::ProductionRankedKernelV1>>(),
        size_of::<(
            &ProductionSourceCorrespondenceV18<'_>,
            &ProductionOptimizedSourceCorrespondenceV18<'_>,
            usize,
            Option<&fe2o3_pliron::ProductionRankedKernelV1>,
            &mut ArgumentBudgetV1<'_>,
        )>(),
        size_of::<
            std::panic::AssertUnwindSafe<(
                &ProductionSourceCorrespondenceV18<'_>,
                &ProductionOptimizedSourceCorrespondenceV18<'_>,
                usize,
                Option<&fe2o3_pliron::ProductionRankedKernelV1>,
                &mut ArgumentBudgetV1<'_>,
            )>,
        >(),
        size_of::<(
            &ProductionSourceCorrespondenceV18<'_>,
            &ProductionOptimizedSourceCorrespondenceV18<'_>,
            usize,
        )>(),
        size_of::<(
            &CheckedDescriptorSourceRolesV18<'_>,
            &mut ArgumentBudgetV1<'_>,
        )>(),
        source_reference_cleanup_headers_v29()?,
    ])
}

fn descriptor_role_headers_v18<T, E>() -> Result<usize, ArgumentResourceV1> {
    argument_sum_v1(&[
        size_of::<[usize; 2]>(),
        size_of::<CheckedDescriptorSourceRolesV18<'_>>(),
        size_of::<Vec<DescriptorSourceRoleRowV18>>(),
        size_of::<(Vec<DescriptorSourceRoleRowV18>, usize)>(),
        argument_product_v1(
            2,
            size_of::<SourceOwnedResultV18<Vec<DescriptorSourceRoleRowV18>>>(),
        )?,
        argument_product_v1(
            2,
            size_of::<SourceOwnedResultV18<(Vec<DescriptorSourceRoleRowV18>, usize)>>(),
        )?,
        size_of::<
            std::thread::Result<SourceOwnedResultV18<(Vec<DescriptorSourceRoleRowV18>, usize)>>,
        >(),
        size_of::<std::thread::Result<Result<T, E>>>(),
        size_of::<Result<T, E>>(),
        size_of::<Option<SourceOwnedQueryFailureV18>>(),
        size_of::<SourceOwnedResultV18<()>>(),
        size_of::<SourceOwnedResultV18<usize>>(),
        size_of::<SourceOwnedResultV18<Option<DescriptorSourceRoleV18>>>(),
        size_of::<Option<ProductionSliceAccessSiteV1>>(),
        size_of::<ProductionSliceAccessSiteV1>(),
        size_of::<SourceOwnedResultV18<Option<ProductionSliceAccessSiteV1>>>(),
        size_of::<DescriptorSourceRoleRowV18>(),
        size_of::<Option<&DescriptorSourceRoleRowV18>>(),
        size_of::<Option<&mut DescriptorSourceRoleRowV18>>(),
        size_of::<SourceOwnedResultV18<&DescriptorSourceRoleRowV18>>(),
        size_of::<SourceOwnedResultV18<&mut DescriptorSourceRoleRowV18>>(),
        size_of::<[(SliceOperation, SliceOperation, DescriptorSourceRoleV18); 4]>(),
        size_of::<std::array::IntoIter<(SliceOperation, SliceOperation, DescriptorSourceRoleV18), 4>>(
        ),
        argument_product_v1(2, size_of::<DescriptorAccessSummaryV18>())?,
        argument_product_v1(
            2,
            size_of::<SourceOwnedResultV18<DescriptorAccessSummaryV18>>(),
        )?,
        size_of::<Option<ProductionSemanticExpressionV2>>(),
        size_of::<ProductionSemanticExpressionV2>(),
        size_of::<SourceOwnedResultV18<Option<ProductionSemanticExpressionV2>>>(),
        size_of::<Option<SourcePhysicalAccessV18<'_>>>(),
        size_of::<SourcePhysicalAccessV18<'_>>(),
        size_of::<SourceOwnedResultV18<Option<SourcePhysicalAccessV18<'_>>>>(),
        size_of::<Option<SourcePhysicalPayloadV18<'_>>>(),
        size_of::<SourcePhysicalPayloadV18<'_>>(),
        size_of::<SourceOwnedResultV18<Option<SourcePhysicalPayloadV18<'_>>>>(),
        size_of::<ProductionSourceScalarInputV18<'_>>(),
        size_of::<SourceOwnedResultV18<ProductionSourceScalarInputV18<'_>>>(),
        global_source_headers_v18()?,
        size_of::<Type>(),
        size_of::<Result<Type, ProductionSemanticKirErrorV1>>(),
        size_of::<Constant>(),
        size_of::<Result<Constant, ProductionSemanticKirErrorV1>>(),
        source_reference_cleanup_headers_v29()?,
    ])
}

impl ProductionSourceCorrespondenceV18<'_> {
    pub(super) fn with_descriptor_source_roles_v18<'work, T, E>(
        &self,
        optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
        root: usize,
        recipe: &fe2o3_pliron::ProductionRankedKernelV1,
        budget: &mut ArgumentBudgetV1<'work>,
        consume: impl for<'scope> FnOnce(
            &CheckedDescriptorSourceRolesV18<'scope>,
            &mut ArgumentBudgetV1<'work>,
        ) -> Result<T, E>,
    ) -> Result<T, E>
    where
        E: From<ProductionSourceOwnedViewErrorV18>,
    {
        self.with_descriptor_source_roles_namespace_v18(
            optimized,
            root,
            Some(recipe),
            budget,
            consume,
        )
    }

    fn with_descriptor_source_roles_namespace_v18<'work, T, E>(
        &self,
        optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
        root: usize,
        recipe: Option<&fe2o3_pliron::ProductionRankedKernelV1>,
        budget: &mut ArgumentBudgetV1<'work>,
        consume: impl for<'scope> FnOnce(
            &CheckedDescriptorSourceRolesV18<'scope>,
            &mut ArgumentBudgetV1<'work>,
        ) -> Result<T, E>,
    ) -> Result<T, E>
    where
        E: From<ProductionSourceOwnedViewErrorV18>,
    {
        // Exact subject/custody precedes scope entry and its new storage/work.
        optimized.check_exact_original_v18(self, budget)?;
        optimized_source_endpoints_v18(self, optimized, budget)?;
        let outer_floor = budget.storage();
        let outer_retained =
            scoped_source_attempt_v29(self.source.cleanup, budget, outer_floor, |budget| {
                self.source.retain_construction(|| {
                    let headers = descriptor_role_outer_headers_v18::<T, E>(
                        std::mem::size_of_val(&consume),
                        std::mem::align_of_val(&consume),
                    )?;
                    budget.reserve_storage(headers)?;
                    Ok(headers)
                })
            })?;
        let outer_scope = DescriptorRoleScopeV18::new(budget);
        let outer_caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let consume_leaves =
                |leaves: &ProductionOptimizedSourceScalarLeavesV18<'_>,
                 budget: &mut ArgumentBudgetV1<'work>| {
                    let floor = budget.storage();
                    let (rows, retained) =
                        scoped_source_attempt_v29(self.source.cleanup, budget, floor, |budget| {
                            self.source.retain_construction(|| {
                                budget.reserve_storage(descriptor_role_headers_v18::<T, E>()?)?;
                                let rows = build_descriptor_source_roles_v18(
                                    self, optimized, root, leaves, budget,
                                )?;
                                let retained = budget
                                    .storage()
                                    .checked_sub(floor)
                                    .ok_or(ArgumentResourceV1::Accounting)?;
                                Ok((rows, retained))
                            })
                        })?;
                    let view = CheckedDescriptorSourceRolesV18 {
                        original: self,
                        optimized,
                        root,
                        rows: &rows,
                        scope: DescriptorRoleScopeV18::new(budget),
                    };
                    let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        consume(&view, budget)
                    }));
                    let prior = self.source.guard.first.get();
                    let postflight = if matches!(&caught, Ok(Ok(_))) {
                        view.check(budget)
                    } else {
                        view.observe_custody(budget)
                    };
                    drop(view);
                    drop(rows);
                    source_owned_finish_callback_v18(
                        caught,
                        prior,
                        postflight,
                        self.source.cleanup,
                        budget,
                        retained,
                    )
                };
            match recipe {
                Some(recipe) => self.with_optimized_scalar_leaves_v18(
                    optimized,
                    root,
                    recipe,
                    budget,
                    consume_leaves,
                ),
                None => self.with_optimized_source_scalar_leaves_v18(
                    optimized,
                    root,
                    budget,
                    consume_leaves,
                ),
            }
        }));
        let outer_prior = self.source.guard.first.get();
        let outer_postflight = if matches!(&outer_caught, Ok(Ok(_))) {
            self.retain_query(outer_scope.observe(self, budget))
                .and_then(|()| self.check(budget))
        } else {
            outer_scope.observe(self, budget)
        };
        drop(outer_scope);
        source_owned_finish_callback_v18(
            outer_caught,
            outer_prior,
            outer_postflight,
            self.source.cleanup,
            budget,
            outer_retained,
        )
    }
}

fn install_descriptor_role_v18(
    original: &ProductionSourceCorrespondenceV18<'_>,
    optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    rows: &mut [DescriptorSourceRoleRowV18],
    input: SliceOperation,
    output: SliceOperation,
    instance: usize,
    site: Option<ProductionSliceAccessSiteV1>,
    role: DescriptorSourceRoleV18,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<()> {
    budget.charge_work(3)?;
    let ProductionOptimizedSourceOperationV18::Retained {
        output: retained, ..
    } = optimized.operation(input, budget)?
    else {
        // A substituted pure dependency is not an exact retained recipe here.
        return Ok(());
    };
    if retained != output {
        return original
            .source
            .missing("descriptor role changed actual dependency");
    }
    let index = descriptor_role_index_v18(rows, output, budget)?;
    let row = rows
        .get_mut(index)
        .filter(|row| row.output == output)
        .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
            "descriptor role output is absent",
        ))?;
    let source_access = matches!(
        role,
        DescriptorSourceRoleV18::Read | DescriptorSourceRoleV18::Write
    );
    let instance = source_access.then_some(instance);
    let site = if source_access { site } else { None };
    if row.role.is_some()
        && (row.input != Some(input)
            || row.role != Some(role)
            || row.instance != instance
            || row.site != site)
    {
        return original
            .source
            .missing("descriptor role has conflicting original recipes");
    }
    row.input = Some(input);
    // Address dependencies retain their own original operation identity. The
    // access's semantic site is not misrepresented as the dependency's site.
    row.instance = instance;
    row.site = site;
    row.role = Some(role);
    row.write_recipe_pending = role == DescriptorSourceRoleV18::Write;
    Ok(())
}

fn build_descriptor_source_roles_v18(
    original: &ProductionSourceCorrespondenceV18<'_>,
    optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    root: usize,
    leaves: &ProductionOptimizedSourceScalarLeavesV18<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<Vec<DescriptorSourceRoleRowV18>> {
    let output = optimized.output_inventory(budget)?;
    let function = optimized_source_root_function_v18(original, optimized, root, budget)?;
    let mut rows =
        emission_vec_v1(function.operations.len(), budget).map_err(source_emission_error_v18)?;
    for operation in &output.operations()[function.operations.clone()] {
        budget.charge_work(1)?;
        rows.push(DescriptorSourceRoleRowV18 {
            output: operation.coordinate,
            input: None,
            instance: None,
            site: None,
            role: None,
            write_recipe_pending: false,
            global: None,
        });
    }
    install_optimized_issued_roles_v18(original, optimized, root, leaves, &mut rows, budget)?;
    let physical = original.source.root(root, budget)?.1;
    let input = original.inventory.functions().get(physical).ok_or(
        ProductionSourceOwnedViewErrorV18::Binding("descriptor original root is absent"),
    )?;
    for operation in &original.inventory.operations()[input.operations.clone()] {
        budget.charge_work(1)?;
        let (pointer, access, write) = match &operation.operation.kind {
            OperationKind::Load { pointer, access } => (*pointer, *access, false),
            OperationKind::Store {
                pointer, access, ..
            } => (*pointer, *access, true),
            _ => continue,
        };
        if access.volatile
            || !matches!(
                access.address_space,
                AddressSpace::Global | AddressSpace::Generic
            )
        {
            continue;
        }
        let Some(anchor) =
            original.retained_memory_access(root, operation.coordinate, pointer, budget)?
        else {
            continue;
        };
        let Some(payload) =
            original.retained_scalar_payload_v18(root, operation.coordinate, &anchor, budget)?
        else {
            continue;
        };
        match (write, payload.source) {
            (false, ScopedMemoryPayloadV29::Load { .. }) => {
                let [result] = operation.operation.results.as_slice() else {
                    return original.source.missing("descriptor read result census");
                };
                if kir_semantic_scalar_v1(&result.ty).is_none() {
                    continue;
                }
                let Some(leaf) = leaves
                    .original
                    .leaves
                    .find([1, payload.value.0 as usize, 0], budget)?
                else {
                    continue;
                };
                if leaf.operation != operation.coordinate || leaf.value != payload.value {
                    return original
                        .source
                        .missing("descriptor read changed original scalar leaf");
                }
            }
            (true, ScopedMemoryPayloadV29::Store { .. }) => {}
            _ => continue,
        }
        let Some(site) =
            descriptor_source_site_v18(original, root, &anchor, operation.coordinate, budget)?
        else {
            continue;
        };
        let ProductionOptimizedSourceOperationV18::Retained { .. } =
            optimized.operation(operation.coordinate, budget)?
        else {
            continue;
        };
        let SemanticKirOptimizedAssertOutcomeV1::Conditional { .. } =
            optimized.assertion(root, anchor.instance, site.assertion, budget)?
        else {
            continue;
        };
        let before = original.with_descriptor_access_v18(
            root,
            anchor.instance,
            site,
            write,
            budget,
            |view, _| Ok(DescriptorAccessSummaryV18::from_facts(view.facts)),
        )?;
        if before.access != operation.coordinate {
            return original
                .source
                .missing("descriptor access ordinal changed original operation");
        }
        let after = original.with_optimized_descriptor_access_v18(
            optimized,
            root,
            anchor.instance,
            site,
            write,
            budget,
            |view, _| Ok(DescriptorAccessSummaryV18::from_facts(view.facts)),
        )?;
        install_global_source_access_v18(
            original,
            optimized,
            &mut rows,
            anchor.instance,
            GlobalSourceAccessOriginV18::Assertion(site),
            before.logical,
            after.logical,
            budget,
        )?;
        for (input, output, role) in [
            (
                before.access,
                after.access,
                if write {
                    DescriptorSourceRoleV18::Write
                } else {
                    DescriptorSourceRoleV18::Read
                },
            ),
            (
                before.address,
                after.address,
                DescriptorSourceRoleV18::Address,
            ),
            (before.data, after.data, DescriptorSourceRoleV18::Data),
            (before.length, after.length, DescriptorSourceRoleV18::Length),
        ] {
            install_descriptor_role_v18(
                original,
                optimized,
                &mut rows,
                input,
                output,
                anchor.instance,
                Some(site),
                role,
                budget,
            )?;
        }
    }
    leaves.visit_store_inputs(budget, |disposition, budget| -> SourceOwnedResultV18<()> {
        let ProductionOptimizedSourceScalarStoreDispositionV18::Retained(request) = disposition
        else {
            return Ok(());
        };
        let index = descriptor_role_index_v18(&rows, request.output, budget)?;
        let Some(row) = rows
            .get_mut(index)
            .filter(|row| row.output == request.output && row.write_recipe_pending)
        else {
            return Ok(());
        };
        let Some(expression) = descriptor_store_expression_v18(leaves, &request, budget)? else {
            return Ok(());
        };
        request.original.check_expression(&expression, budget)?;
        request.check_expression(&expression, budget)?;
        drop(expression);
        row.write_recipe_pending = false;
        Ok(())
    })?;
    Ok(rows)
}

fn descriptor_source_site_v18(
    original: &ProductionSourceCorrespondenceV18<'_>,
    root: usize,
    access: &SourcePhysicalAccessV18<'_>,
    operation: SliceOperation,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<Option<ProductionSliceAccessSiteV1>> {
    budget.charge_work(5)?;
    let Some(ScopedMemoryFrameV29 {
        site: ExecutionSiteV29::Statement { block, statement },
        role: Some(ScopedMemoryRoleV29::Operand(role)),
    }) = access.anchor.source
    else {
        return Ok(None);
    };
    let semantic = original.source.source_semantic(budget)?;
    let root_function = original.source.root(root, budget)?.0;
    let (source_function, _) = original.source.instance(root, access.instance, budget)?;
    if semantic
        .select_kernel_body_for_root_v1(root_function)
        .is_none_or(|selected| selected.body() != source_function)
    {
        return Ok(None);
    }
    let function = semantic
        .functions()
        .get(source_function.index() as usize)
        .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
            "descriptor source declaration is absent",
        ))?;
    let Some(place) = scoped_payload_place_v29(
        function,
        ExecutionSiteV29::Statement { block, statement },
        role,
    ) else {
        return Ok(None);
    };
    if !matches!(
        place
            .projections()
            .last()
            .map(|projection| projection.kind()),
        Some(SemanticProjectionKindV1::Index(_))
    ) {
        return Ok(None);
    }
    let source_block = SemanticBlockIdV1::from_index(block.get());
    let mut assertion = None;
    for (index, block) in function.blocks().iter().enumerate() {
        budget.charge_work(2)?;
        if let SemanticTerminatorKindV1::Assert {
            expected: true,
            target,
            message: SemanticAssertMessageV1::BoundsCheck { .. },
            ..
        } = block.terminator().kind()
            && target.target() == source_block
        {
            if assertion
                .replace(SemanticBlockIdV1::from_index(
                    u32::try_from(index).map_err(|_| ArgumentResourceV1::Arithmetic)?,
                ))
                .is_some()
            {
                return Ok(None);
            }
        }
    }
    let Some(assertion) = assertion else {
        return Ok(None);
    };
    let mut ordinal = 0u32;
    let mut selected = None;
    for row in original.source_operation_rows(
        root,
        access.instance,
        source_block,
        Some(statement),
        budget,
    )? {
        let ProductionSourceOperationV18::Operation(coordinate) =
            original.mapped_source_operation(row.location, budget)?
        else {
            continue;
        };
        let actual = optimized_source_operation_row_v18(original.inventory, coordinate, budget)?;
        try_visit_kir_memory_accesses_v1(actual.operation, |(_, _, space, _)| -> SliceResult<()> {
            budget.charge_work(2)?;
            if space == dialect_kernel::MemorySpaceAttr::Private {
                return Ok(());
            }
            if coordinate == operation && selected.replace(ordinal).is_some() {
                return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
            }
            ordinal = ordinal
                .checked_add(1)
                .ok_or(ArgumentResourceV1::Arithmetic)?;
            Ok(())
        })
        .map_err(source_emission_error_v18)?;
    }
    let access = selected.ok_or(ProductionSourceOwnedViewErrorV18::Binding(
        "descriptor source access is outside its original span",
    ))?;
    Ok(Some(ProductionSliceAccessSiteV1::new(
        root_function,
        source_function,
        source_block,
        Some(statement),
        access,
        assertion,
    )))
}

fn descriptor_store_expression_v18(
    leaves: &ProductionOptimizedSourceScalarLeavesV18<'_>,
    request: &ProductionOptimizedSourceScalarStoreV18<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<Option<ProductionSemanticExpressionV2>> {
    let (instance, function) = request.original(budget)?;
    let original = leaves.original_leaves(budget)?;
    let relation = original.leaves.relation;
    budget.charge_work(4)?;
    let operand = match request.input_for(function, budget)? {
        ProductionSourceScalarInputV18::Operand { operand, .. } => operand,
        ProductionSourceScalarInputV18::Assignment {
            assignment: SemanticStatementKindV1::Assign(assignment),
            ..
        } => match assignment.value().kind() {
            SemanticRvalueKindV1::Use(operand) => operand,
            _ => return Ok(None),
        },
        _ => return Ok(None),
    };
    match operand {
        SemanticOperandV1::Constant(constant) => {
            let SemanticConstantValueV1::Scalar(value) = constant.value() else {
                return Ok(None);
            };
            let scalar = lower_scalar_type(
                relation.source.source_semantic(budget)?.types(),
                constant.ty(),
            )
            .map_err(source_emission_error_v18)?;
            let constant = lower_constant(scalar, *value).map_err(source_emission_error_v18)?;
            let Some((scalar, bits)) = normalize_kir_constant_v1(&constant) else {
                return Ok(None);
            };
            Ok(Some(ProductionSemanticExpressionV2::Constant {
                scalar,
                bits,
            }))
        }
        SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place) => {
            if let Some(expression) = original.original_place(instance, function, place, budget)? {
                return Ok(Some(expression));
            }
            // Only the exact captured original Store operand is used here.
            // The existing read-leaf table and whole-value transport are the
            // recipe producers; arithmetic is not reconstructed from syntax.
            let Some(origin) = request
                .original
                .origins
                .resolve(request.original.value, budget)
                .map_err(source_pointer_inventory_error_v18)?
            else {
                return Ok(None);
            };
            match origin {
                SliceDefinition::Result { .. } => {
                    let Some(value) = request
                        .original
                        .origins
                        .operation_origin(request.original.value, budget)
                        .map_err(source_pointer_inventory_error_v18)?
                    else {
                        return Ok(None);
                    };
                    let Some(read) = original.leaves.find([1, value.0 as usize, 0], budget)? else {
                        return Ok(None);
                    };
                    Ok(Some(ProductionSemanticExpressionV2::Symbol {
                        symbol: read.symbol,
                        scalar: read.scalar,
                    }))
                }
                SliceDefinition::FunctionArgument {
                    function: actual,
                    argument,
                } if place.projections().is_empty() => {
                    let physical = relation.source.root(original.leaves.root, budget)?.1;
                    let row = relation.inventory.functions().get(physical).ok_or(
                        ProductionSourceOwnedViewErrorV18::Binding(
                            "descriptor scalar argument root is absent",
                        ),
                    )?;
                    if row.coordinate != actual {
                        return relation
                            .source
                            .missing("descriptor scalar argument changed root");
                    }
                    let Some(SemanticLocalRoleV1::Argument(source_argument)) = function
                        .locals()
                        .get(place.local().index() as usize)
                        .map(|local| local.role())
                    else {
                        return Ok(None);
                    };
                    let ProductionSourceScalarArgumentV18::Root { argument: expected } =
                        original.original_argument(instance, function, source_argument, budget)?
                    else {
                        return Ok(None);
                    };
                    if request.original.arguments.original_argument(
                        row.function,
                        argument,
                        budget,
                    )? != expected
                    {
                        return relation
                            .source
                            .missing("descriptor Store changed its original scalar argument");
                    }
                    let symbol = PRODUCTION_KERNEL_SCALAR_SYMBOL_BASE_V2
                        .checked_add(expected)
                        .filter(|symbol| {
                            *symbol < fe2o3_pliron::PRODUCTION_SEMANTIC_LOAD_SYMBOL_BASE_V2
                        })
                        .ok_or(ArgumentResourceV1::Arithmetic)?;
                    Ok(Some(ProductionSemanticExpressionV2::Symbol {
                        symbol,
                        scalar: request.scalar(budget)?,
                    }))
                }
                _ => Ok(None),
            }
        }
    }
}

#[cfg(test)]
mod descriptor_role_resources_v18 {
    use super::*;
    include!("production_optimized_source_descriptor_role_resources_v18_tests.rs");
}
