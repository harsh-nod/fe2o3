use fe2o3_kernel_analysis::{CanonicalKirInventoryErrorV1 as Error, CanonicalKirInventoryV1};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKirDefinitionCoordinateV1 as Definition, CanonicalKirFunctionCoordinateV1 as Function,
    ValueId, VerifiedCanonicalKernelIrModuleV12, VerifiedCanonicalKernelIrModuleV18,
};

type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
#[path = "production_value_origin_v1_tests.rs"]
mod tests;

use super::origin_worklist_v1::{OriginStateV1, OriginWorkErrorV1, OriginWorkV1};

type Origin = OriginStateV1<Definition>;

fn origin_error(error: OriginWorkErrorV1) -> Error {
    match error {
        OriginWorkErrorV1::Resource(resource) => Error::Resource(resource),
        OriginWorkErrorV1::Shape => Error::InconsistentOwner,
    }
}

/// Solved whole-value transport for exactly one inventory/function.
/// Only the origin table survives preparation; propagation scratch is released.
pub(super) struct WholeValueOriginsV1<'a, O = VerifiedCanonicalKernelIrModuleV12> {
    inventory: &'a CanonicalKirInventoryV1<'a, O>,
    function: Function,
    definitions: std::ops::Range<usize>,
    origins: Vec<Origin>,
}

pub(super) type WholeValueOriginsV18<'a> =
    WholeValueOriginsV1<'a, VerifiedCanonicalKernelIrModuleV18>;

impl<O> WholeValueOriginsV1<'_, O> {
    pub(super) fn belongs_to(
        &self,
        inventory: &CanonicalKirInventoryV1<'_, O>,
        function: Function,
    ) -> bool {
        std::ptr::eq(self.inventory, inventory) && self.function == function
    }

    // Whole-value transport proves an exact definition, not a pointer cast or
    // a source epoch. Generated recipes require an operation result specifically.
    pub(super) fn operation_origin(
        &self,
        value: ValueId,
        budget: &mut Budget<'_>,
    ) -> Result<Option<ValueId>> {
        let Some(Definition::Result { operation, result }) = self.resolve(value, budget)? else {
            return Ok(None);
        };
        budget.charge_work(4)?;
        let function = self.inventory.functions().get(self.function.0 as usize)
            .filter(|row| row.coordinate == self.function && operation.block.function == self.function)
            .ok_or(Error::InconsistentOwner)?;
        let block = function.blocks.start.checked_add(operation.block.block as usize)
            .filter(|index| *index < function.blocks.end)
            .and_then(|index| self.inventory.blocks().get(index))
            .filter(|row| row.coordinate == operation.block)
            .ok_or(Error::InconsistentOwner)?;
        let operation_row = block.operations.start.checked_add(operation.operation as usize)
            .filter(|index| *index < block.operations.end)
            .and_then(|index| self.inventory.operations().get(index))
            .filter(|row| row.coordinate == operation)
            .ok_or(Error::InconsistentOwner)?;
        Ok(Some(operation_row.operation.results.get(result as usize)
            .ok_or(Error::InconsistentOwner)?.id))
    }

    pub(super) fn resolve(
        &self,
        value: ValueId,
        budget: &mut Budget<'_>,
    ) -> Result<Option<Definition>> {
        let index = self
            .inventory
            .definition_index_for_value(self.function, value, budget)?
            .filter(|index| self.definitions.contains(index))
            .ok_or(Error::InconsistentOwner)?;
        budget.charge_work(1)?;
        match self.origins.get(index - self.definitions.start) {
            Some(Origin::Exact(definition)) => Ok(Some(*definition)),
            Some(Origin::Unknown) => Ok(None),
            Some(Origin::Pending) | None => Err(Error::InconsistentOwner),
        }
    }

    pub(super) fn value_origin(&self, value: ValueId, budget: &mut Budget<'_>) -> Result<Option<ValueId>> {
        match self.resolve(value, budget)? {
            Some(Definition::Result { .. }) => self.operation_origin(value, budget),
            Some(Definition::FunctionArgument { function, argument }) => {
                budget.charge_work(3)?;
                let row = self.inventory.functions().get(function.0 as usize)
                    .filter(|row| row.coordinate == function && function == self.function)
                    .ok_or(Error::InconsistentOwner)?;
                Ok(Some(*row.function.body.as_ref().and_then(|body| body.parameters.get(argument as usize))
                    .ok_or(Error::InconsistentOwner)?))
            }
            Some(Definition::BlockArgument { .. }) => Err(Error::InconsistentOwner),
            None => Ok(None),
        }
    }
}

/// Prepares whole-value block transport, never casts, aliasing or bounds.
/// Every syntactic incoming edge contributes, including unreachable edges.
/// Requested scratch payload is O(function definitions + edge arguments).
/// Ordinary Result returns restore the ledger floor; no unwind/RSS bound is claimed.
pub(super) fn with_whole_value_origins_v1<'a, R>(
    inventory: &'a CanonicalKirInventoryV1<'a>,
    expected_owner: &VerifiedCanonicalKernelIrModuleV12,
    function: Function,
    budget: &mut Budget<'_>,
    consume: impl FnOnce(&WholeValueOriginsV1<'a>, &mut Budget<'_>) -> R,
) -> Result<R> {
    let floor = budget.storage();
    let result = (|| {
        let origins = prepare_inner(inventory, expected_owner, function, budget)?;
        let retained = origins
            .origins
            .len()
            .checked_mul(std::mem::size_of::<Origin>())
            .ok_or(Resource::Arithmetic)?;
        let scratch = budget
            .storage()
            .checked_sub(floor)
            .and_then(|bytes| bytes.checked_sub(retained))
            .ok_or(Resource::Accounting)?;
        budget.release_storage(scratch)?;
        Ok(consume(&origins, budget))
    })();
    let release = budget
        .storage()
        .checked_sub(floor)
        .ok_or(Resource::Accounting)?;
    budget.release_storage(release)?;
    result
}

// The V18 consumer borrows the same inventory and propagation engine. Its
// custody is nested in the real source attempt, so an origin-table floor loss
// vetoes every containing refund before the borrowed table drops.
pub(super) fn with_whole_value_origins_v18<'a, 'work, R, E>(
    relation: &'a super::ProductionSourceCorrespondenceV18<'a>,
    function: Function,
    budget: &mut Budget<'work>,
    consume: impl FnOnce(&WholeValueOriginsV18<'a>, &mut Budget<'work>) -> std::result::Result<R, E>,
) -> std::result::Result<R, E>
where E: From<super::ProductionSourceOwnedViewErrorV18> {
    with_source_inventory_origins_v18(
        relation,
        relation.inventory,
        &relation.source.owner.inner.pending.graph,
        function,
        budget,
        consume,
    )
}

pub(super) fn with_optimized_whole_value_origins_v18<'a, 'work, R, E>(
    relation: &'a super::ProductionSourceCorrespondenceV18<'a>,
    optimized: &'a super::ProductionOptimizedSourceCorrespondenceV18<'a>,
    inventory: &'a fe2o3_kernel_analysis::CanonicalKirInventoryV18<'a>,
    function: Function,
    budget: &mut Budget<'work>,
    consume: impl FnOnce(&WholeValueOriginsV18<'a>, &mut Budget<'work>) -> std::result::Result<R, E>,
) -> std::result::Result<R, E>
where E: From<super::ProductionSourceOwnedViewErrorV18> {
    relation.query(budget)?;
    budget.charge_work(3).map_err(super::ProductionSourceOwnedViewErrorV18::from)?;
    if !std::ptr::eq(relation.inventory, optimized.input_inventory(budget)?)
        || !std::ptr::eq(relation.source, optimized.original_source(budget)?)
        || !std::ptr::eq(inventory, optimized.output_inventory(budget)?)
    {
        return relation.source.missing("optimized whole-value endpoint association").map_err(Into::into);
    }
    with_source_inventory_origins_v18(
        relation, inventory, inventory.owner(), function, budget, consume,
    )
}

fn with_source_inventory_origins_v18<'a, 'work, R, E>(
    relation: &'a super::ProductionSourceCorrespondenceV18<'a>,
    inventory: &'a fe2o3_kernel_analysis::CanonicalKirInventoryV18<'a>,
    expected_owner: &VerifiedCanonicalKernelIrModuleV18,
    function: Function,
    budget: &mut Budget<'work>,
    consume: impl FnOnce(&WholeValueOriginsV18<'a>, &mut Budget<'work>) -> std::result::Result<R, E>,
) -> std::result::Result<R, E>
where E: From<super::ProductionSourceOwnedViewErrorV18> {
    use super::ProductionSourceOwnedViewErrorV18 as ViewError;
    let query_error = |error| match error {
        Error::Resource(error) => ViewError::Resource(error),
        Error::InconsistentOwner => ViewError::Binding("whole-value inventory association"),
    };
    relation.query(budget)?;
    let floor = budget.storage();
    let (origins, storage) = super::scoped_source_attempt_v29(relation.source.cleanup, budget, floor, |budget| {
        let headers = super::argument_sum_v1(&[
            std::mem::size_of::<WholeValueOriginsV18<'_>>(),
            std::mem::size_of::<std::thread::Result<std::result::Result<R, E>>>(),
        ])?;
        budget.reserve_storage(headers)?;
        let origins = relation.retain_query(
            prepare_inner(
                inventory,
                expected_owner,
                function,
                budget,
            )
            .map_err(query_error),
        )?;
        let retained =
            super::argument_product_v1(origins.origins.len(), std::mem::size_of::<Origin>())?;
        let storage = super::argument_sum_v1(&[headers, retained])?;
        let scratch = budget
            .storage()
            .checked_sub(floor)
            .and_then(|live| live.checked_sub(storage))
            .ok_or(Resource::Accounting)?;
        budget.release_storage(scratch)?;
        Ok::<_, ViewError>((origins, storage))
    })?;
        let slot = std::ptr::from_ref(budget) as usize;
        let ledger = budget.work_ledger_identity_v1();
        let live_floor = budget.storage();
        let caught =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| consume(&origins, budget)));
        let prior = relation.source.guard.first.get();
        let invalid = slot != std::ptr::from_ref(budget) as usize
            || ledger != budget.work_ledger_identity_v1()
            || budget.storage() < live_floor;
        if invalid {
            relation.source.cleanup.deny_refund();
        }
        let postflight = if invalid {
            Err(ViewError::Resource(Resource::Accounting))
        } else if matches!(&caught, Ok(Ok(_))) {
            relation.check(budget)
        } else {
            relation.observe_custody(budget)
        };
        drop(origins);
        super::source_owned_finish_callback_v18(caught, prior, postflight, relation.source.cleanup, budget, storage)
}

fn prepare_inner<'a, O>(
    inventory: &'a CanonicalKirInventoryV1<'a, O>,
    expected_owner: &O,
    function: Function,
    budget: &mut Budget<'_>,
) -> Result<WholeValueOriginsV1<'a, O>> {
    budget.charge_work(3)?;
    if !inventory.belongs_to(expected_owner) {
        return Err(Error::InconsistentOwner);
    }
    let function_row = inventory
        .functions()
        .get(function.0 as usize)
        .filter(|row| row.coordinate == function && row.function.body.is_some())
        .ok_or(Error::InconsistentOwner)?;
    let range = function_row.definitions.clone();
    let definitions = inventory
        .definitions()
        .get(range.clone())
        .ok_or(Error::InconsistentOwner)?;
    let edges = inventory
        .edge_arguments()
        .get(function_row.edge_arguments.clone())
        .ok_or(Error::InconsistentOwner)?;
    let mut work =
        OriginWorkV1::new(definitions.len(), edges.len(), budget).map_err(origin_error)?;
    for definition in definitions {
        let (actual_function, origin) = match definition.coordinate {
            Definition::FunctionArgument { function, .. } => {
                (function, Origin::Exact(definition.coordinate))
            }
            Definition::BlockArgument { block, .. } => (
                block.function,
                if block.block == 0 {
                    Origin::Unknown
                } else {
                    Origin::Pending
                },
            ),
            Definition::Result { operation, .. } => (
                operation.block.function,
                Origin::Exact(definition.coordinate),
            ),
        };
        if actual_function != function {
            return Err(Error::InconsistentOwner);
        }
        work.seed_next(origin, budget).map_err(origin_error)?;
    }
    for edge in edges {
        if !range.contains(&edge.incoming_definition) || !range.contains(&edge.target_definition) {
            return Err(Error::InconsistentOwner);
        }
        let source = edge.incoming_definition - range.start;
        let target = edge.target_definition - range.start;
        if !matches!(
            definitions[target].coordinate,
            Definition::BlockArgument { block, .. } if block.function == function
        ) {
            return Err(Error::InconsistentOwner);
        }
        work.add_link(source, target, budget)
            .map_err(origin_error)?;
    }
    let origins = work.solve(budget).map_err(origin_error)?;
    Ok(WholeValueOriginsV1 {
        inventory,
        function,
        definitions: range,
        origins,
    })
}
