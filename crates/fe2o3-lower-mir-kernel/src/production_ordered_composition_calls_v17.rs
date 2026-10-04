// Exact V17 sibling. No V12 inventory, owner projection, graph clone or raw
// module admission: every function/call comes from the retained composition.
struct OrderedCanonicalCallGroupV17<'a> {
    source: &'a SemanticKirFunctionCorrespondenceV1,
    canonical: &'a Function,
    calls: &'a [SemanticKirCallReturnV1],
    spans: &'a [SemanticKirTerminatorOperationSpanV1],
    direct: &'a [SemanticKirParameterBindingV1],
    components: &'a [SemanticKirParameterComponentBindingV1],
    ignored: &'a [SemanticKirIgnoredParameterBindingV1],
}
impl OrderedCanonicalCallGroupV17<'_> {
    fn parameters(&self) -> ArgumentTraceV1<'_> {
        ArgumentTraceV1 {
            direct: self.direct,
            components: self.components,
            ignored: self.ignored,
        }
    }
}
struct OrderedCanonicalCallBindingV17<'a> {
    key: fe2o3_kernel_ir::OrderedProgramCallKeyV1,
    callee: usize,
    site: CheckedCallSiteV1<'a>,
}

/// Scoped exact source/canonical call and ABI views for a V17 composition.
/// Keys are owner-local locators, not source, launch or executable authority.
pub struct ProductionCanonicalCallsV17<'a> {
    owner: &'a ProductionOrderedCompositionPreRankedKirOwnerV1,
    groups: Vec<OrderedCanonicalCallGroupV17<'a>>,
    calls: Vec<OrderedCanonicalCallBindingV17<'a>>,
    ledger: usize,
    work_ledger: ArgumentLedgerV1,
    floor: usize,
}
impl ProductionCanonicalCallsV17<'_> {
    /// Exact borrowed pre-ranked owner identity; equal bytes are insufficient.
    pub fn belongs_to(&self, owner: &ProductionOrderedCompositionPreRankedKirOwnerV1) -> bool {
        std::ptr::eq(self.owner, owner)
    }
    /// Number of actual canonical Defined calls, not expanded region count.
    pub fn call_count(&self) -> usize {
        self.calls.len()
    }
    /// Canonical-call locators in the retained structural owner's order.
    pub fn sites(&self) -> impl Iterator<Item = fe2o3_kernel_ir::OrderedProgramCallKeyV1> + '_ {
        self.calls.iter().map(|row| row.key)
    }
    /// Borrows the existing checked ABI/result transport view. The original
    /// ledger and all retained receipts must stay live during the callback.
    pub fn with_call<'w, R>(
        &self,
        key: fe2o3_kernel_ir::OrderedProgramCallKeyV1,
        budget: &mut ArgumentBudgetV1<'w>,
        consumer: impl for<'s> FnOnce(
            &mut ProductionCallViewV1<'s, 'w>,
        ) -> Result<R, ProductionSemanticKirErrorV1>,
    ) -> Result<R, ProductionSemanticKirErrorV1> {
        budget.charge_work(4)?;
        if self.ledger != budget as *const ArgumentBudgetV1<'_> as usize
            || self.work_ledger != budget.work_ledger_identity_v1()
            || budget.storage() < self.floor
        {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        with_canonical_call_scratch_v1(budget, |budget| {
            budget.charge_work(self.calls.len())?;
            let binding = self
                .calls
                .iter()
                .find(|row| row.key == key)
                .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
            with_checked_call_site_v1(
                self.owner.semantic_ssa(),
                &self.owner.correspondence.call_result_components,
                binding.site,
                self.groups[binding.callee].parameters(),
                budget,
                consumer,
            )
        })
    }
}
impl ProductionOrderedCompositionPreRankedKirOwnerV1 {
    /// Reuses exact call correspondence and argument/result assembly for the
    /// retained V17 graph. A callback cannot escape a borrowed view or replace
    /// the source owner with an independently decoded canonical subject.
    pub fn with_checked_canonical_calls_v17<'w, R>(
        &self,
        budget: &mut ArgumentBudgetV1<'w>,
        consumer: impl for<'s> FnOnce(
            &ProductionCanonicalCallsV17<'s>,
            &mut ArgumentBudgetV1<'w>,
        ) -> Result<R, ProductionSemanticKirErrorV1>,
    ) -> Result<R, ProductionSemanticKirErrorV1> {
        if budget.storage() < self.live_storage_floor_v1()? {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        with_canonical_call_scratch_v1(budget, |budget| {
            let calls = build_ordered_canonical_calls_v17(self, budget)?;
            let floor = budget.storage();
            let result = consumer(&calls, budget);
            if budget.work_ledger_identity_v1() != calls.work_ledger || budget.storage() != floor {
                return Err(ArgumentResourceV1::Accounting.into());
            }
            result
        })
    }
}

fn build_ordered_canonical_calls_v17<'a>(
    owner: &'a ProductionOrderedCompositionPreRankedKirOwnerV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<ProductionCanonicalCallsV17<'a>, ProductionSemanticKirErrorV1> {
    let rows = &owner.correspondence;
    let composition = owner.composition();
    let module = composition.canonical().module();
    let mismatch = || ProductionSemanticKirErrorV1::CorrespondenceMismatch;
    budget.charge_work(argument_sum_v1(&[
        argument_product_v1(rows.lowered_functions.len(), 6)?,
        rows.call_returns.len(),
        rows.terminator_operation_spans.len(),
        rows.parameter_bindings.len(),
        rows.parameter_component_bindings.len(),
        rows.ignored_parameter_bindings.len(),
    ])?)?;
    if rows.lowered_functions.len() != composition.helpers().len() + 1
        || rows.lowered_functions.len() > 3
        || composition.calls().len() > 8
    {
        return Err(mismatch());
    }
    budget.reserve_storage(argument_sum_v1(&[
        std::mem::size_of::<ProductionCanonicalCallsV17<'_>>(),
        argument_product_v1(
            rows.lowered_functions.len(),
            std::mem::size_of::<OrderedCanonicalCallGroupV17<'_>>(),
        )?,
        argument_product_v1(
            composition.calls().len(),
            std::mem::size_of::<OrderedCanonicalCallBindingV17<'_>>(),
        )?,
    ])?)?;
    let mut groups = argument_vec_v1(rows.lowered_functions.len())?;
    let mut calls = argument_vec_v1(composition.calls().len())?;
    // These arrays never grow after this exact reservation. Charge any reported
    // allocator excess before filling; the outer scope drops them before release.
    budget.reserve_storage(argument_sum_v1(&[
        argument_product_v1(
            groups
                .capacity()
                .checked_sub(rows.lowered_functions.len())
                .ok_or(ArgumentResourceV1::Accounting)?,
            std::mem::size_of::<OrderedCanonicalCallGroupV17<'_>>(),
        )?,
        argument_product_v1(
            calls
                .capacity()
                .checked_sub(composition.calls().len())
                .ok_or(ArgumentResourceV1::Accounting)?,
            std::mem::size_of::<OrderedCanonicalCallBindingV17<'_>>(),
        )?,
    ])?)?;
    let index_floor = budget.storage();
    let targets = CallTargetIndexV1::new(module, &rows.lowered_functions, budget)?;
    let index_storage = budget
        .storage()
        .checked_sub(index_floor)
        .ok_or(ArgumentResourceV1::Accounting)?;
    let mut remaining_calls = rows.call_returns.as_ref();
    let mut remaining_spans = rows.terminator_operation_spans.as_ref();
    let mut remaining_direct = rows.parameter_bindings.as_ref();
    let mut remaining_components = rows.parameter_component_bindings.as_ref();
    let mut remaining_ignored = rows.ignored_parameter_bindings.as_ref();
    macro_rules! group {
        ($remaining:ident, $source:ident) => {{
            let count = $remaining
                .iter()
                .take_while(|row| {
                    row.correspondence_owner == $source.correspondence_owner
                        && row.semantic_function == $source.semantic_function
                })
                .count();
            let (selected, rest) = $remaining.split_at(count);
            $remaining = rest;
            selected
        }};
    }
    for source in &rows.lowered_functions {
        let (_, canonical) = targets.source(
            source.correspondence_owner,
            source.semantic_function,
            budget,
        )?;
        let group = OrderedCanonicalCallGroupV17 {
            source,
            canonical,
            calls: group!(remaining_calls, source),
            spans: group!(remaining_spans, source),
            direct: group!(remaining_direct, source),
            components: group!(remaining_components, source),
            ignored: group!(remaining_ignored, source),
        };
        if group.spans.len() != canonical.body.as_ref().ok_or_else(mismatch)?.blocks.len() {
            return Err(mismatch());
        }
        validate_call_correspondence_v1(
            owner.semantic_ssa(),
            source,
            canonical,
            &targets,
            group.calls,
            &rows.call_result_components,
            group.spans,
            budget,
        )?;
        groups.push(group);
    }
    if !remaining_calls.is_empty()
        || !remaining_spans.is_empty()
        || !remaining_direct.is_empty()
        || !remaining_components.is_empty()
        || !remaining_ignored.is_empty()
    {
        return Err(mismatch());
    }
    for row in composition.calls() {
        budget.charge_work(groups.len() + 4)?;
        let site = row.site();
        let caller_function = module
            .functions
            .get(site.function_ordinal() as usize)
            .ok_or_else(mismatch)?;
        let group = groups
            .iter()
            .find(|g| std::ptr::eq(g.canonical, caller_function))
            .ok_or_else(mismatch)?;
        let block = caller_function
            .body
            .as_ref()
            .and_then(|b| b.blocks.get(site.block_ordinal() as usize))
            .filter(|b| b.id == site.block())
            .ok_or_else(mismatch)?;
        let operation = composition
            .call_operation(composition.canonical().identity(), row.key(), budget)
            .map_err(ordered_composition_structural_error_v1)?;
        let OperationKind::Call { callee: target, .. } = &operation.kind else {
            return Err(mismatch());
        };
        let span = group
            .spans
            .get(site.block_ordinal() as usize)
            .filter(|s| s.kernel_ir_block == block.id)
            .ok_or_else(mismatch)?;
        budget.charge_work(group.calls.len() + 2)?;
        let anchor = group
            .calls
            .iter()
            .find(|r| r.semantic_block == span.semantic_block)
            .ok_or_else(mismatch)?;
        if !matches!(anchor.kind, SemanticKirCallReturnKindV1::Call { call_operation, .. }
            if call_operation == site.operation_ordinal())
        {
            return Err(mismatch());
        }
        let source_function = owner
            .semantic_ssa()
            .source_semantic()
            .functions()
            .get(group.source.semantic_function.index() as usize)
            .ok_or_else(mismatch)?;
        let source_block = source_function
            .blocks()
            .get(span.semantic_block.index() as usize)
            .ok_or_else(mismatch)?;
        let SemanticTerminatorKindV1::Call(source) = source_block.terminator().kind() else {
            return Err(mismatch());
        };
        let Some(SemanticCallableDeclV1::Defined { function: callee }) = owner
            .semantic_ssa()
            .source_semantic()
            .callables()
            .get(source.callee().index() as usize)
        else {
            return Err(mismatch());
        };
        let (callee, association) =
            targets.source_row(group.source.correspondence_owner, *callee, budget)?;
        let callee_group = groups.get(callee).ok_or_else(mismatch)?;
        let structural_helper = composition
            .helper_function(composition.canonical().identity(), row.callee(), budget)
            .map_err(ordered_composition_structural_error_v1)?;
        if !std::ptr::eq(association, callee_group.source)
            || !std::ptr::eq(structural_helper, callee_group.canonical)
            || *target != callee_group.canonical.id
        {
            return Err(mismatch());
        }
        calls.push(OrderedCanonicalCallBindingV17 {
            key: row.key(),
            callee,
            site: CheckedCallSiteV1 {
                caller: group.source,
                source,
                callee: association,
                callee_target: callee_group.canonical,
                block,
                span,
                anchor,
                returns: callee_group.calls,
            },
        });
    }
    budget.charge_work(rows.call_returns.len())?;
    if rows
        .call_returns
        .iter()
        .filter(|r| matches!(r.kind, SemanticKirCallReturnKindV1::Call { .. }))
        .count()
        != calls.len()
    {
        return Err(mismatch());
    }
    drop(targets);
    budget.release_storage(index_storage)?;
    Ok(ProductionCanonicalCallsV17 {
        owner,
        groups,
        calls,
        ledger: budget as *const ArgumentBudgetV1<'_> as usize,
        work_ledger: budget.work_ledger_identity_v1(),
        floor: budget.storage(),
    })
}
