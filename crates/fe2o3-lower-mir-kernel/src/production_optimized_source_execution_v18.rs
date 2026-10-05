//! Original lifecycle sites composed with one actual checked successor.
use super::*;
use fe2o3_kernel_ir::{ExecutionOperationV15 as Execution, ExecutionRoleV15 as Role};

#[path = "production_optimized_source_execution_census_v18.rs"]
mod census;
#[path = "production_optimized_source_native_lifecycle_v18.rs"]
mod native;
pub use native::{
    ProductionLifecycleCheckedNativePoliciesV18, ProductionMixedMemoryCheckedNativePoliciesV26,
    ProductionPredicatedMemoryCheckedNativePoliciesV89,
    ProductionPrivateMemoryCheckedNativePoliciesV18, ProductionSourceNativeLifecycleDiagnosticV18,
    ProductionSourceNativeLifecycleErrorV18, ProductionSourcePrivateMemoryRootRequestV18,
};
#[cfg(test)]
#[path = "production_optimized_source_execution_controls_v18_tests.rs"]
mod controls;

/// The closed original lifecycle subset checked by this scoped relation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductionOptimizedExecutionKindV18 {
    /// The original root's unique context issuance.
    ContextIssue,
    /// One original provider instance's workgroup derivation.
    WorkgroupDerive,
    /// One original provider instance's normal return.
    ScopeEnd,
}

#[derive(Clone, Copy)]
struct Recipe {
    root: usize,
    instance: usize,
    block: SemanticBlockIdV1,
    kind: ProductionOptimizedExecutionKindV18,
    input: OpCoordinate,
    output: Option<OpCoordinate>,
}

fn execution_recipe_owned_headers_v18<T, E, F>(_: &F) -> Result<usize, ArgumentResourceV1> {
    type Entry<F> = (Vec<Recipe>, usize, F);
    type Capture<'a, 'work, F> = (
        &'a ProductionOptimizedSourceCorrespondenceV18<'a>,
        &'a ProductionSourceOwnedViewV18<'a>,
        &'a mut ArgumentBudgetV1<'work>,
        &'a std::cell::Cell<usize>,
        &'a std::cell::Cell<usize>,
        F,
        bool,
    );
    type Construct<'a, 'work, F> = (
        &'a ProductionOptimizedSourceCorrespondenceV18<'a>,
        &'a ProductionSourceOwnedViewV18<'a>,
        &'a mut ArgumentBudgetV1<'work>,
        &'a F,
        usize,
    );
    type OwnedConstruct<'a, F> = (
        F,
        &'a ProductionSourceOwnedViewV18<'a>,
        &'a ProductionOptimizedSourceCorrespondenceV18<'a>,
    );
    type Invoke<'a, 'work, F> = (
        F,
        &'a ProductionOptimizedExecutionRecipesV18<'a>,
        &'a mut ArgumentBudgetV1<'work>,
    );
    argument_sum_v1(&[
        argument_product_v1(2, size_of::<Capture<'_, '_, F>>())?,
        argument_product_v1(2, std::mem::align_of::<Capture<'_, '_, F>>())?,
        size_of::<std::panic::AssertUnwindSafe<Capture<'_, '_, F>>>(),
        size_of::<Construct<'_, '_, F>>(),
        size_of::<std::panic::AssertUnwindSafe<Construct<'_, '_, F>>>(),
        argument_product_v1(2, size_of::<OwnedConstruct<'_, F>>())?,
        argument_product_v1(2, std::mem::align_of::<OwnedConstruct<'_, F>>())?,
        size_of::<std::panic::AssertUnwindSafe<OwnedConstruct<'_, F>>>(),
        size_of::<Entry<F>>(),
        argument_product_v1(2, size_of::<SourceOwnedResultV18<Entry<F>>>())?,
        size_of::<std::thread::Result<SourceOwnedResultV18<Entry<F>>>>(),
        size_of::<std::panic::AssertUnwindSafe<Entry<F>>>(),
        size_of::<std::thread::Result<()>>(),
        size_of::<Invoke<'_, '_, F>>(),
        size_of::<std::panic::AssertUnwindSafe<Invoke<'_, '_, F>>>(),
        size_of::<ProductionOptimizedExecutionRecipesV18<'_>>(),
        size_of::<Vec<Recipe>>(),
        argument_product_v1(2, size_of::<SourceOwnedResultV18<Vec<Recipe>>>())?,
        size_of::<std::thread::Result<Result<T, E>>>(),
        size_of::<Result<T, E>>(),
        size_of::<std::panic::AssertUnwindSafe<Result<T, E>>>(),
        size_of::<fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1>(),
        argument_product_v1(12, size_of::<usize>())?,
        source_reference_cleanup_headers_v29()?,
    ])
}

#[cfg(test)]
thread_local! {
    static RECIPE_REFUSE_BEFORE_INVOCATION_V18: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// A non-escaping original-event/insertion/input/output relation.
/// This is not a native annotation, final policy report, or launch authority.
/// The current native missing-recipe gate remains independent and mandatory.
///
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::ProductionOptimizedExecutionRecipesV18;
/// fn forge() -> ProductionOptimizedExecutionRecipesV18<'static> {
///     ProductionOptimizedExecutionRecipesV18 { rows: &[] }
/// }
/// ```
///
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::{ProductionOptimizedSourceCorrespondenceV18,
///     ProductionSourceOwnedViewErrorV18};
/// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1;
/// fn escape(view: &ProductionOptimizedSourceCorrespondenceV18<'_>,
///     budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>) {
///     let _escaped = view.with_execution_recipes_v18(budget,
///         |recipes, _| Ok::<_, ProductionSourceOwnedViewErrorV18>(recipes));
/// }
/// ```
pub struct ProductionOptimizedExecutionRecipesV18<'scope> {
    optimized: &'scope ProductionOptimizedSourceCorrespondenceV18<'scope>,
    rows: &'scope [Recipe],
    floor: usize,
}

impl ProductionOptimizedExecutionRecipesV18<'_> {
    fn observe_custody(&self, budget: &ArgumentBudgetV1<'_>) -> SourceOwnedResultV18<()> {
        if self.optimized.observe_custody(budget).is_err() || budget.storage() < self.floor {
            self.optimized.original.source.cleanup.deny_refund();
            return Err(ArgumentResourceV1::Accounting.into());
        }
        Ok(())
    }

    fn check(&self, budget: &ArgumentBudgetV1<'_>) -> SourceOwnedResultV18<()> {
        self.optimized.retain(self.observe_custody(budget))?;
        self.optimized.check(budget)
    }

    /// Complete count, including explicitly removed unreachable occurrences.
    pub fn len(&self, budget: &mut ArgumentBudgetV1<'_>) -> SourceOwnedResultV18<usize> {
        self.check(budget)?;
        self.optimized
            .retain(budget.charge_work(1).map_err(Into::into))?;
        Ok(self.rows.len())
    }

    /// Original root, instance, semantic block, and lifecycle role.
    pub fn original_site(
        &self,
        ordinal: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<(
        usize,
        usize,
        SemanticBlockIdV1,
        ProductionOptimizedExecutionKindV18,
    )> {
        let row = self.row(ordinal, budget)?;
        Ok((row.root, row.instance, row.block, row.kind))
    }

    /// Exact original operation and its checked retained/unreachable disposition.
    pub fn operation(
        &self,
        ordinal: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<ProductionOptimizedSourceOperationV18> {
        let row = self.row(ordinal, budget)?;
        Ok(match row.output {
            Some(output) => ProductionOptimizedSourceOperationV18::Retained {
                input: row.input,
                output,
            },
            None => ProductionOptimizedSourceOperationV18::RemovedUnreachable { input: row.input },
        })
    }

    fn row(
        &self,
        ordinal: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<&Recipe> {
        self.check(budget)?;
        self.optimized.retain((|| {
            budget.charge_work(1)?;
            self.rows
                .get(ordinal)
                .ok_or(ProductionSourceOwnedViewErrorV18::Binding(
                    "execution recipe ordinal",
                ))
        })())
    }
}

impl ProductionOptimizedSourceCorrespondenceV18<'_> {
    #[cfg(test)]
    pub(crate) fn test_execution_recipe_refuse_before_invocation_v18(&self) {
        RECIPE_REFUSE_BEFORE_INVOCATION_V18.set(true);
    }
    #[cfg(test)]
    pub(crate) fn test_execution_recipe_controls_v18(
        &self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> SourceOwnedResultV18<()> {
        controls::actual_controls(self, budget)
    }
    /// Checks the complete closed lifecycle census before entering the consumer.
    /// Original, input and output owners, their epochs, and the continuing ledger
    /// remain borrowed throughout this scope. No inert profile is manufactured.
    /// The returned value and result header retain `size_of::<T>() +
    /// size_of::<Result<T, E>>()` credits in the surrounding source lease. The
    /// caller may release those fixed credits only after dropping that result;
    /// separately allocated consumer output remains the consumer's obligation.
    pub fn with_execution_recipes_v18<'work, T, E>(
        &self,
        budget: &mut ArgumentBudgetV1<'work>,
        consume: impl for<'scope> FnOnce(
            &ProductionOptimizedExecutionRecipesV18<'scope>,
            &mut ArgumentBudgetV1<'work>,
        ) -> Result<T, E>,
    ) -> Result<T, E>
    where
        E: From<ProductionSourceOwnedViewErrorV18>,
    {
        self.with_execution_recipes_owned_v18(budget, false, consume)
    }

    // Only typed sibling integrations use this entrance. Their enclosing
    // callback already prepays the returned T/E envelopes; no result credit
    // transfer is introduced inside a native exact-floor callback.
    fn with_execution_recipes_prepaid_v18<'work, T, E>(
        &self,
        budget: &mut ArgumentBudgetV1<'work>,
        consume: impl for<'scope> FnOnce(
            &ProductionOptimizedExecutionRecipesV18<'scope>,
            &mut ArgumentBudgetV1<'work>,
        ) -> Result<T, E>,
    ) -> Result<T, E>
    where
        E: From<ProductionSourceOwnedViewErrorV18>,
    {
        self.with_execution_recipes_owned_v18(budget, true, consume)
    }

    fn with_execution_recipes_owned_v18<'work, T, E, F>(
        &self,
        budget: &mut ArgumentBudgetV1<'work>,
        result_prepaid: bool,
        consume: F,
    ) -> Result<T, E>
    where
        E: From<ProductionSourceOwnedViewErrorV18>,
        F: for<'scope> FnOnce(
            &ProductionOptimizedExecutionRecipesV18<'scope>,
            &mut ArgumentBudgetV1<'work>,
        ) -> Result<T, E>,
    {
        let source = self.original.source;
        let floor = budget.storage();
        let slot = std::ptr::from_ref(budget) as usize;
        let ledger = budget.work_ledger_identity_v1();
        let returned = std::cell::Cell::new(0);
        let accepted = std::cell::Cell::new(0);
        // Own F before the original query and result-credit arithmetic. The
        // closed construction still uses the unchanged scoped scratch helper.
        let caught = {
            let budget = &mut *budget;
            let returned = &returned;
            let accepted = &accepted;
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
                if result_prepaid {
                    self.check(budget)?;
                } else {
                    self.query(budget)?;
                    self.retain((|| {
                        let bytes = argument_sum_v1(&[size_of::<T>(), size_of::<Result<T, E>>()])?;
                        budget.reserve_storage(bytes)?;
                        returned.set(bytes);
                        Ok(())
                    })())?;
                }
                let headers = self.retain((|| {
                    let bytes = execution_recipe_owned_headers_v18::<T, E, _>(&consume)?;
                    budget.reserve_storage(bytes)?;
                    accepted.set(bytes);
                    Ok(bytes)
                })())?;
                let construction_floor = budget.storage();
                let (rows, consume) = scoped_source_attempt_v29(
                    source.cleanup,
                    budget,
                    construction_floor,
                    move |budget| {
                        let rows = source.retain_construction(|| build(self, budget))?;
                        Ok::<_, ProductionSourceOwnedViewErrorV18>((rows, consume))
                    },
                )?;
                // The shared helper has settled its own temporary header.
                let rows_credit = argument_product_v1(rows.capacity(), size_of::<Recipe>())?;
                let storage = argument_sum_v1(&[headers, rows_credit])?;
                if construction_floor.checked_add(rows_credit) != Some(budget.storage()) {
                    source.cleanup.deny_refund();
                    return Err(ArgumentResourceV1::Accounting.into());
                }
                Ok::<_, ProductionSourceOwnedViewErrorV18>((rows, storage, consume))
            }))
        };
        let local = match &caught {
            Ok(Ok((_, storage, _))) => *storage,
            _ => accepted.get(),
        };
        let custody = if slot == std::ptr::from_ref(budget) as usize
            && ledger == budget.work_ledger_identity_v1()
            && floor
                .checked_add(returned.get())
                .and_then(|value| value.checked_add(local))
                == Some(budget.storage())
        {
            self.observe_custody(budget)
        } else {
            source.cleanup.deny_refund();
            Err(ArgumentResourceV1::Accounting.into())
        };
        let (rows, storage, consume) = match caught {
            Ok(Ok(entry)) if custody.is_ok() => entry,
            Ok(Ok(entry)) => {
                let dropped =
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || drop(entry)));
                let _ = self.observe_custody(budget);
                if let Err(payload) = dropped {
                    std::panic::resume_unwind(payload);
                }
                return self
                    .retain(Err(ArgumentResourceV1::Accounting.into()))
                    .map_err(Into::into);
            }
            // The public result credit remains transferred even on failure.
            // The closed helper already settled only its construction scratch.
            Ok(Err(error)) => {
                if custody.is_ok() && budget.release_storage(accepted.get()).is_err() {
                    source.cleanup.deny_refund();
                }
                return self.retain(Err(error)).map_err(Into::into);
            }
            Err(payload) => {
                if custody.is_ok() && budget.release_storage(accepted.get()).is_err() {
                    source.cleanup.deny_refund();
                }
                std::panic::resume_unwind(payload);
            }
        };
        let view = ProductionOptimizedExecutionRecipesV18 {
            optimized: self,
            rows: &rows,
            floor: budget.storage(),
        };
        #[cfg(test)]
        if RECIPE_REFUSE_BEFORE_INVOCATION_V18.replace(false) {
            let _ = self.retain::<()>(Err(ProductionSourceOwnedViewErrorV18::Binding(
                "execution recipe test pre-invocation refusal",
            )));
        }
        let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            view.check(budget).map_err(E::from)?;
            consume(&view, budget)
        }));
        let prior = source.guard.first.get();
        // Observe the new scope's floor even on a consumer error or panic.
        let postflight = if matches!(&caught, Ok(Ok(_))) {
            view.check(budget)
        } else {
            view.observe_custody(budget)
        };
        drop(view);
        drop(rows);
        // An arbitrary consumer may return newly owned T or E backing. Refund
        // only our measured rows/frames, never the consumer's storage delta.
        let released = if source.cleanup.is_denied() {
            Err(ArgumentResourceV1::Accounting)
        } else {
            budget
                .release_storage(storage)
                .inspect_err(|_| source.cleanup.deny_refund())
        };
        match caught {
            Err(payload) => std::panic::resume_unwind(payload),
            Ok(Err(error)) => match prior {
                Some(first) => Err(first.error().into()),
                None => Err(error),
            },
            Ok(Ok(value)) => {
                match source.retain_query(postflight.and(released.map_err(Into::into))) {
                    Ok(()) => Ok(value),
                    Err(error) => {
                        drop(value);
                        Err(error.into())
                    }
                }
            }
        }
    }
}

fn source_error(error: ProductionSemanticKirErrorV1) -> ProductionSourceOwnedViewErrorV18 {
    match error {
        ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(error) => error.into(),
        _ => ProductionSourceOwnedViewErrorV18::Binding("execution recipe original source census"),
    }
}

fn module_error(error: ScopedModuleErrorV29) -> ProductionSourceOwnedViewErrorV18 {
    match error {
        ScopedModuleErrorV29::Source(error) => source_error(error),
        ScopedModuleErrorV29::Canonical(
            fe2o3_kernel_ir::CanonicalKernelIrReplayAdmissionErrorV18::Resource(error),
        ) => error.into(),
        _ => ProductionSourceOwnedViewErrorV18::Binding(
            "execution recipe original source projection",
        ),
    }
}

fn build(
    view: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<Vec<Recipe>> {
    let source = view.original.source;
    let input = view.checked.input();
    budget.charge_work(input.operations().len())?;
    let count = input
        .operations()
        .iter()
        .filter(|row| matches!(row.operation.kind, OperationKind::Execution(_)))
        .count();
    let mut rows = resources::vector(count, budget)?;
    let owned = &source.owner.inner.source;
    // Only closed construction runs in this projection. Its temporary vectors
    // settle before the public scoped recipe consumer can run.
    owned
        .input
        .with_source_with_cleanup(
            &owned.owner,
            &owned.launch,
            source.cleanup,
            budget,
            |original, budget| {
                Ok(view.retain(census::build(view, original, count, &mut rows, budget)))
            },
        )
        .map_err(module_error)??;
    view.check(budget)?;
    private_array_heapsort_v1(
        &mut rows,
        |row| operation_key(row.input),
        &mut SourceCorrespondenceWorkV18(budget),
        || ArgumentResourceV1::Arithmetic.into(),
    )?;
    check_census(input, rows.iter().map(|row| row.input), budget)?;
    let scratch = budget.storage();
    budget.reserve_storage(size_of::<Vec<OpCoordinate>>())?;
    let mut output = resources::vector(count, budget)?;
    for row in &rows {
        budget.charge_work(1)?;
        if let Some(coordinate) = row.output {
            if output.len() >= count {
                return resources::binding("execution recipe output capacity");
            }
            output.push(coordinate);
        }
    }
    private_array_heapsort_v1(
        &mut output,
        |row| operation_key(*row),
        &mut SourceCorrespondenceWorkV18(budget),
        || ArgumentResourceV1::Arithmetic.into(),
    )?;
    check_census(view.checked.output(), output.iter().copied(), budget)?;
    drop(output);
    view.check(budget)?;
    budget.release_storage(
        budget
            .storage()
            .checked_sub(scratch)
            .ok_or(ArgumentResourceV1::Accounting)?,
    )?;
    Ok(rows)
}

fn operation_key(coordinate: OpCoordinate) -> [usize; 3] {
    [
        coordinate.block.function.0 as usize,
        coordinate.block.block as usize,
        coordinate.operation as usize,
    ]
}

fn check_census(
    inventory: &Inventory<'_>,
    coordinates: impl IntoIterator<Item = OpCoordinate>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<()> {
    let mut coordinates = coordinates.into_iter();
    for actual in inventory.operations() {
        budget.charge_work(1)?;
        if matches!(actual.operation.kind, OperationKind::Execution(_))
            && coordinates.next() != Some(actual.coordinate)
        {
            return resources::binding("execution recipe complete operation census");
        }
    }
    budget.charge_work(1)?;
    if coordinates.next().is_some() {
        return resources::binding("execution recipe complete operation census");
    }
    Ok(())
}

fn check_payload(
    event: DeferredLifecycleKindV29,
    operation: &Operation,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<ProductionOptimizedExecutionKindV18> {
    use ProductionOptimizedExecutionKindV18 as Kind;
    budget.charge_work(6)?;
    let (kind, result) = match (event, &operation.kind) {
        (
            DeferredLifecycleKindV29::Issue { result },
            OperationKind::Execution(Execution::ContextIssue),
        ) => (Kind::ContextIssue, Some((result.value, Role::Context))),
        (
            DeferredLifecycleKindV29::Derive { context, result },
            OperationKind::Execution(Execution::WorkgroupDerive { context: actual }),
        ) if *actual == context.value => {
            (Kind::WorkgroupDerive, Some((result.value, Role::Workgroup)))
        }
        (
            DeferredLifecycleKindV29::End { workgroup },
            OperationKind::Execution(Execution::ScopeEnd {
                workgroup: actual,
                discarded,
            }),
        ) if *actual == workgroup.value && discarded.is_empty() => (Kind::ScopeEnd, None),
        _ => return resources::binding("execution recipe original operation payload"),
    };
    let valid = match result {
        Some((value, role)) => {
            operation.results.len() == 1
                && operation.results[0].id == value
                && operation.results[0].ty == Type::Execution(role)
        }
        None => operation.results.is_empty(),
    };
    if !valid {
        return resources::binding("execution recipe original operation results");
    }
    Ok(kind)
}

fn ordered_disposition(
    coordinate: OpCoordinate,
    actual: ProductionOptimizedSourceOperationV18,
) -> SourceOwnedResultV18<Option<OpCoordinate>> {
    match actual {
        ProductionOptimizedSourceOperationV18::Retained { input, output }
            if input == coordinate =>
        {
            Ok(Some(output))
        }
        ProductionOptimizedSourceOperationV18::RemovedUnreachable { input }
            if input == coordinate =>
        {
            Ok(None)
        }
        _ => resources::binding("execution recipe ordered operation rewritten"),
    }
}

fn join_output(
    view: &ProductionOptimizedSourceCorrespondenceV18<'_>,
    coordinate: OpCoordinate,
    budget: &mut ArgumentBudgetV1<'_>,
) -> SourceOwnedResultV18<Option<OpCoordinate>> {
    let input = view.checked.input();
    let output = view.checked.output();
    let before = &input.operations()[resources::operation_index(input, coordinate, budget)?];
    let mapped = ordered_disposition(coordinate, view.operation(coordinate, budget)?)?;
    let after = mapped
        .map(|coordinate| resources::operation_index(output, coordinate, budget))
        .transpose()?
        .map(|index| &output.operations()[index]);
    if let Some(after) = after {
        budget.charge_work(4)?;
        let same = match (&before.operation.kind, &after.operation.kind) {
            (
                OperationKind::Execution(Execution::ContextIssue),
                OperationKind::Execution(Execution::ContextIssue),
            )
            | (
                OperationKind::Execution(Execution::WorkgroupDerive { .. }),
                OperationKind::Execution(Execution::WorkgroupDerive { .. }),
            ) => true,
            (
                OperationKind::Execution(Execution::ScopeEnd { discarded: a, .. }),
                OperationKind::Execution(Execution::ScopeEnd { discarded: b, .. }),
            ) => a.is_empty() && b.is_empty(),
            _ => false,
        };
        if !same
            || before.operands.len() != after.operands.len()
            || before.results.len() != after.results.len()
        {
            return resources::binding("execution recipe checked output payload");
        }
    }
    for (ordinal, definition) in input.definitions()[before.results.clone()]
        .iter()
        .enumerate()
    {
        let descendants = view.definition_descendants(definition.coordinate, budget)?;
        budget.charge_work(2)?;
        match after {
            None if descendants.is_empty() => (),
            Some(after) if descendants.len() == 1 => {
                let expected = Definition::Result {
                    operation: after.coordinate,
                    result: u32::try_from(ordinal).map_err(|_| ArgumentResourceV1::Arithmetic)?,
                };
                if descendants[0].kind
                    != fe2o3_kernel_ir::CanonicalKirDefinitionDescendantKindV1::Retained
                    || descendants[0].output != expected
                    || output.definitions()[resources::definition_index(output, expected, budget)?]
                        .ty
                        != definition.ty
                {
                    return resources::binding("execution recipe checked result descendant");
                }
            }
            _ => return resources::binding("execution recipe checked result descendant"),
        }
    }
    for (ordinal, operand) in input.uses()[before.operands.clone()].iter().enumerate() {
        let found = view.operand(operand.coordinate, budget)?;
        budget.charge_work(2)?;
        match (after, found) {
            (None, None) => (),
            (Some(after), Some(found)) => {
                let expected = &output.uses()[after.operands.start + ordinal];
                if found.coordinate != expected.coordinate
                    || found.definition != output.definitions()[expected.definition].coordinate
                {
                    return resources::binding("execution recipe checked operand occurrence");
                }
            }
            _ => return resources::binding("execution recipe checked operand occurrence"),
        }
    }
    Ok(mapped)
}
