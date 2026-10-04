// Opt-in original-call/native carriage. Access domains and pointer formation
// remain separate obligations; the V18/V26 completion paths are unchanged.
struct PendingSourceNativeWritesV88<'s, 'g> {
    original: &'s ProductionSourceCorrespondenceV18<'s>,
    optimized: &'s ProductionOptimizedSourceCorrespondenceV18<'s>,
    native: &'s fe2o3_pliron::PendingCanonicalGlobalAccessesV18<'s, 'g>,
    root: usize,
    rows: &'s [Option<OptimizedSourceWriteV87>],
    slot: usize,
    ledger: fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
    required: usize,
}

struct PendingSourceNativeWriteV88<'s> {
    fact: &'s OptimizedSourceWriteV87,
    guard: GlobalSourceGuardV85,
}

impl PendingSourceNativeWriteV88<'_> {
    const fn grants_memory_or_launch_authority(&self) -> bool {
        false
    }
    const fn requires_address_formation_domain(&self) -> bool {
        true
    }
}

fn source_native_write_headers_v88(
    capture: usize,
    alignment: usize,
) -> SourceOwnedResultV18<usize> {
    type Frame<'a> = (
        PendingSourceNativeWritesV88<'a, 'a>,
        PendingSourceNativeWriteV88<'a>,
        Vec<Option<OptimizedSourceWriteV87>>,
        Option<OptimizedSourceWriteV87>,
        [&'a (); 20],
        [usize; 12],
        [bool; 4],
        [SliceOperation; 7],
        [SliceDefinition; 2],
        [ValueId; 7],
        [&'a Operation; 7],
        [&'a CanonicalKirDefinitionRefV1<'a>; 2],
        [(&'a Operation, &'a ValueDef); 2],
        Option<[(&'a Operation, &'a ValueDef); 2]>,
        Result<
            Option<[(&'a Operation, &'a ValueDef); 2]>,
            fe2o3_pliron::CanonicalRankedPolicyFailureV1,
        >,
        SourceOwnedResultV18<Vec<Option<OptimizedSourceWriteV87>>>,
        SourceOwnedResultV18<Option<PendingSourceNativeWriteV88<'a>>>,
        SourceOwnedResultV18<GlobalSourceGuardV85>,
        SourceOwnedResultV18<()>,
        std::thread::Result<SourceOwnedResultV18<()>>,
        std::slice::Iter<'a, Option<OptimizedSourceWriteV87>>,
        std::array::IntoIter<SliceOperation, 4>,
        fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
    );
    Ok(argument_sum_v1(&[
        optimized_write_headers_v87()?,
        size_of::<Frame<'_>>(),
        std::mem::align_of::<Frame<'_>>(),
        argument_product_v1(2, size_of::<SourceOwnedResultV18<Frame<'_>>>())?,
        argument_product_v1(3, capture)?,
        alignment,
    ])?)
}

fn source_native_write_guard_v88(
    inventory: &fe2o3_kernel_analysis::CanonicalKirInventoryV18<'_>,
    fact: &OptimizedSourceWriteV87,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<GlobalSourceGuardV85> {
    let definition = |value, budget: &mut ArgumentBudgetV1<'_>| {
        inventory
            .definition_for_value(fact.output[6].block.function, value, budget)
            .map_err(source_pointer_inventory_error_v18)?
            .filter(|row| row.ty == &Type::BOOL)
            .map(|row| row.coordinate)
            .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                "native write boolean definition differs",
            ))
    };
    Ok(GlobalSourceGuardV85::ExplicitPredicate {
        condition: definition(fact.tail.predicate, budget)?,
        bound_comparison: definition(fact.tail.extent, budget)?,
    })
}

fn check_source_native_write_v88(
    original: &ProductionSourceCorrespondenceV18<'_>,
    optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    native: &fe2o3_pliron::PendingCanonicalGlobalAccessesV18<'_, '_>,
    fact: &OptimizedSourceWriteV87,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<()> {
    let inventory = optimized.pending_global_output_v18(original, budget)?;
    native
        .check_owner(inventory.owner(), budget)
        .map_err(slice_entry_native_error_v25)?;
    original.retain_query((|| {
        budget.charge_work(64)?;
        // Native candidates must be the exact owner rows, not equal copied KIR.
        // The native scope also checked every unqueried global effect.
        for coordinate in [
            fact.output[0],
            fact.output[4],
            fact.output[5],
            fact.output[6],
        ] {
            let expected = source_operation_row_v18(inventory, coordinate, budget)?.operation;
            let actual = native
                .operation(inventory.owner(), coordinate, budget)
                .map_err(slice_entry_native_error_v25)?;
            if !actual.is_some_and(|row| std::ptr::eq(row, expected)) {
                return original
                    .source
                    .missing("native write carrier lacks its exact occurrence");
            }
        }
        let GlobalSourceGuardV85::ExplicitPredicate {
            condition,
            bound_comparison,
        } = source_native_write_guard_v88(inventory, fact, budget)?
        else {
            unreachable!()
        };
        let definitions = native
            .explicit_guard_definitions_v86(inventory.owner(), condition, bound_comparison, budget)
            .map_err(slice_entry_native_error_v25)?
            .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                "native write predicate is absent",
            ))?;
        for ((actual, value), expected) in
            definitions.into_iter().zip([condition, bound_comparison])
        {
            let SliceDefinition::Result { operation, result } = expected else {
                return original
                    .source
                    .missing("native write predicate is not a result");
            };
            let row = source_operation_row_v18(inventory, operation, budget)?.operation;
            if !std::ptr::eq(actual, row)
                || !row
                    .results
                    .get(result as usize)
                    .is_some_and(|expected| std::ptr::eq(value, expected))
            {
                return original
                    .source
                    .missing("native write predicate changed its actual definition");
            }
        }
        // Replay the full immutable suffix, including zero and Select producers
        // which are not themselves memory carriers in the native census.
        let store = source_operation_row_v18(inventory, fact.output[6], budget)?.operation;
        let mut operations = [store; 7];
        for (operation, coordinate) in operations.iter_mut().zip(fact.output) {
            budget.charge_work(1)?;
            *operation = source_operation_row_v18(inventory, coordinate, budget)?.operation;
        }
        let tail = check_checked_write_tail_v85(
            CheckedWriteInputsV85 {
                slice: fact.receiver,
                index: fact.index,
                precondition: None,
                value: fact.value,
                element: fact.source.element,
            },
            &operations,
            budget,
        )
        .map_err(source_emission_error_v18)?;
        if tail != fact.tail {
            return original
                .source
                .missing("native write suffix changed its source recipe");
        }
        native
            .check_owner(inventory.owner(), budget)
            .map_err(slice_entry_native_error_v25)
    })())
}

impl PendingSourceNativeWritesV88<'_, '_> {
    fn check(&self, budget: &mut ArgumentBudgetV1<'_>) -> SourceOwnedResultV18<()> {
        if self.slot != std::ptr::from_ref(&*budget) as usize
            || self.ledger != budget.work_ledger_identity_v1()
            || budget.storage() < self.required
        {
            self.original.source.cleanup.deny_refund();
            self.native.refuse_retained_custody();
            return self
                .original
                .retain_query(Err(ArgumentResourceV1::Accounting.into()));
        }
        let inventory = self
            .optimized
            .pending_global_output_v18(self.original, budget)?;
        self.original.retain_query(
            self.native
                .check_owner(inventory.owner(), budget)
                .map_err(slice_entry_native_error_v25),
        )
    }

    fn original_call_count(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<usize> {
        self.check(budget)?;
        Ok(self.rows.len())
    }

    fn write(
        &self,
        ordinal: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<Option<PendingSourceNativeWriteV88<'_>>> {
        self.check(budget)?;
        self.original.retain_query((|| {
            budget.charge_work(2)?;
            let row = self
                .rows
                .get(ordinal)
                .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                    "native write original-call ordinal is absent",
                ))?;
            row.as_ref()
                .map(|fact| {
                    let inventory = self.optimized.output_inventory(budget)?;
                    Ok(PendingSourceNativeWriteV88 {
                        fact,
                        guard: source_native_write_guard_v88(inventory, fact, budget)?,
                    })
                })
                .transpose()
        })())
    }
}

impl ProductionSourceCorrespondenceV18<'_> {
    fn with_pending_source_native_writes_v88<'work>(
        &self,
        optimized: &ProductionOptimizedSourceCorrespondenceV18<'_>,
        root: usize,
        native: &fe2o3_pliron::PendingCanonicalGlobalAccessesV18<'_, '_>,
        budget: &mut ArgumentBudgetV1<'work>,
        consume: impl for<'s, 'g> FnOnce(
            &PendingSourceNativeWritesV88<'s, 'g>,
            &mut ArgumentBudgetV1<'work>,
        ) -> SourceOwnedResultV18<()>,
    ) -> SourceOwnedResultV18<()> {
        self.check(budget)?;
        let inventory = optimized.pending_global_output_v18(self, budget)?;
        self.retain_query(
            native
                .check_owner(inventory.owner(), budget)
                .map_err(slice_entry_native_error_v25),
        )?;
        let consume = SourceCallbackCustodyV29::new(consume);
        let headers = source_native_write_headers_v88(
            std::mem::size_of_val(&consume),
            std::mem::align_of_val(&consume),
        )?;
        self.retain_query(source_scalar_normalization_scratch_v18(
            self.source.cleanup,
            budget,
            headers,
            move |budget| {
                let mut consume = consume;
                let rows = optimized_source_writes_v87(self, optimized, root, budget)?;
                for fact in rows.iter().flatten() {
                    budget.charge_work(1)?;
                    check_source_native_write_v88(self, optimized, native, fact, budget)?;
                }
                let view = PendingSourceNativeWritesV88 {
                    original: self,
                    optimized,
                    native,
                    root,
                    rows: &rows,
                    slot: std::ptr::from_ref(&*budget) as usize,
                    ledger: budget.work_ledger_identity_v1(),
                    required: budget.storage(),
                };
                let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    consume
                        .take()
                        .expect("source native write callback is invoked once")(
                        &view, budget
                    )
                }));
                drop(consume);
                let postflight = view.check(budget).and_then(|()| {
                    if budget.storage() == view.required {
                        Ok(())
                    } else {
                        self.retain_query(Err(ArgumentResourceV1::Accounting.into()))
                    }
                });
                source_owned_finish_callback_v18(
                    caught,
                    self.source.guard.first.get(),
                    postflight,
                    self.source.cleanup,
                    budget,
                    0,
                )
            },
        ))
    }
}

#[cfg(test)]
include!("production_source_native_writes_v88_tests.rs");
