/// One original root-qualified ordinary call and its exact source ABI association.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductionCanonicalPrivateCallSiteV1 {
    operation: usize,
    root: SemanticFunctionIdV1,
    source_call: usize,
    caller: usize,
    callee: usize,
}
impl ProductionCanonicalPrivateCallSiteV1 {
    /// Physical occurrence ordinal in the private-operation roster.
    pub const fn operation(&self) -> usize {
        self.operation
    }
    /// Original source root, retained across shared helper aliases.
    pub const fn root(&self) -> SemanticFunctionIdV1 {
        self.root
    }
    /// Original inventory call ordinal, not an ordinal in F.
    pub const fn original_call(&self) -> usize {
        self.source_call
    }
    /// Original caller association with its source/canonical binding.
    pub const fn caller_association(&self) -> usize {
        self.caller
    }
    /// Original callee association and retained local-frame/argument obligations.
    pub const fn callee_association(&self) -> usize {
        self.callee
    }
}

// These indexes contain only ordinals into the exact borrowed source. Numeric
// keys select candidates; source pointers, coordinates and full rows still join.
struct CpcCallIndexV1 {
    associations: Vec<(u64, usize)>,
    operations: Vec<Option<usize>>,
    aliases: Vec<(usize, usize, usize)>,
    ranges: Vec<std::ops::Range<usize>>,
}
fn cpc_key_v1(source: &SemanticKirFunctionCorrespondenceV1) -> u64 {
    call_source_key_v1(source.correspondence_owner, source.semantic_function)
}
fn cpc_index_error_v1(
    error: SemanticKirAssertOriginErrorV1,
) -> ProductionCanonicalScalarSourceErrorV1 {
    match error {
        SemanticKirAssertOriginErrorV1::Resource(error) => error.into(),
        _ => cs_invalid_v1("private call index"),
    }
}
fn cpc_sort_v1<T, K: Ord>(
    rows: &mut [T],
    key: impl Fn(&T) -> K,
    budget: &mut ArgumentBudgetV1<'_>,
) -> CsResultV1<()> {
    assert_origin_sort_v1(rows, budget, |a, b, budget| {
        budget.charge_work(1)?;
        Ok(key(a).cmp(&key(b)))
    })
    .map_err(cpc_index_error_v1)
}
fn cpc_find_v1<T, K: Ord>(
    rows: &[T],
    wanted: K,
    key: impl Fn(&T) -> K,
    budget: &mut ArgumentBudgetV1<'_>,
) -> CsResultV1<Option<usize>> {
    assert_origin_find_v1(rows, budget, |row, budget| {
        budget.charge_work(1)?;
        Ok(key(row).cmp(&wanted))
    })
    .map_err(cpc_index_error_v1)
}
impl CpcCallIndexV1 {
    fn new(
        source: &ProductionCanonicalRankedMetadataV1<'_>,
        physical: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> CsResultV1<Self> {
        // The enclosing CpcOrigins header pays these four Vec headers.
        let mut associations = cs_vec_v1(source.calls.groups.len(), budget)?;
        for (ordinal, group) in source.calls.groups.iter().enumerate() {
            budget.charge_work(1)?;
            cs_push_v1(
                &mut associations,
                (cpc_key_v1(group.function.source()), ordinal),
                budget,
            )?;
        }
        cpc_sort_v1(&mut associations, |row| row.0, budget)?;
        let mut operations = cs_vec_v1(source.inventory.operations().len(), budget)?;
        budget.charge_work(source.inventory.operations().len())?;
        operations.resize(source.inventory.operations().len(), None);
        let aliases = cs_vec_v1(source.calls.calls.len(), budget)?;
        let mut ranges = cs_vec_v1(physical, budget)?;
        budget.charge_work(physical)?;
        ranges.resize(physical, 0..0);
        let index = Self {
            associations,
            operations,
            aliases,
            ranges,
        };
        index.check_associations(source, budget)?;
        Ok(index)
    }
    fn check_associations(
        &self,
        source: &ProductionCanonicalRankedMetadataV1<'_>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> CsResultV1<()> {
        budget.charge_work(1)?;
        if self.associations.len() != source.calls.groups.len() {
            return Err(cs_invalid_v1("complete original caller index"));
        }
        let mut previous = None;
        for &(key, ordinal) in &self.associations {
            budget.charge_work(3)?;
            let group = source
                .calls
                .groups
                .get(ordinal)
                .ok_or_else(|| cs_invalid_v1("original caller index ordinal"))?;
            if key != cpc_key_v1(group.function.source()) || previous.is_some_and(|p| p >= key) {
                return Err(cs_invalid_v1("original caller index identity"));
            }
            previous = Some(key);
        }
        Ok(())
    }
    fn caller(
        &self,
        source: &ProductionCanonicalRankedMetadataV1<'_>,
        caller: &SemanticKirFunctionCorrespondenceV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> CsResultV1<usize> {
        budget.charge_work(1)?;
        let found = cpc_find_v1(&self.associations, cpc_key_v1(caller), |row| row.0, budget)?
            .ok_or_else(|| cs_invalid_v1("original caller association"))?;
        budget.charge_work(1)?;
        let ordinal = self.associations[found].1;
        if source
            .calls
            .groups
            .get(ordinal)
            .is_none_or(|g| !std::ptr::eq(g.function.source(), caller))
        {
            return Err(cs_invalid_v1("original caller association"));
        }
        Ok(ordinal)
    }
    fn finish(
        &mut self,
        calls: &[ProductionCanonicalPrivateCallSiteV1],
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> CsResultV1<()> {
        for (ordinal, call) in calls.iter().enumerate() {
            cs_push_v1(
                &mut self.aliases,
                (call.operation, call.caller, ordinal),
                budget,
            )?;
        }
        cpc_sort_v1(&mut self.aliases, |row| (row.0, row.1), budget)?;
        let mut next = 0;
        for (operation, range) in self.ranges.iter_mut().enumerate() {
            budget.charge_work(1)?;
            let start = next;
            while next < self.aliases.len() {
                budget.charge_work(1)?;
                if self.aliases[next].0 != operation {
                    break;
                }
                next = argument_sum_v1(&[next, 1])?;
            }
            *range = start..next;
        }
        self.check_aliases(calls, self.ranges.len(), budget)
    }
    fn check_aliases(
        &self,
        calls: &[ProductionCanonicalPrivateCallSiteV1],
        physical: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> CsResultV1<()> {
        budget.charge_work(1)?;
        if self.aliases.len() != calls.len() || self.ranges.len() != physical {
            return Err(cs_invalid_v1("complete original call inverse"));
        }
        let mut previous = None;
        for &(operation, caller, ordinal) in &self.aliases {
            budget.charge_work(3)?;
            let call = calls
                .get(ordinal)
                .ok_or_else(|| cs_invalid_v1("original call inverse ordinal"))?;
            let key = (operation, caller);
            if operation >= physical
                || key != (call.operation, call.caller)
                || previous.is_some_and(|p| p >= key)
            {
                return Err(cs_invalid_v1("original call inverse identity"));
            }
            previous = Some(key);
        }
        let mut next = 0;
        for (operation, range) in self.ranges.iter().enumerate() {
            budget.charge_work(1)?;
            if range.start != next || range.end < next || range.end > self.aliases.len() {
                return Err(cs_invalid_v1("original call alias range"));
            }
            for row in &self.aliases[range.clone()] {
                budget.charge_work(1)?;
                if row.0 != operation {
                    return Err(cs_invalid_v1("original call alias range"));
                }
            }
            next = range.end;
        }
        if next != self.aliases.len() {
            return Err(cs_invalid_v1("complete original call alias ranges"));
        }
        Ok(())
    }
}

fn cpc_call_row_v1(
    origins: &CpcOriginsV1<'_, '_>,
    binding: &CanonicalCallBindingV1<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> CsResultV1<ProductionCanonicalPrivateCallSiteV1> {
    let source = origins.source;
    budget.charge_work(3)?;
    let actual = source
        .inventory
        .calls()
        .get(binding.call)
        .ok_or_else(|| cs_invalid_v1("original call ordinal"))?;
    let callee = source
        .calls
        .groups
        .get(binding.callee)
        .ok_or_else(|| cs_invalid_v1("original callee association"))?;
    let caller = origins.index.caller(source, binding.site.caller, budget)?;
    if actual.target != Some(callee.function.canonical.coordinate)
        || actual.coordinate.block.function
            != source.calls.groups[caller].function.canonical.coordinate
        || callee.function.source().correspondence_owner != binding.site.caller.correspondence_owner
    {
        return Err(cs_invalid_v1(
            "root-qualified original callee/frame identity",
        ));
    }
    let ordinal = cs_operation_v1(source.inventory, actual.coordinate, budget)?;
    budget.charge_work(2)?;
    let operation = origins
        .index
        .operations
        .get(ordinal)
        .copied()
        .flatten()
        .ok_or_else(|| cs_invalid_v1("complete ordinary call operation roster"))?;
    if origins.operations.get(operation).is_none_or(|row| {
        row.coordinate != actual.coordinate
            || row.kind != ProductionCanonicalPrivateOperationKindV1::Call
    }) {
        return Err(cs_invalid_v1("ordinary call operation kind"));
    }
    Ok(ProductionCanonicalPrivateCallSiteV1 {
        operation,
        root: binding.site.caller.correspondence_owner,
        source_call: binding.call,
        caller,
        callee: binding.callee,
    })
}
fn cpc_original_calls_v1(
    origins: &mut CpcOriginsV1<'_, '_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> CsResultV1<()> {
    for binding in &origins.source.calls.calls {
        let row = cpc_call_row_v1(origins, binding, budget)?;
        cs_push_v1(&mut origins.calls, row, budget)?;
    }
    origins.index.finish(&origins.calls, budget)
}
fn cpc_check_original_calls_v1(
    origins: &CpcOriginsV1<'_, '_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> CsResultV1<()> {
    budget.charge_work(1)?;
    if origins.calls.len() != origins.source.calls.calls.len() {
        return Err(cs_invalid_v1(
            "complete original root-qualified call roster",
        ));
    }
    origins
        .index
        .check_aliases(&origins.calls, origins.operations.len(), budget)?;
    for (row, binding) in origins.calls.iter().zip(&origins.source.calls.calls) {
        let expected = cpc_call_row_v1(origins, binding, budget)?;
        if *row != expected {
            return Err(cs_invalid_v1("original call alias/callee custody"));
        }
    }
    Ok(())
}

struct CpcFinalCallIndexV1 {
    functions: Vec<Option<usize>>,
    calls: Vec<Option<usize>>,
}
impl CpcFinalCallIndexV1 {
    fn new(
        source: &CanonicalKirInventoryV1<'_>,
        output: &CanonicalKirInventoryV1<'_>,
        lineage: &CsLineageV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> CsResultV1<Self> {
        budget.charge_work(1)?;
        if lineage.functions.len() != output.functions().len() {
            return Err(cs_invalid_v1("final callable function roster"));
        }
        budget.reserve_storage(std::mem::size_of::<Self>())?;
        let mut functions = cs_vec_v1(source.functions().len(), budget)?;
        budget.charge_work(source.functions().len())?;
        functions.resize(source.functions().len(), None);
        for (ordinal, original) in lineage.functions.iter().enumerate() {
            let source_ordinal = cs_function_v1(source, *original, budget)?;
            let final_ordinal =
                cs_function_v1(output, output.functions()[ordinal].coordinate, budget)?;
            budget.charge_work(1)?;
            if final_ordinal != ordinal || functions[source_ordinal].replace(ordinal).is_some() {
                return Err(cs_invalid_v1("final callable function lineage"));
            }
        }
        let mut calls = cs_vec_v1(output.operations().len(), budget)?;
        budget.charge_work(output.operations().len())?;
        calls.resize(output.operations().len(), None);
        for (ordinal, call) in output.calls().iter().enumerate() {
            let operation = cs_operation_v1(output, call.coordinate, budget)?;
            budget.charge_work(1)?;
            if calls[operation].replace(ordinal).is_some() {
                return Err(cs_invalid_v1("retained final call occurrence"));
            }
        }
        Ok(Self { functions, calls })
    }
}

#[allow(clippy::too_many_arguments)]
fn cpc_final_calls_v1(
    origins: &CpcOriginsV1<'_, '_>,
    transport: &CpcTransportV1<'_, '_, '_>,
    output: &CanonicalKirInventoryV1<'_>,
    lineage: &CsLineageV1,
    coverage: &Coverage<'_>,
    callables: &[ProductionCanonicalAssertionCallableV1],
    budget: &mut ArgumentBudgetV1<'_>,
) -> CsResultV1<()> {
    use fe2o3_kernel_analysis::{
        CanonicalKirCallEffectDecisionV1 as Effect, CanonicalKirCallEffectsV1,
    };
    let source = origins.source;
    origins.check(source, budget)?;
    let (effects, receipt) =
        CanonicalKirCallEffectsV1::derive(output, budget).map_err(Failure::CallEffects)?;
    budget.reserve_storage(receipt.retained_storage())?;
    budget.charge_work(1)?;
    if callables.len() != source.calls.groups.len() {
        return Err(cs_invalid_v1("complete source callable classifications"));
    }
    let index = CpcFinalCallIndexV1::new(source.inventory, output, lineage, budget)?;
    for callable in callables {
        let original = cs_function_v1(source.inventory, callable.function, budget)?;
        budget.charge_work(1)?;
        let final_index = index
            .functions
            .get(original)
            .copied()
            .flatten()
            .ok_or_else(|| cs_invalid_v1("final callable function lineage"))?;
        let function = output
            .functions()
            .get(final_index)
            .ok_or_else(|| cs_invalid_v1("final callable function roster"))?;
        let decision = effects
            .decision(function.coordinate, budget)
            .map_err(Failure::CallEffects)?;
        budget.charge_work(2)?;
        if decision == Effect::Incomplete
            || (matches!(
                callable.kind,
                ProductionCanonicalAssertionCallKindV1::EmptyOnly
                    | ProductionCanonicalAssertionCallKindV1::DeterministicEmpty
            ) && decision != Effect::CompleteEmpty)
        {
            return Err(cs_invalid_v1("fresh final callable effect closure"));
        }
        // PrivateFrame keeps its original source decision even if checked dead
        // control removes all physical effects; it never acquires an empty grant.
    }
    for (operation_index, state) in transport.operations.iter().enumerate() {
        budget.charge_work(2)?;
        if state.kind != ProductionCanonicalPrivateOperationKindV1::Call {
            continue;
        }
        let Some(coordinate) = state.current else {
            continue;
        };
        let ordinal = cs_operation_v1(output, coordinate, budget)?;
        budget.charge_work(1)?;
        let call = index
            .calls
            .get(ordinal)
            .copied()
            .flatten()
            .and_then(|n| output.calls().get(n))
            .ok_or_else(|| cs_invalid_v1("retained final call occurrence"))?;
        if call.coordinate != coordinate {
            return Err(cs_invalid_v1("retained final call occurrence"));
        }
        let mut aliases = 0usize;
        for &(_, _, ordinal) in
            &origins.index.aliases[origins.index.ranges[operation_index].clone()]
        {
            budget.charge_work(1)?;
            let alias = &origins.calls[ordinal];
            aliases = argument_sum_v1(&[aliases, 1])?;
            let caller = &source.calls.groups[alias.caller].function;
            let callee = &source.calls.groups[alias.callee].function;
            let actual_caller = cs_function_v1(output, coordinate.block.function, budget)?;
            let actual_callee = call
                .target
                .ok_or_else(|| cs_invalid_v1("ordinary final call target"))?;
            let actual_callee = cs_function_v1(output, actual_callee, budget)?;
            budget.charge_work(4)?;
            if lineage.functions[actual_caller] != caller.canonical.coordinate
                || lineage.functions[actual_callee] != callee.canonical.coordinate
                || alias.root != caller.source().correspondence_owner
                || alias.root != callee.source().correspondence_owner
            {
                return Err(cs_invalid_v1("final root/callee/function lineage"));
            }
            let original = &source.inventory.calls()[alias.source_call];
            budget.charge_work(argument_sum_v1(&[
                original.callee.len(),
                call.callee.len(),
            ])?)?;
            if original.callee != call.callee {
                return Err(cs_invalid_v1("final exact callee symbol"));
            }
        }
        let row = &origins.operations[operation_index];
        if aliases == 0 {
            for alias in &origins.aliases[row.aliases.clone()] {
                budget.charge_work(2)?;
                if !coverage.synthetic.get(alias.span).copied().unwrap_or(false) {
                    return Err(cs_invalid_v1(
                        "call without ordinary or exact trap source alias",
                    ));
                }
                coverage.span(source, alias.span, budget)?;
            }
        } else {
            if aliases != row.aliases.len() {
                return Err(cs_invalid_v1(
                    "complete final ordinary-call alias multiplicity",
                ));
            }
            for source_alias in &origins.aliases[row.aliases.clone()] {
                // check_aliases already established strict unique keys. The
                // exact association must still be present for every source row.
                if cpc_find_v1(
                    &origins.index.aliases[origins.index.ranges[operation_index].clone()],
                    source_alias.association,
                    |r| r.1,
                    budget,
                )?
                .is_none()
                {
                    return Err(cs_invalid_v1("final call inverse source alias"));
                }
            }
        }
    }
    // The complete operation tracker has already joined every actual output Call
    // to exactly one retained original physical occurrence, including trap calls.
    Ok(())
}

#[cfg(test)]
mod private_call_index_resources_v1_tests {
    use super::*;
    include!("production_canonical_private_call_index_resources_v1_tests.rs");
}

#[cfg(test)]
pub(super) use private_call_index_resources_v1_tests::read_test_private_index_v1;
