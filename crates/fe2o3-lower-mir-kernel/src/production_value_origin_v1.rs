use fe2o3_kernel_analysis::{CanonicalKirInventoryErrorV1 as Error, CanonicalKirInventoryV1};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKirDefinitionCoordinateV1 as Definition, CanonicalKirFunctionCoordinateV1 as Function,
    ValueId, VerifiedCanonicalKernelIrModuleV12,
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
pub(super) struct WholeValueOriginsV1<'a> {
    inventory: &'a CanonicalKirInventoryV1<'a>,
    function: Function,
    definitions: std::ops::Range<usize>,
    origins: Vec<Origin>,
}

impl WholeValueOriginsV1<'_> {
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

fn prepare_inner<'a>(
    inventory: &'a CanonicalKirInventoryV1<'a>,
    expected_owner: &VerifiedCanonicalKernelIrModuleV12,
    function: Function,
    budget: &mut Budget<'_>,
) -> Result<WholeValueOriginsV1<'a>> {
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
