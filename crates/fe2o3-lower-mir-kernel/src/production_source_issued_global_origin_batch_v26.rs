// Root-local inert classification cache. Its caller owns construction storage;
// the graph index and CFG scratch settle before any map is returned. No query
// can rebuild a missing entry or apply rows to an equal-but-distinct function.
struct SourceIssuedGlobalOriginsV26<'function> {
    function: &'function Function,
    rows: Vec<(ValueId, Option<ValueId>)>,
    slot: usize,
    ledger: fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
    floor: usize,
    owned: usize,
}

impl<'function> SourceIssuedGlobalOriginsV26<'function> {
    fn headers() -> usize {
        std::mem::size_of::<(
            Self,
            Result<Self, ProductionSemanticKirErrorV1>,
            [usize; 8],
            Option<ValueId>,
        )>()
    }

    fn prepare(
        function: &'function Function,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Self, ProductionSemanticKirErrorV1> {
        let floor = budget.storage();
        budget.reserve_storage(Self::headers())?;
        let body = function.body.as_ref().ok_or_else(source_issued_error_v29)?;
        let mut count = 0;
        let mut generic = false;
        for ty in &function.signature.parameters {
            budget.charge_work(1)?;
            generic |= matches!(ty, Type::Pointer(pointer) if pointer.address_space == AddressSpace::Generic);
        }
        for block in &body.blocks {
            budget.charge_work(1)?;
            for value in &block.parameters {
                budget.charge_work(1)?;
                generic |= matches!(&value.ty, Type::Pointer(pointer) if pointer.address_space == AddressSpace::Generic);
            }
            for operation in &block.operations {
                budget.charge_work(1)?;
                for value in &operation.results {
                    budget.charge_work(1)?;
                    generic |= matches!(&value.ty, Type::Pointer(pointer) if pointer.address_space == AddressSpace::Generic);
                }
                if source_issued_scalar_memory_pointer_v26(operation).is_some() {
                    count = argument_sum_v1(&[count, 1])?;
                }
            }
        }
        let mut rows = emission_vec_v1(if generic { count } else { 0 }, budget)?;
        if generic {
            for block in &body.blocks {
                budget.charge_work(1)?;
                for operation in &block.operations {
                    budget.charge_work(1)?;
                    if let Some(pointer) = source_issued_scalar_memory_pointer_v26(operation) {
                        rows.push((pointer, None));
                    }
                }
            }
        }
        call_splice_sort_work_v1(rows.len(), budget).map_err(source_address_call_error_v29)?;
        rows.sort_unstable_by_key(|row| row.0);
        budget.charge_work(rows.len())?;
        rows.dedup_by_key(|row| row.0);
        let owned = budget
            .storage()
            .checked_sub(floor)
            .ok_or(ArgumentResourceV1::Accounting)?;
        // Output backing is allocated before this scope, so only the index,
        // walk frames and CFG are refunded while rows remain live and charged.
        with_canonical_call_scratch_v1(budget, |budget| {
            if rows.is_empty() {
                return Ok(());
            }
            let actual = SourceIssuedActualV29::from_function(function, budget)?;
            let mut used = 0;
            for index in 0..rows.len() {
                budget.charge_work(2)?;
                if matches!(actual.value(rows[index].0, budget)?.ty,
                    Type::Pointer(pointer) if pointer.address_space == AddressSpace::Generic)
                {
                    rows[used] = rows[index];
                    used += 1;
                }
            }
            rows.truncate(used);
            if rows.is_empty() {
                return Ok(());
            }
            source_issued_pointer_walk_quote_v26(actual.values.len(), rows.len(), budget)?;
            fe2o3_kernel_ir::with_function_control_flow_v1(
                function,
                Default::default(),
                budget,
                |view| {
                    for row in &mut rows {
                        row.1 = source_issued_pointer_walk_v26(&actual, row.0, None, view)?;
                    }
                    Ok(())
                },
            )
            .map_err(|error| match error {
                fe2o3_kernel_ir::FunctionControlFlowScopeErrorV1::Resource(error) => error.into(),
                _ => source_issued_error_v29(),
            })
        })?;
        if budget.storage() != argument_sum_v1(&[floor, owned])? {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        Ok(Self {
            function,
            rows,
            slot: std::ptr::from_ref(budget) as usize,
            ledger: budget.work_ledger_identity_v1(),
            floor,
            owned,
        })
    }

    fn query(
        &self,
        function: &Function,
        pointer: ValueId,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Option<ValueId>, ProductionSemanticKirErrorV1> {
        if self.slot != std::ptr::from_ref(budget) as usize
            || self.ledger != budget.work_ledger_identity_v1()
            || budget.storage() < argument_sum_v1(&[self.floor, self.owned])?
        {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        budget.charge_work(2)?;
        if !std::ptr::eq(self.function, function) {
            return Err(source_issued_error_v29());
        }
        charge_execution_cfg_lookup_v29(self.rows.len(), budget)?;
        let index = self
            .rows
            .binary_search_by_key(&pointer, |row| row.0)
            .map_err(|_| source_issued_error_v29())?;
        Ok(self.rows[index].1)
    }
}

fn source_issued_scalar_memory_pointer_v26(operation: &Operation) -> Option<ValueId> {
    match operation.kind {
        OperationKind::Load { pointer, .. }
        | OperationKind::GuardedLoad { pointer, .. }
        | OperationKind::Store { pointer, .. }
        | OperationKind::GuardedStore { pointer, .. } => Some(pointer),
        _ => None,
    }
}

#[cfg(test)]
mod source_issued_global_origin_batch_tests_v26 {
    use super::*;
    include!("production_source_issued_global_origin_batch_v26_tests.rs");
}
