// Independently replay the source query at retained compiler custody. The table
// owns no graph facts and cannot substitute an elision label for source proof.

struct MaskedSourceAssertionTableV1<'source> {
    source: &'source fe2o3_mir_model::semantic_mir_v1::AdmittedInertSemanticMirV1,
    function: SemanticFunctionIdV1,
    rows: Vec<Option<fe2o3_mir_model::SemanticMaskedShiftFactV1<'source>>>,
    slot: usize,
    ledger: fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
    floor: usize,
    reserved: usize,
    poisoned: std::cell::Cell<bool>,
}

impl MaskedSourceAssertionTableV1<'_> {
    fn check(&self, budget: &Budget<'_>) -> Result<(), ProjectionError> {
        if self.slot != budget as *const Budget<'_> as usize
            || self.ledger != budget.work_ledger_identity_v1()
            || self
                .floor
                .checked_add(self.reserved)
                .is_none_or(|minimum| budget.storage() < minimum)
        {
            self.poisoned.set(true);
        }
        if self.poisoned.get() {
            Err(resource(Resource::Accounting))
        } else {
            Ok(())
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn proves(
        &self,
        source: &fe2o3_mir_model::semantic_mir_v1::AdmittedInertSemanticMirV1,
        function_id: SemanticFunctionIdV1,
        function: &super::SemanticFunctionDeclV1,
        block: usize,
        expected: bool,
        successor: SemanticBlockIdV1,
        budget: &mut Budget<'_>,
    ) -> Result<bool, ProjectionError> {
        self.check(budget)?;
        budget.charge_work(8).map_err(resource)?;
        if !std::ptr::eq(self.source, source)
            || function_id != self.function
            || !source
                .functions()
                .get(function_id.index() as usize)
                .is_some_and(|actual| std::ptr::eq(actual, function))
        {
            return Err(reject(CanonicalAssertionErrorV1::Binding(
                "masked assertion query belongs to another actual source function",
            )));
        }
        let Some(actual) = function.blocks().get(block) else {
            return Err(reject(CanonicalAssertionErrorV1::Binding(
                "masked assertion block is outside its source function",
            )));
        };
        let super::SemanticTerminatorKindV1::Assert {
            expected: actual_expected,
            target,
            ..
        } = actual.terminator().kind()
        else {
            return Err(reject(CanonicalAssertionErrorV1::Binding(
                "masked assertion query is not an actual source Assert",
            )));
        };
        if *actual_expected != expected || target.target() != successor {
            return Err(reject(CanonicalAssertionErrorV1::Binding(
                "masked assertion query changed expected value or source successor",
            )));
        }
        Ok(self
            .rows
            .get(block)
            .and_then(Option::as_ref)
            .is_some_and(|fact| {
                expected
                    && std::ptr::eq(fact.owner(), source)
                    && fact.function() == function_id
                    && fact.assertion_block().index() as usize == block
                    && fact.successor_block() == successor
            }))
    }
}

impl<'r, 'i, 'g, 'b, 'w> CanonicalAssertionSessionV1<'r, 'i, 'g, 'b, 'w> {
    #[cfg(test)]
    pub(super) fn with_source_masked_assertions_query_budget_v1<'test, T>(
        &self,
        budget: &mut Budget<'test>,
        correspondence_owner: SemanticFunctionIdV1,
        semantic_function: SemanticFunctionIdV1,
        run: impl for<'scope> FnOnce(
            &mut CanonicalSourceAssertionFactsV1<'scope, 'i, 'g, 'scope, 'test>,
        ) -> Result<T, ProjectionError>,
    ) -> Result<T, ProjectionError> {
        CanonicalAssertionSessionV1 {
            owner: self.owner,
            origins: self.origins,
            report: self.report,
            budget,
        }
        .with_source_masked_assertions_v1(correspondence_owner, semantic_function, run)
    }

    pub(super) fn with_source_masked_assertions_v1<T>(
        &mut self,
        correspondence_owner: SemanticFunctionIdV1,
        semantic_function: SemanticFunctionIdV1,
        run: impl for<'scope> FnOnce(
            &mut CanonicalSourceAssertionFactsV1<'scope, 'i, 'g, 'scope, 'w>,
        ) -> Result<T, ProjectionError>,
    ) -> Result<T, ProjectionError> {
        let source = self.owner.semantic_ssa().source_semantic();
        let function = source
            .functions()
            .get(semantic_function.index() as usize)
            .ok_or_else(|| {
                reject(CanonicalAssertionErrorV1::Binding(
                    "masked assertion preparation lost its source function",
                ))
            })?;
        self.owner
            .semantic_ssa()
            .plan_for_function(semantic_function)
            .ok_or_else(|| {
                reject(CanonicalAssertionErrorV1::Binding(
                    "masked assertion preparation lost its admitted source SSA plan",
                ))
            })?;
        let budget = &mut *self.budget;
        let mut table = MaskedSourceAssertionTableV1 {
            source,
            function: semantic_function,
            rows: Vec::new(),
            slot: budget as *const Budget<'_> as usize,
            ledger: budget.work_ledger_identity_v1(),
            floor: budget.storage(),
            reserved: 0,
            poisoned: std::cell::Cell::new(false),
        };
        let mut deferred_panic = None;
        let mut result = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            table.check(budget)?;
            budget
                .charge_work(function.blocks().len())
                .map_err(resource)?;
            let has_shift_assertion = function.blocks().iter().any(|block| {
                matches!(
                    block.terminator().kind(),
                    super::SemanticTerminatorKindV1::Assert {
                        message: super::SemanticAssertMessageV1::Overflow {
                            operation: super::SemanticBinaryOpV1::ShiftLeft
                                | super::SemanticBinaryOpV1::ShiftRight,
                            ..
                        },
                        ..
                    }
                )
            });
            let count = if has_shift_assertion {
                function.blocks().len()
            } else {
                0
            };
            let width =
                std::mem::size_of::<Option<fe2o3_mir_model::SemanticMaskedShiftFactV1<'_>>>();
            budget.charge_work(count).map_err(resource)?;
            let requested = count
                .checked_mul(width)
                .ok_or_else(|| resource(Resource::Arithmetic))?;
            budget.reserve_storage(requested).map_err(resource)?;
            table.reserved = requested;
            table
                .rows
                .try_reserve_exact(count)
                .map_err(|_| resource(Resource::Allocation))?;
            let excess = table
                .rows
                .capacity()
                .checked_sub(count)
                .and_then(|count| count.checked_mul(width))
                .ok_or_else(|| resource(Resource::Arithmetic))?;
            let reserved = table
                .reserved
                .checked_add(excess)
                .ok_or_else(|| resource(Resource::Arithmetic))?;
            budget.reserve_storage(excess).map_err(resource)?;
            table.reserved = reserved;
            table.rows.resize_with(count, || None);
            if has_shift_assertion {
                fe2o3_lower_mir_kernel::with_production_semantic_masked_shift_query_v1(
                    source,
                    semantic_function,
                    fe2o3_mir_model::SemanticMaskedShiftLimitsV1::default(),
                    budget,
                    |query, budget| {
                        for (block, row) in table.rows.iter_mut().enumerate() {
                            budget.charge_work(1)?;
                            *row = query
                                .assertion(SemanticBlockIdV1::from_index(block as u32), budget)?;
                        }
                        Ok(())
                    },
                )
                .map_err(|error| reject(CanonicalAssertionErrorV1::MaskedAssertion(error)))?;
            }
            run(&mut CanonicalSourceAssertionFactsV1 {
                owner: self.owner,
                origins: self.origins,
                report: self.report,
                budget,
                correspondence_owner,
                semantic_function,
                masked: Some(&table),
            })
        })) {
            Ok(result) => result,
            Err(payload) => {
                deferred_panic = Some(payload);
                Err(reject(CanonicalAssertionErrorV1::MaskedAssertion(
                    fe2o3_lower_mir_kernel::ProductionSemanticMaskedShiftQueryErrorV1::Panicked,
                )))
            }
        };
        if let Err(error) = table.check(budget) {
            if let Err(payload) =
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| drop(result)))
            {
                deferred_panic = Some(payload);
            }
            result = Err(error);
        }
        let (reserved, poisoned) = (table.reserved, table.poisoned.get());
        drop(table);
        if !poisoned
            && let Err(error) = budget.release_storage(reserved)
        {
            drop(result);
            drop(deferred_panic);
            return Err(resource(error));
        }
        drop(deferred_panic);
        result
    }
}
