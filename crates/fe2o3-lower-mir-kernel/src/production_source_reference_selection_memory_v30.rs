// Selected memory receipts retain an ordered pointer program and the separate
// source-derived leaf obligations. They are not a union of access authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PendingSourceSelectedDescriptorV30 {
    instance: ProductionCallInstanceIdV1,
    descriptor: usize,
    slice: ValueId,
    original_index: ValueId,
    original_scalar: ScalarType,
    data: ValueId,
    index: ValueId,
    pointer: ValueId,
    formation: (BlockId, u32),
    element: ScalarType,
    space: AddressSpace,
    access: AccessMode,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PendingSourceSelectedLeafOriginV30 {
    Issued(PendingSourceIssuedIssuerV29),
    Descriptor(PendingSourceSelectedDescriptorV30),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PendingSourceSelectedLeafV30 {
    node: usize,
    origin: PendingSourceSelectedLeafOriginV30,
    pointer: ValueId,
    first_guard: usize,
    guard_count: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PendingSourceSelectedGuardV30 {
    leaf: usize,
    original_descriptor_guard: Option<usize>,
    condition: ValueId,
    block: BlockId,
    edge: usize,
}

#[derive(Clone, Copy)]
struct SourceSelectedDescriptorGuardV30 {
    instance: ProductionCallInstanceIdV1,
    index: SsaValueV1,
    slice: ValueId,
    original_index: ValueId,
    original_scalar: ScalarType,
    retained: PendingSourceSelectedGuardV30,
}

impl SourceSelectedDescriptorGuardV30 {
    fn key(self) -> (usize, SsaValueV1, ValueId) {
        (self.instance.index(), self.index, self.slice)
    }
}

struct SourceSelectedGuardCacheV30 {
    plan: usize,
    issued: Vec<SourceIssuedGuardV29>,
    descriptors: Option<Vec<SourceSelectedDescriptorGuardV30>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PendingSourceSelectedUseV30 {
    Access,
    Incoming(usize),
    Invocation(usize),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PendingSourceSelectedObligationV30 {
    leaf: usize,
    usage: PendingSourceSelectedUseV30,
    block: BlockId,
    guard: Option<usize>,
    at_access: bool,
}

struct PendingSourceSelectedAccessV30 {
    instance: ProductionCallInstanceIdV1,
    anchor: usize,
    subject: SourceReferenceSelectionSubjectV29,
    selection: SourceReferenceSelectionActualV30,
    pointer: ValueId,
    value: ValueId,
    operation: (BlockId, u32),
    memory: MemoryAccess,
    writing: bool,
    leaves: Vec<PendingSourceSelectedLeafV30>,
    guards: Vec<PendingSourceSelectedGuardV30>,
    obligations: Vec<PendingSourceSelectedObligationV30>,
}

#[cfg(test)]
thread_local! {
    static SOURCE_SELECTED_ACCESS_ATTEMPTS_V30: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

fn source_reference_selection_memory_error_v30() -> ProductionSemanticKirErrorV1 {
    source_reference_error_v29(
        "selected reference access differs from its conditional source obligations",
    )
}

impl PendingSourceSelectedAccessV30 {
    fn retained_storage(&self) -> Result<usize, ArgumentResourceV1> {
        argument_sum_v1(&[
            argument_product_v1(
                self.selection.nodes.capacity(),
                std::mem::size_of::<SourceReferenceSelectionActualNodeV30>(),
            )?,
            argument_product_v1(
                self.selection.edges.capacity(),
                std::mem::size_of::<SourceReferenceSelectionActualEdgeV30>(),
            )?,
            argument_product_v1(
                self.leaves.capacity(),
                std::mem::size_of::<PendingSourceSelectedLeafV30>(),
            )?,
            argument_product_v1(
                self.guards.capacity(),
                std::mem::size_of::<PendingSourceSelectedGuardV30>(),
            )?,
            argument_product_v1(
                self.obligations.capacity(),
                std::mem::size_of::<PendingSourceSelectedObligationV30>(),
            )?,
        ])
    }

    fn matches(
        &self,
        other: &Self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<bool, ProductionSemanticKirErrorV1> {
        budget.charge_work(15)?;
        if self.instance != other.instance
            || self.anchor != other.anchor
            || self.subject != other.subject
            || self.pointer != other.pointer
            || self.value != other.value
            || self.operation != other.operation
            || self.memory != other.memory
            || self.writing != other.writing
            || self.selection.nodes.len() != other.selection.nodes.len()
            || self.selection.edges.len() != other.selection.edges.len()
            || self.leaves.len() != other.leaves.len()
            || self.guards.len() != other.guards.len()
            || self.obligations.len() != other.obligations.len()
        {
            return Ok(false);
        }
        budget.charge_work(argument_sum_v1(&[
            argument_product_v1(
                self.selection.nodes.len(),
                std::mem::size_of::<SourceReferenceSelectionActualNodeV30>(),
            )?,
            argument_product_v1(
                self.selection.edges.len(),
                std::mem::size_of::<SourceReferenceSelectionActualEdgeV30>(),
            )?,
            argument_product_v1(
                self.leaves.len(),
                std::mem::size_of::<PendingSourceSelectedLeafV30>(),
            )?,
            argument_product_v1(
                self.guards.len(),
                std::mem::size_of::<PendingSourceSelectedGuardV30>(),
            )?,
            argument_product_v1(
                self.obligations.len(),
                std::mem::size_of::<PendingSourceSelectedObligationV30>(),
            )?,
        ])?)?;
        Ok(self.selection.nodes == other.selection.nodes
            && self.selection.edges == other.selection.edges
            && self.leaves == other.leaves
            && self.guards == other.guards
            && self.obligations == other.obligations)
    }
}

fn copied_selected_rows_v30(
    rows: &[PendingSourceSelectedAccessV30],
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<Vec<PendingSourceSelectedAccessV30>, ProductionSemanticKirErrorV1> {
    fn copied<T: Copy>(
        rows: &[T],
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Vec<T>, ProductionSemanticKirErrorV1> {
        budget.reserve_storage(source_reference_emission_headers_v29::<Vec<T>>()?)?;
        let mut output = emission_vec_v1(rows.len(), budget)?;
        budget.charge_work(argument_product_v1(rows.len(), std::mem::size_of::<T>())?)?;
        output.extend_from_slice(rows);
        Ok(output)
    }
    budget.reserve_storage(source_reference_selection_memory_headers_v30()?)?;
    let mut output = emission_vec_v1(rows.len(), budget)?;
    for row in rows {
        budget.charge_work(8)?;
        output.push(PendingSourceSelectedAccessV30 {
            instance: row.instance,
            anchor: row.anchor,
            subject: row.subject,
            selection: SourceReferenceSelectionActualV30 {
                nodes: copied(&row.selection.nodes, budget)?,
                edges: copied(&row.selection.edges, budget)?,
            },
            pointer: row.pointer,
            value: row.value,
            operation: row.operation,
            memory: row.memory,
            writing: row.writing,
            leaves: copied(&row.leaves, budget)?,
            guards: copied(&row.guards, budget)?,
            obligations: copied(&row.obligations, budget)?,
        });
    }
    Ok(output)
}

fn source_reference_selection_memory_headers_v30() -> Result<usize, ArgumentResourceV1> {
    argument_sum_v1(&[
        source_reference_emission_headers_v29::<PendingSourceSelectedAccessV30>()?,
        source_reference_emission_headers_v29::<PendingSourceSelectedDescriptorV30>()?,
        source_reference_emission_headers_v29::<PendingSourceSelectedLeafV30>()?,
        source_reference_emission_headers_v29::<PendingSourceSelectedGuardV30>()?,
        source_reference_emission_headers_v29::<PendingSourceSelectedObligationV30>()?,
        source_reference_emission_headers_v29::<Vec<PendingSourceSelectedAccessV30>>()?,
        source_reference_emission_headers_v29::<Vec<PendingSourceSelectedLeafV30>>()?,
        source_reference_emission_headers_v29::<Vec<PendingSourceSelectedGuardV30>>()?,
        source_reference_emission_headers_v29::<Vec<PendingSourceSelectedObligationV30>>()?,
        source_reference_emission_headers_v29::<Vec<usize>>()?,
        source_reference_emission_headers_v29::<Vec<Option<usize>>>()?,
        source_reference_emission_headers_v29::<Vec<bool>>()?,
        source_reference_emission_headers_v29::<Vec<(usize, BlockId, Option<usize>)>>()?,
        source_reference_emission_headers_v29::<(usize, BlockId, Option<usize>)>()?,
        source_reference_emission_headers_v29::<SourceSelectedDescriptorGuardV30>()?,
        source_reference_emission_headers_v29::<Vec<SourceSelectedDescriptorGuardV30>>()?,
        source_reference_emission_headers_v29::<SourceSelectedGuardCacheV30>()?,
        source_reference_emission_headers_v29::<Option<Vec<SourceSelectedDescriptorGuardV30>>>()?,
        source_reference_emission_headers_v29::<Vec<SourceIssuedGuardV29>>()?,
        source_reference_emission_headers_v29::<SourceAddressValueAccessV29>()?,
        source_reference_emission_headers_v29::<SourceIssuedRecipeV29>()?,
        source_reference_emission_headers_v29::<(
            SourceIssuedRootTransportV29,
            PendingSourceIssuedIssuerV29,
        )>()?,
        source_reference_emission_headers_v29::<SourceIssuedActualValueV29<'_>>()?,
        source_reference_emission_headers_v29::<SourceSelectorDefinitionV29<'_>>()?,
        source_reference_emission_headers_v29::<CheckedSourceDescriptorGuardV29<'_>>()?,
        source_reference_emission_headers_v29::<Option<CheckedSourceDescriptorGuardV29<'_>>>()?,
        source_reference_emission_headers_v29::<SourceReferenceDescriptorUseV29>()?,
        source_reference_emission_headers_v29::<SourceReferenceDescriptorGuardUseV29>()?,
        source_reference_emission_headers_v29::<&SemanticValueBindingV1>()?,
        source_reference_emission_headers_v29::<&Operation>()?,
        source_reference_emission_headers_v29::<Option<(BlockId, u32)>>()?,
        source_reference_emission_headers_v29::<(ValueId, ScalarType)>()?,
        source_reference_emission_headers_v29::<()>()?,
        std::mem::size_of::<Result<(), fe2o3_kernel_ir::FunctionControlFlowScopeErrorV1>>(),
    ])
}

impl SourceIssuedAccessesV29<'_, '_, '_> {
    fn prepare_selected_guards_v30(
        &mut self,
        references: &SourceReferenceEmissionV29<'_, '_>,
        descriptors: bool,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        budget.charge_work(3)?;
        let plan = std::ptr::from_ref(references.plan) as usize;
        if let Some(cache) = &self.selected_guards {
            if cache.plan != plan {
                return Err(ArgumentResourceV1::Accounting.into());
            }
        } else {
            self.selected_guards = Some(SourceSelectedGuardCacheV30 {
                plan,
                issued: source_issued_guards_v29(
                    &self.source_index.pending.function,
                    &self.actual,
                    budget,
                )?,
                descriptors: None,
            });
        }
        if descriptors
            && self
                .selected_guards
                .as_ref()
                .is_some_and(|cache| cache.descriptors.is_none())
        {
            let guards = self.selected_descriptor_guards_v30(references, budget)?;
            self.selected_guards
                .as_mut()
                .ok_or(ArgumentResourceV1::Accounting)?
                .descriptors = Some(guards);
        }
        Ok(())
    }

    fn finish_selected_v30(
        &mut self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        budget.charge_work(1)?;
        if self.retained.selected.is_empty() {
            return Ok(());
        }
        budget.reserve_storage(source_reference_selection_memory_headers_v30()?)?;
        call_splice_sort_work_v1(self.retained.selected.len(), budget)
            .map_err(source_address_call_error_v29)?;
        self.retained
            .selected
            .sort_unstable_by_key(|row| (row.instance.index(), row.anchor));
        for pair in self.retained.selected.windows(2) {
            budget.charge_work(2)?;
            if (pair[0].instance, pair[0].anchor) == (pair[1].instance, pair[1].anchor) {
                return Err(source_reference_selection_memory_error_v30());
            }
        }
        for legacy in &self.retained.accesses {
            charge_execution_cfg_lookup_v29(self.retained.selected.len(), budget)?;
            if self
                .retained
                .selected
                .binary_search_by_key(&(legacy.instance.index(), legacy.anchor), |row| {
                    (row.instance.index(), row.anchor)
                })
                .is_ok()
            {
                return Err(source_reference_selection_memory_error_v30());
            }
        }
        let mut operations = emission_vec_v1(self.retained.selected.len(), budget)?;
        for (index, selected) in self.retained.selected.iter().enumerate() {
            budget.charge_work(1)?;
            operations.push(index);
            for leaf in &selected.leaves {
                budget.charge_work(1)?;
                let PendingSourceSelectedLeafOriginV30::Issued(issuer) = leaf.origin else {
                    continue;
                };
                charge_execution_cfg_lookup_v29(self.retained.issuers.len(), budget)?;
                let matched = self
                    .retained
                    .issuers
                    .binary_search_by_key(&(issuer.instance.index(), issuer.block.index()), |row| {
                        (row.instance.index(), row.block.index())
                    })
                    .ok()
                    .and_then(|index| self.retained.issuers.get(index));
                budget.charge_work(std::mem::size_of::<PendingSourceIssuedIssuerV29>())?;
                if matched != Some(&issuer) {
                    return Err(source_reference_selection_memory_error_v30());
                }
            }
        }
        call_splice_sort_work_v1(operations.len(), budget)
            .map_err(source_address_call_error_v29)?;
        operations.sort_unstable_by_key(|&index| self.retained.selected[index].operation);
        for pair in operations.windows(2) {
            budget.charge_work(2)?;
            if self.retained.selected[pair[0]].operation
                == self.retained.selected[pair[1]].operation
            {
                return Err(source_reference_selection_memory_error_v30());
            }
        }
        Ok(())
    }
}

// Collapse only transparent aliases. A parameter remains an opaque inductive
// boundary with its own complete incoming roster, including the invocation.
fn source_reference_selection_terminals_v30(
    selection: &SourceReferenceSelectionActualV30,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<Vec<usize>, ProductionSemanticKirErrorV1> {
    budget.reserve_storage(argument_sum_v1(&[
        argument_product_v1(2, source_reference_emission_headers_v29::<Vec<usize>>()?)?,
        source_reference_emission_headers_v29::<Vec<bool>>()?,
    ])?)?;
    let count = selection.nodes.len();
    let mut terminal = emission_vec_v1(count, budget)?;
    let mut active = emission_vec_v1(count, budget)?;
    let mut path = emission_vec_v1(count, budget)?;
    for (index, node) in selection.nodes.iter().enumerate() {
        budget.charge_work(3)?;
        terminal.push(match node.original.step {
            SourceReferenceSelectionStepV29::Leaf(_)
            | SourceReferenceSelectionStepV29::Parameter { .. } => index,
            SourceReferenceSelectionStepV29::Alias { .. }
            | SourceReferenceSelectionStepV29::CallArgument { .. } => usize::MAX,
            SourceReferenceSelectionStepV29::Pending => {
                return Err(source_reference_selection_memory_error_v30());
            }
        });
        active.push(false);
    }
    for start in 0..count {
        budget.charge_work(1)?;
        let mut current = start;
        loop {
            budget.charge_work(4)?;
            let resolved = *terminal
                .get(current)
                .ok_or_else(source_reference_selection_memory_error_v30)?;
            if resolved != usize::MAX {
                while let Some(index) = path.pop() {
                    budget.charge_work(2)?;
                    terminal[index] = resolved;
                    active[index] = false;
                }
                break;
            }
            if active[current] {
                return Err(source_reference_selection_memory_error_v30());
            }
            active[current] = true;
            path.push(current);
            current = match selection.nodes[current].original.step {
                SourceReferenceSelectionStepV29::Alias { input, .. }
                | SourceReferenceSelectionStepV29::CallArgument { input, .. } => input,
                _ => return Err(source_reference_selection_memory_error_v30()),
            };
        }
    }
    Ok(terminal)
}

fn source_reference_selection_obligations_v30(
    selection: &SourceReferenceSelectionActualV30,
    leaves: &[PendingSourceSelectedLeafV30],
    access: BlockId,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<Vec<PendingSourceSelectedObligationV30>, ProductionSemanticKirErrorV1> {
    budget.reserve_storage(argument_sum_v1(&[
        source_reference_emission_headers_v29::<Vec<Option<usize>>>()?,
        source_reference_emission_headers_v29::<Vec<PendingSourceSelectedObligationV30>>()?,
    ])?)?;
    let terminals = source_reference_selection_terminals_v30(selection, budget)?;
    let mut leaf_index = emission_vec_v1(selection.nodes.len(), budget)?;
    leaf_index.resize(selection.nodes.len(), None);
    budget.charge_work(selection.nodes.len())?;
    for (index, leaf) in leaves.iter().enumerate() {
        budget.charge_work(4)?;
        let Some(slot) = leaf_index.get_mut(leaf.node) else {
            return Err(source_reference_selection_memory_error_v30());
        };
        if slot.replace(index).is_some()
            || !matches!(
                selection.nodes[leaf.node].original.step,
                SourceReferenceSelectionStepV29::Leaf(_)
            )
        {
            return Err(source_reference_selection_memory_error_v30());
        }
    }
    let mut rows = Vec::new();
    let mut add = |node: usize, usage, block, budget: &mut ArgumentBudgetV1<'_>| {
        let endpoint = *terminals
            .get(node)
            .ok_or_else(source_reference_selection_memory_error_v30)?;
        budget.charge_work(3)?;
        if matches!(
            selection.nodes[endpoint].original.step,
            SourceReferenceSelectionStepV29::Leaf(_)
        ) {
            let leaf =
                leaf_index[endpoint].ok_or_else(source_reference_selection_memory_error_v30)?;
            emission_push_v1(
                &mut rows,
                PendingSourceSelectedObligationV30 {
                    leaf,
                    usage,
                    block,
                    guard: None,
                    at_access: false,
                },
                budget,
            )?;
        }
        Ok::<_, ProductionSemanticKirErrorV1>(())
    };
    budget.reserve_storage(argument_sum_v1(&[
        argument_product_v1(2, std::mem::size_of_val(&add))?,
        argument_product_v1(2, std::mem::align_of_val(&add))?,
        source_reference_emission_headers_v29::<()>()?,
    ])?)?;
    add(0, PendingSourceSelectedUseV30::Access, access, budget)?;
    for (index, node) in selection.nodes.iter().enumerate() {
        budget.charge_work(3)?;
        let SourceReferenceSelectionStepV29::Parameter { first, count, .. } = node.original.step
        else {
            continue;
        };
        let end = argument_sum_v1(&[first, count])?;
        let edges = selection
            .edges
            .get(first..end)
            .ok_or_else(source_reference_selection_memory_error_v30)?;
        if let Some(invocation) = node.invocation {
            add(
                invocation.input,
                PendingSourceSelectedUseV30::Invocation(index),
                invocation.source,
                budget,
            )?;
        }
        for (ordinal, edge) in edges.iter().enumerate() {
            add(
                edge.original.input,
                PendingSourceSelectedUseV30::Incoming(first + ordinal),
                edge.source,
                budget,
            )?;
        }
    }
    Ok(rows)
}

fn check_source_reference_selection_guards_v30(
    function: &Function,
    actual: &SourceIssuedActualV29<'_>,
    receipt: &mut PendingSourceSelectedAccessV30,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    budget.reserve_storage(source_reference_emission_headers_v29::<
        Vec<(usize, BlockId, Option<usize>)>,
    >()?)?;
    let expected = source_reference_selection_obligations_v30(
        &receipt.selection,
        &receipt.leaves,
        receipt.operation.0,
        budget,
    )?;
    budget.charge_work(argument_sum_v1(&[expected.len(), 2])?)?;
    if receipt
        .selection
        .nodes
        .first()
        .is_none_or(|node| node.pointer != receipt.pointer)
        || expected.len() != receipt.obligations.len()
        || expected.iter().zip(&receipt.obligations).any(|(a, b)| {
            (a.leaf, a.usage, a.block) != (b.leaf, b.usage, b.block)
                || b.guard.is_some()
                || b.at_access
        })
    {
        return Err(source_reference_selection_memory_error_v30());
    }
    let mut queries = Vec::new();
    for (leaf, row) in receipt.leaves.iter().enumerate() {
        budget.charge_work(6)?;
        let end = argument_sum_v1(&[row.first_guard, row.guard_count])?;
        let guards = receipt
            .guards
            .get(row.first_guard..end)
            .ok_or_else(source_reference_selection_memory_error_v30)?;
        budget.charge_work(guards.len())?;
        if guards.is_empty() || guards.iter().any(|guard| guard.leaf != leaf) {
            return Err(source_reference_selection_memory_error_v30());
        }
        emission_push_v1(&mut queries, (leaf, receipt.operation.0, None), budget)?;
    }
    for row in &receipt.obligations {
        budget.charge_work(1)?;
        emission_push_v1(&mut queries, (row.leaf, row.block, None), budget)?;
    }
    call_splice_sort_work_v1(argument_product_v1(queries.len(), 2)?, budget)
        .map_err(source_address_call_error_v29)?;
    queries.sort_unstable_by_key(|row| (row.0, row.1));
    budget.charge_work(queries.len())?;
    queries.dedup_by_key(|row| (row.0, row.1));
    source_issued_pointer_walk_quote_v26(actual.values.len(), receipt.leaves.len(), budget)?;
    budget.charge_work(argument_product_v1(receipt.leaves.len(), 8)?)?;
    for (leaf, _, _) in &queries {
        budget.charge_work(argument_sum_v1(&[receipt.leaves[*leaf].guard_count, 3])?)?;
    }
    let mut valid = true;
    let check = |view: &mut fe2o3_kernel_ir::FunctionControlFlowViewV1<'_, '_, '_>| {
        for leaf in &receipt.leaves {
            let node = &receipt.selection.nodes[leaf.node];
            valid &= node.pointer == leaf.pointer
                || source_reference_pointer_transport_until_v29(
                    actual,
                    node.pointer,
                    leaf.pointer,
                    view,
                )? == Some(leaf.pointer);
        }
        for (leaf, block, selected) in &mut queries {
            let row = receipt.leaves[*leaf];
            for (ordinal, guard) in receipt.guards
                [row.first_guard..row.first_guard + row.guard_count]
                .iter()
                .enumerate()
            {
                if view.success_edge_dominates(guard.block, guard.edge, *block)? {
                    *selected = Some(row.first_guard + ordinal);
                    break;
                }
            }
        }
        Ok(())
    };
    budget.reserve_storage(argument_sum_v1(&[
        std::mem::size_of_val(&check),
        std::mem::align_of_val(&check),
        std::mem::size_of::<Result<(), fe2o3_kernel_ir::FunctionControlFlowScopeErrorV1>>(),
    ])?)?;
    fe2o3_kernel_ir::with_function_control_flow_v1(function, Default::default(), budget, check)
        .map_err(|error| match error {
            fe2o3_kernel_ir::FunctionControlFlowScopeErrorV1::Resource(error) => error.into(),
            _ => source_reference_selection_memory_error_v30(),
        })?;
    if !valid {
        return Err(source_reference_selection_memory_error_v30());
    }
    for row in &mut receipt.obligations {
        for (block, at_access) in [(receipt.operation.0, true), (row.block, false)] {
            budget.charge_work(argument_sum_v1(&[
                call_splice_search_work_v1(queries.len()),
                3,
            ])?)?;
            let index = queries
                .binary_search_by_key(&(row.leaf, block), |row| (row.0, row.1))
                .map_err(|_| source_reference_selection_memory_error_v30())?;
            if let Some(guard) = queries[index].2 {
                row.guard = Some(guard);
                row.at_access = at_access;
                break;
            }
        }
        if row.guard.is_none() {
            return Err(source_reference_selection_memory_error_v30());
        }
    }
    Ok(())
}

fn source_selected_scalar_archive_v30(
    instances: &ExecutionInstancesV29<'_>,
    index: &SourceAddressSourceIndexV29<'_>,
    actual: &SourceIssuedActualV29<'_>,
    instance: ProductionCallInstanceIdV1,
    source: SsaValueV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(ValueId, ScalarType), ProductionSemanticKirErrorV1> {
    let archive = index
        .sidecar(instance, budget)?
        .execution_observation
        .as_ref()
        .ok_or_else(source_reference_selection_memory_error_v30)?;
    let SemanticValueBindingV1::Value {
        id,
        ty: Type::Scalar(scalar),
    } = archive.lookup_original_v29(instances, instance, source, budget)?
    else {
        return Err(source_reference_selection_memory_error_v30());
    };
    budget.charge_work(1)?;
    if *actual.value(*id, budget)?.ty != Type::Scalar(*scalar) {
        return Err(source_reference_selection_memory_error_v30());
    }
    Ok((*id, *scalar))
}

fn source_selected_value_occurrence_v30(
    instances: &ExecutionInstancesV29<'_>,
    index: &SourceAddressSourceIndexV29<'_>,
    actual: &SourceIssuedActualV29<'_>,
    instance: ProductionCallInstanceIdV1,
    occurrence: usize,
    value: ValueId,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let occurrences = instances
        .occurrences(instance)
        .ok_or_else(source_descriptor_error_v29)?;
    let event = occurrences
        .events()
        .get(occurrence)
        .ok_or_else(source_descriptor_error_v29)?;
    let (source_block, statement) = scoped_memory_site_key_v29(event.site());
    let (block, first, count) = if let Some(statement) = statement {
        let span = index.statement(
            SourceReferenceSiteV29 {
                instance,
                block: SemanticBlockIdV1::from_index(source_block),
                statement: Some(statement as usize),
            },
            budget,
        )?;
        (
            span.kernel_ir_block,
            span.first_operation_ordinal,
            span.operation_count,
        )
    } else {
        budget.charge_work(argument_product_v1(
            call_splice_search_work_v1(index.terminators.len()),
            2,
        )?)?;
        let ordinal = index
            .terminators
            .binary_search_by_key(
                &(instance.index(), source_block),
                SourceAddressTerminatorV29::key,
            )
            .map_err(|_| source_descriptor_error_v29())?;
        let span = index.terminators[ordinal].span;
        (
            span.kernel_ir_block,
            span.first_operation_ordinal,
            span.operation_count,
        )
    };
    let definition = actual.value(value, budget)?;
    let (owner, position) = definition
        .location
        .ok_or_else(source_descriptor_error_v29)?;
    let position = u32::try_from(position).map_err(|_| ArgumentResourceV1::Arithmetic)?;
    let end = first
        .checked_add(count)
        .ok_or(ArgumentResourceV1::Arithmetic)?;
    let mut found = false;
    for original in first..end {
        budget.charge_work(2)?;
        if index.emitted.point(instance, block, original, budget)? == Some((owner, position)) {
            if found {
                return Err(source_descriptor_error_v29());
            }
            found = true;
        }
    }
    if !found {
        return Err(source_descriptor_error_v29());
    }
    Ok(())
}

impl SourceIssuedActualV29<'_> {
    fn selected_index_normalization_v30(
        &self,
        original: ValueId,
        scalar: ScalarType,
        offset: ValueId,
        location: (BlockId, usize),
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        check_source_index_normalization_with_v30(
            original,
            scalar,
            None,
            offset,
            self.values.len(),
            location.0,
            location.1,
            budget,
            |value, budget| {
                self.value(value, budget)
                    .map(|row| (row.ty, row.operation, row.location))
            },
        )
    }
}

impl SourceIssuedAccessesV29<'_, '_, '_> {
    fn selected_reference_access_v30(
        &mut self,
        references: &SourceReferenceEmissionV29<'_, '_>,
        instance: ProductionCallInstanceIdV1,
        anchor: usize,
        row: &ScopedMemoryAnchorV29,
        place: &SemanticPlaceV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<bool, ProductionSemanticKirErrorV1> {
        #[cfg(test)]
        SOURCE_SELECTED_ACCESS_ATTEMPTS_V30.set(SOURCE_SELECTED_ACCESS_ATTEMPTS_V30.get() + 1);
        self.check(budget)?;
        references.check(budget)?;
        references.plan.check_owner(self.instances, budget)?;
        let declaration = self
            .instances
            .instance(instance)
            .ok_or_else(source_reference_selection_memory_error_v30)?
            .declaration();
        let local = declaration
            .locals()
            .get(place.local().index() as usize)
            .ok_or_else(source_reference_selection_memory_error_v30)?;
        budget.charge_work(6)?;
        if !matches!(self.instances.owner().source_semantic().types().get(local.ty().index() as usize)
            .map(SemanticTypeDeclV1::shape), Some(SemanticTypeShapeV1::Pointer(pointer))
                if pointer.kind() == SemanticPointerKindV1::Reference
                    && pointer.metadata() == SemanticPointerMetadataV1::None
                    && pointer.pointee() == place.ty())
            || !matches!(place.projections(), [projection]
                if projection.kind() == SemanticProjectionKindV1::Dereference)
        {
            return Ok(false);
        }
        let frame = row
            .source
            .ok_or_else(source_reference_selection_memory_error_v30)?;
        let Some(ScopedMemoryRoleV29::Operand(role)) = frame.role else {
            return Err(source_reference_selection_memory_error_v30());
        };
        let Some(SourceExternalReferenceOriginV29::Selected(subject)) =
            source_external_reference_origin_from_use_v29(
                references.plan,
                instance,
                frame.site,
                role,
                place,
                Some(self.source_index),
                budget,
            )?
        else {
            return Ok(false);
        };
        budget.reserve_storage(source_reference_selection_memory_headers_v30()?)?;
        let builder = SourceReferenceSelectionBuilderV29::build(
            references.plan,
            instance,
            frame.site,
            role,
            place,
            budget,
        )?;
        if builder.graph.subject != subject {
            return Err(source_reference_selection_memory_error_v30());
        }
        let mut selection = source_reference_selection_bind_v30(
            references.plan,
            self.source_index,
            &builder.graph,
            &self.actual,
            budget,
        )?;
        source_reference_selection_check_transport_v30(
            &self.source_index.pending.function,
            &self.actual,
            &mut selection,
            budget,
        )?;
        let function = self
            .instances
            .instance(instance)
            .ok_or_else(source_reference_selection_memory_error_v30)?
            .declaration();
        self.source_index
            .frame_gap(instance, frame, row.block, row.position, budget)?;
        let operation =
            self.source_index
                .emitted
                .operation(instance, row.block, row.position, budget)?;
        let occurrences = self
            .instances
            .occurrences(instance)
            .ok_or_else(source_reference_selection_memory_error_v30)?;
        check_scoped_payload_v29(function, &occurrences, row, operation, budget)?;
        let memory = source_address_value_access_v29(operation)?
            .ok_or_else(source_reference_selection_memory_error_v30)?;
        let Type::Scalar(element) =
            lower_scalar_type(self.instances.owner().source_semantic().types(), place.ty())?
        else {
            return Err(source_reference_selection_memory_error_v30());
        };
        let root = selection
            .nodes
            .first()
            .ok_or_else(source_reference_selection_memory_error_v30)?;
        budget.charge_work(12)?;
        if memory.object
            || memory.pointer != root.pointer
            || root.element != element
            || memory.access.alignment == 0
            || u64::from(memory.access.alignment)
                > self.instances.owner().source_semantic().types()[place.ty().index() as usize]
                    .layout()
                    .alignment_bytes()
            || *self.actual.value(memory.value, budget)?.ty != Type::Scalar(element)
            || (memory.writing && root.access != AccessMode::ReadWrite)
            || !matches!(row.kind, ScopedMemoryAnchorKindV29::Access { pointer, .. } if pointer == memory.pointer)
            || !source_issued_memory_pointer_v26(
                &self.actual,
                memory.pointer,
                memory.access,
                memory.writing,
                element,
                budget,
            )?
        {
            return Err(source_reference_selection_memory_error_v30());
        }
        self.original_v26(instance, budget)?;
        charge_execution_cfg_lookup_v29(self.originals.len(), budget)?;
        check_source_issued_payload_v29(
            self.originals
                .get(&instance.index())
                .ok_or_else(source_reference_selection_memory_error_v30)?,
            row,
            operation,
            &self.actual,
            budget,
        )?;
        let at = self
            .source_index
            .emitted
            .point(
                instance,
                row.block,
                u32::try_from(row.position).map_err(|_| ArgumentResourceV1::Arithmetic)?,
                budget,
            )?
            .ok_or_else(source_reference_selection_memory_error_v30)?;
        budget.charge_work(builder.graph.nodes.len())?;
        let has_descriptors = builder.graph.nodes.iter().any(|node| {
            matches!(
                node.step,
                SourceReferenceSelectionStepV29::Leaf(
                    SourceExternalReferenceOriginV29::Descriptor { .. }
                )
            )
        });
        self.prepare_selected_guards_v30(references, has_descriptors, budget)?;
        let mut leaves = Vec::new();
        let mut guards = Vec::new();
        for (node_index, node) in selection.nodes.iter().enumerate() {
            budget.charge_work(3)?;
            let SourceReferenceSelectionStepV29::Leaf(origin) = node.original.step else {
                continue;
            };
            let leaf_index = leaves.len();
            let first_guard = guards.len();
            let (origin, pointer) = match origin {
                SourceExternalReferenceOriginV29::Issued {
                    instance: issuer_instance,
                    recipe,
                } => {
                    let physical = source_issued_archive_recipe_v29(
                        self.instances,
                        issuer_instance,
                        self.source_index,
                        recipe,
                        budget,
                    )?;
                    self.original_v26(issuer_instance, budget)?;
                    charge_execution_cfg_lookup_v29(self.originals.len(), budget)?;
                    let original = self
                        .originals
                        .get(&issuer_instance.index())
                        .ok_or_else(source_issued_error_v29)?;
                    let (transport, retained) =
                        original.actual_issuer(physical, references, &self.actual, budget)?;
                    if retained.element != element
                        || (memory.writing && retained.access != AccessMode::ReadWrite)
                    {
                        return Err(source_reference_selection_memory_error_v30());
                    }
                    emission_push_v1(&mut self.transports, transport, budget)?;
                    let key = (issuer_instance.index(), recipe.issuer);
                    charge_execution_cfg_lookup_v29(self.issuers.len(), budget)?;
                    if !self.issuers.contains(&key) {
                        reserve_execution_cfg_map_entry_v29::<(usize, SsaValueV1), ()>(
                            self.issuers.len(),
                            budget,
                        )?;
                        self.issuers.insert(key);
                        emission_push_v1(&mut self.retained.issuers, retained, budget)?;
                    }
                    let issued_guards = &self
                        .selected_guards
                        .as_ref()
                        .ok_or(ArgumentResourceV1::Accounting)?
                        .issued;
                    budget.charge_work(argument_product_v1(
                        call_splice_search_work_v1(issued_guards.len()),
                        2,
                    )?)?;
                    let first =
                        issued_guards.partition_point(|guard| guard.present < physical.present);
                    let end =
                        issued_guards.partition_point(|guard| guard.present <= physical.present);
                    for guard in &issued_guards[first..end] {
                        budget.charge_work(1)?;
                        emission_push_v1(
                            &mut guards,
                            PendingSourceSelectedGuardV30 {
                                leaf: leaf_index,
                                original_descriptor_guard: None,
                                condition: guard.present,
                                block: guard.block,
                                edge: guard.edge,
                            },
                            budget,
                        )?;
                    }
                    (
                        PendingSourceSelectedLeafOriginV30::Issued(retained),
                        physical.pointer,
                    )
                }
                SourceExternalReferenceOriginV29::Descriptor {
                    instance: producer,
                    descriptor,
                } => {
                    let retained =
                        self.selected_descriptor_v30(references, producer, descriptor, budget)?;
                    if retained.element != element
                        || (memory.writing && retained.access != AccessMode::ReadWrite)
                    {
                        return Err(source_reference_selection_memory_error_v30());
                    }
                    let source = references
                        .plan
                        .descriptors
                        .get(descriptor)
                        .ok_or_else(source_descriptor_error_v29)?;
                    let key = (producer.index(), source.index_value, retained.slice);
                    let descriptor_guards = self
                        .selected_guards
                        .as_ref()
                        .and_then(|cache| cache.descriptors.as_ref())
                        .ok_or(ArgumentResourceV1::Accounting)?;
                    budget.charge_work(argument_product_v1(
                        call_splice_search_work_v1(descriptor_guards.len()),
                        6,
                    )?)?;
                    let first = descriptor_guards.partition_point(|guard| guard.key() < key);
                    let end = descriptor_guards.partition_point(|guard| guard.key() <= key);
                    for guard in &descriptor_guards[first..end] {
                        budget.charge_work(3)?;
                        if guard.original_index != retained.original_index
                            || guard.original_scalar != retained.original_scalar
                        {
                            continue;
                        }
                        let mut selected = guard.retained;
                        selected.leaf = leaf_index;
                        emission_push_v1(&mut guards, selected, budget)?;
                    }
                    (
                        PendingSourceSelectedLeafOriginV30::Descriptor(retained),
                        retained.pointer,
                    )
                }
                SourceExternalReferenceOriginV29::Selected(_) => {
                    return Err(source_reference_selection_memory_error_v30());
                }
            };
            let guard_count = guards
                .len()
                .checked_sub(first_guard)
                .ok_or(ArgumentResourceV1::Arithmetic)?;
            call_splice_sort_work_v1(argument_product_v1(guard_count, 2)?, budget)
                .map_err(source_address_call_error_v29)?;
            guards[first_guard..].sort_unstable_by_key(|guard| {
                (guard.block, guard.edge, guard.original_descriptor_guard)
            });
            emission_push_v1(
                &mut leaves,
                PendingSourceSelectedLeafV30 {
                    node: node_index,
                    origin,
                    pointer,
                    first_guard,
                    guard_count,
                },
                budget,
            )?;
        }
        let obligations =
            source_reference_selection_obligations_v30(&selection, &leaves, at.0, budget)?;
        let mut retained = PendingSourceSelectedAccessV30 {
            instance,
            anchor,
            subject,
            selection,
            pointer: memory.pointer,
            value: memory.value,
            operation: at,
            memory: memory.access,
            writing: memory.writing,
            leaves,
            guards,
            obligations,
        };
        check_source_reference_selection_guards_v30(
            &self.source_index.pending.function,
            &self.actual,
            &mut retained,
            budget,
        )?;
        emission_push_v1(&mut self.retained.selected, retained, budget)?;
        Ok(true)
    }

    fn selected_descriptor_v30(
        &self,
        references: &SourceReferenceEmissionV29<'_, '_>,
        instance: ProductionCallInstanceIdV1,
        descriptor: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<PendingSourceSelectedDescriptorV30, ProductionSemanticKirErrorV1> {
        let row = references
            .plan
            .descriptors
            .get(descriptor)
            .ok_or_else(source_descriptor_error_v29)?;
        let source = row.check(self.instances, budget)?;
        budget.charge_work(3)?;
        if row.instance != instance || row.projection == 0 {
            return Err(source_descriptor_error_v29());
        }
        let used = references
            .descriptors
            .get(descriptor)
            .and_then(std::cell::Cell::get)
            .ok_or_else(source_descriptor_error_v29)?;
        let SourceReferenceSelectorProducerV29::Address {
            base,
            offset,
            pointer,
            block,
            operation,
        } = used.producer
        else {
            return Err(source_descriptor_error_v29());
        };
        let archive = self
            .source_index
            .sidecar(instance, budget)?
            .execution_observation
            .as_ref()
            .ok_or_else(source_descriptor_error_v29)?;
        let holder =
            archive.lookup_original_v29(self.instances, instance, row.holder_value, budget)?;
        let held = source_descriptor_binding_v29(holder, source, row.projection - 1, budget)?;
        let original_index = source_selected_scalar_archive_v30(
            self.instances,
            self.source_index,
            &self.actual,
            instance,
            row.index_value,
            budget,
        )?;
        let space = references.plan.descriptor_value_space(
            instance,
            row.holder_occurrence,
            source,
            row.projection - 1,
            row.pointer_type,
            budget,
        )?;
        let Type::Scalar(element) = lower_scalar_type(
            self.instances.owner().source_semantic().types(),
            row.element,
        )?
        else {
            return Err(source_descriptor_error_v29());
        };
        budget.charge_work(7)?;
        if held.0 != used.slice
            || original_index != (used.original, used.scalar)
            || !matches!(space, AddressSpace::Global | AddressSpace::Generic)
            || *self.actual.value(used.slice, budget)?.ty != *held.1
        {
            return Err(source_descriptor_error_v29());
        }
        check_source_descriptor_type_v29(
            self.instances.owner().source_semantic().types(),
            row.pointer_type,
            row.element,
            space,
            held.1,
            budget,
        )?;
        let Type::Slice(slice) = held.1 else {
            return Err(source_descriptor_error_v29());
        };
        let producer = self
            .source_index
            .emitted
            .operation(instance, block, operation, budget)?;
        let address = self.actual.value(pointer, budget)?;
        let data = self.actual.value(base, budget)?;
        let location = address.location.ok_or_else(source_descriptor_error_v29)?;
        let formation = self
            .source_index
            .emitted
            .point(
                instance,
                block,
                u32::try_from(operation).map_err(|_| ArgumentResourceV1::Arithmetic)?,
                budget,
            )?
            .ok_or_else(source_descriptor_error_v29)?;
        budget.charge_work(9)?;
        if !std::ptr::eq(
            address.operation.ok_or_else(source_descriptor_error_v29)?,
            producer,
        ) || !matches!((&producer.kind, producer.results.as_slice()),
                (OperationKind::GetElementPointer { base: actual_base, offset: actual_offset }, [result])
                    if *actual_base == base && *actual_offset == offset && result.id == pointer)
            || source_issued_pointer_shape_v26(address.ty) != Some((element, space, slice.access))
            || data.ty != address.ty
            || !matches!(data.operation.map(|op| &op.kind), Some(OperationKind::SliceData { slice }) if *slice == used.slice)
            || data
                .location
                .is_none_or(|(owner, ordinal)| owner != location.0 || ordinal >= location.1)
            || formation
                != (
                    location.0,
                    u32::try_from(location.1).map_err(|_| ArgumentResourceV1::Arithmetic)?,
                )
        {
            return Err(source_descriptor_error_v29());
        }
        source_selected_value_occurrence_v30(
            self.instances,
            self.source_index,
            &self.actual,
            instance,
            row.occurrence,
            pointer,
            budget,
        )?;
        self.actual.selected_index_normalization_v30(
            used.original,
            used.scalar,
            offset,
            location,
            budget,
        )?;
        Ok(PendingSourceSelectedDescriptorV30 {
            instance,
            descriptor,
            slice: used.slice,
            original_index: used.original,
            original_scalar: used.scalar,
            data: base,
            index: offset,
            pointer,
            formation,
            element,
            space,
            access: slice.access,
        })
    }

    fn selected_descriptor_guards_v30(
        &self,
        references: &SourceReferenceEmissionV29<'_, '_>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Vec<SourceSelectedDescriptorGuardV30>, ProductionSemanticKirErrorV1> {
        let controls =
            source_reference_selection_controls_v30(references.plan, self.source_index, budget)?;
        let mut guards = Vec::new();
        for (ordinal, row) in references.plan.descriptor_guards.iter().enumerate() {
            budget.charge_work(3)?;
            let Some(used) = references
                .descriptor_guards
                .get(ordinal)
                .and_then(std::cell::Cell::get)
            else {
                continue;
            };
            let source = row
                .check(self.instances, budget)?
                .ok_or_else(source_descriptor_error_v29)?;
            let archive = self
                .source_index
                .sidecar(row.instance, budget)?
                .execution_observation
                .as_ref()
                .ok_or_else(source_descriptor_error_v29)?;
            let condition = source_selected_scalar_archive_v30(
                self.instances,
                self.source_index,
                &self.actual,
                row.instance,
                source.condition,
                budget,
            )?;
            let index = source_selected_scalar_archive_v30(
                self.instances,
                self.source_index,
                &self.actual,
                row.instance,
                source.index,
                budget,
            )?;
            let length = source_selected_scalar_archive_v30(
                self.instances,
                self.source_index,
                &self.actual,
                row.instance,
                source.length,
                budget,
            )?;
            let holder =
                archive.lookup_original_v29(self.instances, row.instance, source.holder, budget)?;
            let held = source_descriptor_binding_v29(holder, source.source, source.fields, budget)?;
            budget.charge_work(8)?;
            if condition != (used.condition, ScalarType::Bool)
                || index != (used.index, used.index_scalar)
                || length != (used.length, used.length_scalar)
                || held.0 != used.slice
                || !matches!(used.index_scalar, ScalarType::U64 | ScalarType::Index)
                || !matches!(used.length_scalar, ScalarType::U64 | ScalarType::Index)
                || self.actual.value(used.slice, budget)?.ty != held.1
            {
                return Err(source_descriptor_error_v29());
            }
            let space = references.plan.descriptor_value_space(
                row.instance,
                row.holder,
                source.source,
                source.fields,
                source.pointer_type,
                budget,
            )?;
            check_source_descriptor_type_v29(
                self.instances.owner().source_semantic().types(),
                source.pointer_type,
                source.element,
                space,
                held.1,
                budget,
            )?;
            let comparison = self.actual.value(used.condition, budget)?;
            let location = comparison
                .location
                .ok_or_else(source_descriptor_error_v29)?;
            let Some(OperationKind::Compare {
                predicate: ComparePredicate::LessThan,
                lhs,
                rhs,
            }) = comparison.operation.map(|op| &op.kind)
            else {
                return Err(source_descriptor_error_v29());
            };
            source_selected_value_occurrence_v30(
                self.instances,
                self.source_index,
                &self.actual,
                row.instance,
                row.comparison,
                used.condition,
                budget,
            )?;
            if used.index_scalar == ScalarType::U64 && used.length_scalar == ScalarType::U64 {
                if *lhs != used.index || *rhs != used.length {
                    return Err(source_descriptor_error_v29());
                }
            } else {
                self.actual.selected_index_normalization_v30(
                    used.index,
                    used.index_scalar,
                    *lhs,
                    location,
                    budget,
                )?;
                self.actual.selected_index_normalization_v30(
                    used.length,
                    used.length_scalar,
                    *rhs,
                    location,
                    budget,
                )?;
            }
            let mut metadata = self.actual.value(used.length, budget)?;
            if used.length_scalar == ScalarType::U64 {
                let Some(OperationKind::Cast {
                    kind: CastKind::Bitcast,
                    value,
                    to: Type::Scalar(ScalarType::U64),
                }) = metadata.operation.map(|op| &op.kind)
                else {
                    return Err(source_descriptor_error_v29());
                };
                source_selected_value_occurrence_v30(
                    self.instances,
                    self.source_index,
                    &self.actual,
                    row.instance,
                    row.metadata,
                    metadata.id,
                    budget,
                )?;
                metadata = self.actual.value(*value, budget)?;
            }
            if *metadata.ty != Type::INDEX
                || !matches!(metadata.operation.map(|op| &op.kind), Some(OperationKind::SliceLength { slice }) if *slice == used.slice)
            {
                return Err(source_descriptor_error_v29());
            }
            source_selected_value_occurrence_v30(
                self.instances,
                self.source_index,
                &self.actual,
                row.instance,
                row.metadata,
                metadata.id,
                budget,
            )?;
            let guard = source_reference_selection_control_v30(
                &controls,
                row.instance,
                SemanticBlockIdV1::from_index(row.assertion),
                budget,
            )?;
            let success = source_reference_selection_control_v30(
                &controls,
                row.instance,
                source.success,
                budget,
            )?;
            charge_execution_cfg_lookup_v29(self.source_index.emitted.blocks.len(), budget)?;
            let at = self
                .source_index
                .emitted
                .blocks
                .binary_search_by_key(&guard.terminal.0, |row| row.0)
                .map_err(|_| source_descriptor_error_v29())?;
            let block = self.source_index.emitted.blocks[at].1;
            if used.block != guard.entry
                || used.success != success.entry
                || !matches!(block.terminator.as_ref(), Some(Terminator::ConditionalBranch { condition, then_target, else_target, .. })
                if *condition == used.condition && *then_target == success.entry
                    && *else_target == used.failure && then_target != else_target)
            {
                return Err(source_descriptor_error_v29());
            }
            emission_push_v1(
                &mut guards,
                SourceSelectedDescriptorGuardV30 {
                    instance: row.instance,
                    index: source.index,
                    slice: used.slice,
                    original_index: used.index,
                    original_scalar: used.index_scalar,
                    retained: PendingSourceSelectedGuardV30 {
                        leaf: usize::MAX,
                        original_descriptor_guard: Some(ordinal),
                        condition: used.condition,
                        block: guard.terminal,
                        edge: 0,
                    },
                },
                budget,
            )?;
        }
        call_splice_sort_work_v1(argument_product_v1(guards.len(), 3)?, budget)
            .map_err(source_address_call_error_v29)?;
        guards.sort_unstable_by_key(|row| (row.key(), row.retained.block, row.retained.edge));
        Ok(guards)
    }
}
