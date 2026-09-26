//! Rich immutable source tables on the original ledger. This remains a lexical
//! preparation view, not final capability readiness or a ranked recipe.
use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1;

struct RichPreparedSourceV1 {
    dense: PreparedSourceV1,
    retained: RetainedPreparationTablesV1,
}

/// Immutable same-source preparation tables. No references or owned tables may
/// escape the higher-ranked callback, and no normal/retained authority is made.
pub(in crate::production_ranked_projection_v1) struct RichNominalSourceTablesV1<'a> {
    function: &'a SemanticFunctionDeclV1,
    prepared: &'a RichPreparedSourceV1,
}
impl RichNominalSourceTablesV1<'_> {
    pub(in crate::production_ranked_projection_v1) fn function(&self) -> &SemanticFunctionDeclV1 {
        self.function
    }
    pub(in crate::production_ranked_projection_v1) fn dense_inputs(
        &self,
    ) -> NominalCapabilityInputsV1<'_> {
        NominalCapabilityInputsV1::from_borrowed_source_v1(
            self.function,
            self.enum_payload_dominance(),
            self.allocations(),
            self.constants(),
        )
    }
    pub(in crate::production_ranked_projection_v1) fn scalar_counts(&self) -> &[u8] {
        &self.prepared.retained.scalar.counts
    }
    pub(in crate::production_ranked_projection_v1) fn scalar_blocks(&self) -> &[Vec<usize>] {
        &self.prepared.retained.scalar.blocks
    }
    pub(in crate::production_ranked_projection_v1) fn scalar_assignments(
        &self,
    ) -> &[Option<ScalarAssignmentSiteV1>] {
        &self.prepared.retained.scalar.assignments
    }
    pub(in crate::production_ranked_projection_v1) fn address_escaped(&self) -> &[bool] {
        &self.prepared.retained.scalar.address_escaped
    }
    pub(in crate::production_ranked_projection_v1) fn stable_argument_origins(
        &self,
    ) -> &[Option<u32>] {
        &self.prepared.retained.provenance.stable_argument_origins
    }
    pub(in crate::production_ranked_projection_v1) fn allocation_origins(&self) -> &[Option<u32>] {
        &self.prepared.retained.provenance.allocation_origins
    }
    pub(in crate::production_ranked_projection_v1) fn allocation_provenance(
        &self,
    ) -> &[Option<LocalAllocationProvenanceV1>] {
        &self.prepared.retained.provenance.allocation_provenance
    }
    pub(in crate::production_ranked_projection_v1) fn enum_payload_dominance(
        &self,
    ) -> &SemanticEnumPayloadDominanceV1 {
        &self.prepared.dense.dominance
    }
    pub(in crate::production_ranked_projection_v1) fn allocations(
        &self,
    ) -> &[Option<AllocationContractV1>] {
        &self.prepared.dense.allocations
    }
    pub(in crate::production_ranked_projection_v1) fn constants(&self) -> &[Option<u64>] {
        &self.prepared.dense.constants
    }
}

#[derive(Clone, Copy)]
struct Custody {
    slot: usize,
    ledger: CanonicalKernelIrWorkLedgerIdentityV1,
    floor: usize,
    work: usize,
    peak: usize,
}
impl Custody {
    fn new(budget: &Budget<'_>) -> Result<Self> {
        if budget.failed_work().is_some() || budget.failed_storage().is_some() {
            return Err(Resource::Accounting.into());
        }
        Ok(Self {
            slot: budget as *const Budget<'_> as usize,
            ledger: budget.work_ledger_identity_v1(),
            floor: budget.storage(),
            work: budget.work(),
            peak: budget.peak_storage(),
        })
    }
    fn check(self, budget: &Budget<'_>, owned: usize) -> Result<()> {
        let floor = self.floor.checked_add(owned).ok_or(Resource::Arithmetic)?;
        if self.slot != budget as *const Budget<'_> as usize
            || self.ledger != budget.work_ledger_identity_v1()
            || budget.storage() < floor
            || budget.work() < self.work
            || budget.peak_storage() < self.peak
        {
            return Err(Resource::Accounting.into());
        }
        Ok(())
    }
}
fn rich_header<R>(callback_bytes: usize) -> Result<usize> {
    let mut bytes = 4096usize;
    for amount in [
        size_of::<RichPreparedSourceV1>(),
        size_of::<Option<RetainedPreparationTablesV1>>(),
        size_of::<RichNominalSourceTablesV1<'static>>(),
        size_of::<PreparationResourcesV1<'static, 'static>>(),
        size_of::<ModelMeter<'static, 'static, 'static>>(),
        size_of::<Custody>(),
        callback_bytes.checked_mul(2).ok_or(Resource::Arithmetic)?,
        size_of::<Result<R>>()
            .checked_mul(2)
            .ok_or(Resource::Arithmetic)?,
    ] {
        bytes = bytes.checked_add(amount).ok_or(Resource::Arithmetic)?;
    }
    Ok(bytes)
}
fn with_rich_scope<'w, R, F>(
    callables: &[SemanticCallableDeclV1],
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    budget: &mut Budget<'w>,
    inspect: F,
) -> Result<R>
where
    R: Copy + 'static,
    F: for<'s> FnOnce(&RichNominalSourceTablesV1<'s>, &mut Budget<'w>) -> Result<R>,
{
    // Checked before any allocation in this factory, including large callbacks
    // and Copy return payloads. N1's independent query has already completed its
    // entry-floor check before this private scope can reserve anything.
    let frame = rich_header::<R>(size_of::<F>())?;
    let before = Custody::new(budget)?;
    budget.charge_work(32)?;
    let mut owned = 0usize;
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        let prepared = {
            let mut retained = None;
            let dense = {
                let mut resources = PreparationResourcesV1::new(budget, &mut owned);
                resources.reserve_storage(frame).map_err(query_error)?;
                prepare_with_retained(
                    callables,
                    types,
                    function,
                    &mut resources,
                    Some(&mut retained),
                )
                .map_err(query_error)?
            };
            let retained = retained.ok_or(QueryError::Unavailable(
                "rich preparation core did not retain its source tables",
            ))?;
            RichPreparedSourceV1 { dense, retained }
        };
        let view = RichNominalSourceTablesV1 {
            function,
            prepared: &prepared,
        };
        let result = inspect(&view, budget);
        drop(prepared);
        result
    }));
    // All partial vectors and the complete prepared owner are gone here.
    let result = match outcome {
        Ok(Ok(_)) if budget.failed_work().is_some() || budget.failed_storage().is_some() => {
            Err(Resource::Accounting.into())
        }
        Ok(result) => result,
        Err(payload) => {
            drop(payload);
            Err(QueryError::CallbackPanicked)
        }
    };
    before.check(budget, owned)?;
    // Refund only this scope's accepted charges. Surplus and denial history stay.
    budget.release_storage(owned)?;
    result
}

#[allow(clippy::too_many_arguments)]
pub(in crate::production_ranked_projection_v1) fn with_nominal_rich_source_preparation_v1<
    'w,
    R,
    F,
>(
    owner: &ProductionPreRankedKirOwnerV1,
    inventory: &CanonicalKirInventoryV1<'_>,
    root: SemanticFunctionIdV1,
    caller: SemanticFunctionIdV1,
    block: SemanticBlockIdV1,
    call: &SemanticDirectCallV1,
    budget: &mut Budget<'w>,
    inspect: F,
) -> Result<R>
where
    R: Copy + 'static,
    F: for<'s> FnOnce(&RichNominalSourceTablesV1<'s>, &mut Budget<'w>) -> Result<R>,
{
    owner.with_bf16_nominal_entry_resources_v1(inventory, budget, move |budget| {
        // Arithmetic is checked before invoking any nested allocating query; no
        // reservation may mask an insufficient original owner floor.
        let _ = rich_header::<R>(size_of::<F>())?;
        owner.with_checked_bf16_nominal_call_v1(
            inventory,
            root,
            caller,
            block,
            call,
            budget,
            |checked, budget| {
                let source_owner = checked.emission().owner();
                if !std::ptr::eq(source_owner, owner) || !checked.belongs_to(inventory) {
                    return Err(QueryError::Unavailable(
                        "rich prepared source owner/inventory differs",
                    ));
                }
                let source = source_owner.semantic_ssa().source_semantic();
                let function = source.functions().get(caller.index() as usize).ok_or(
                    QueryError::Unavailable("rich prepared caller absent from source"),
                )?;
                if source.functions().len() != 2
                    || source.types().len() > 4096
                    || source.callables().len() > 4096
                    || function.blocks().len() > 32
                    || function.locals().len() > 4096
                {
                    return Err(QueryError::Unavailable(
                        "rich prepared source exceeds closed owner profile",
                    ));
                }
                with_rich_scope(
                    source.callables(),
                    source.types(),
                    function,
                    budget,
                    inspect,
                )
            },
        )
    })
}

/// Synthetic resource/component seam ONLY, never a source-authority constructor.
#[cfg(test)]
pub(in crate::production_ranked_projection_v1) fn with_rich_tables_for_test_v1<'w, R, F>(
    callables: &[SemanticCallableDeclV1],
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    budget: &mut Budget<'w>,
    inspect: F,
) -> Result<R>
where
    R: Copy + 'static,
    F: for<'s> FnOnce(&RichNominalSourceTablesV1<'s>, &mut Budget<'w>) -> Result<R>,
{
    with_rich_scope(callables, types, function, budget, inspect)
}
#[cfg(test)]
pub(in crate::production_ranked_projection_v1) fn rich_frame_for_test_v1<R>(
    callback_bytes: usize,
) -> Result<usize> {
    rich_header::<R>(callback_bytes)
}

/// Unwired genuine-source seam for root qualification. Both routes execute the
/// same actual source on the original meter; this makes no numerical/ranked claim.
#[cfg(test)]
#[allow(clippy::too_many_arguments)]
pub(in crate::production_ranked_projection_v1) fn observe_rich_source_comparison_for_test_v1(
    owner: &ProductionPreRankedKirOwnerV1,
    inventory: &CanonicalKirInventoryV1<'_>,
    root: SemanticFunctionIdV1,
    caller: SemanticFunctionIdV1,
    block: SemanticBlockIdV1,
    call: &SemanticDirectCallV1,
    budget: &mut Budget<'_>,
) -> Result<()> {
    with_nominal_rich_source_preparation_v1(
        owner,
        inventory,
        root,
        caller,
        block,
        call,
        budget,
        |rich, budget| {
            let source = owner.semantic_ssa().source_semantic();
            let before = Custody::new(budget)?;
            let mut owned = 0usize;
            let result = catch_unwind(AssertUnwindSafe(|| {
                let old = {
                    let mut resources = PreparationResourcesV1::new(budget, &mut owned);
                    resources
                        .reserve_storage(header::<()>()?)
                        .map_err(query_error)?;
                    prepare(
                        source.callables(),
                        source.types(),
                        rich.function(),
                        &mut resources,
                    )
                    .map_err(query_error)?
                };
                // The dominance equality visits bounded owned tables; this test
                // pays an independently conservative source-sized comparison bound.
                let locals = rich.function().locals().len();
                // The analyzer's own charged initialization/traversal counter
                // bounds all initialized equality-table entries, including
                // nested variant rows. Do not assume a blocks-squared bound.
                let comparisons = old
                    .dominance
                    .work_units()
                    .checked_add(rich.enum_payload_dominance().work_units())
                    .and_then(|n| n.checked_mul(4))
                    .and_then(|n| locals.checked_mul(8).and_then(|l| n.checked_add(l)))
                    .and_then(|n| n.checked_add(64))
                    .ok_or(Resource::Arithmetic)?;
                budget.charge_work(comparisons)?;
                let matches = old.dominance == *rich.enum_payload_dominance()
                    && old.allocations == rich.allocations()
                    && old.constants == rich.constants();
                drop(old);
                if !matches {
                    return Err(QueryError::Unavailable(
                        "rich/existing source tables differ",
                    ));
                }
                Ok(())
            }));
            let result = match result {
                Ok(Ok(_))
                    if budget.failed_work().is_some() || budget.failed_storage().is_some() =>
                {
                    Err(Resource::Accounting.into())
                }
                Ok(result) => result,
                Err(payload) => {
                    drop(payload);
                    Err(QueryError::CallbackPanicked)
                }
            };
            before.check(budget, owned)?;
            budget.release_storage(owned)?;
            result
        },
    )
}

#[cfg(test)]
pub(in crate::production_ranked_projection_v1) fn measure_preparation_core_for_test_v1(
    callables: &[SemanticCallableDeclV1],
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    retain: bool,
    budget: &mut Budget<'_>,
) -> Result<(usize, usize)> {
    let before = Custody::new(budget)?;
    let frame = rich_header::<(usize, usize)>(0)?;
    let mut owned = 0;
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        let mut retained = None;
        let prepared = {
            let mut resources = PreparationResourcesV1::new(budget, &mut owned);
            resources.reserve_storage(frame).map_err(query_error)?;
            if retain {
                prepare_with_retained(
                    callables,
                    types,
                    function,
                    &mut resources,
                    Some(&mut retained),
                )
            } else {
                prepare(callables, types, function, &mut resources)
            }
            .map_err(query_error)?
        };
        assert_eq!(retained.is_some(), retain);
        drop(prepared);
        drop(retained);
        Ok((budget.work() - before.work, owned - frame))
    }));
    let result = match outcome {
        Ok(result) => result,
        Err(payload) => {
            drop(payload);
            Err(QueryError::CallbackPanicked)
        }
    };
    before.check(budget, owned)?;
    budget.release_storage(owned)?;
    result
}
