//! One original-anchor census joins retained call results to actual store inputs.

use super::*;
use fe2o3_kernel_ir::{AddressSpace, OperationKind, StorageOperationV1, Type as PhysicalType};
use fe2o3_lower_mir_kernel::ProductionSourceObjectEndpointRoleV39 as EndpointRole;
use fe2o3_mir_model::semantic_mir_v1::SemanticTypeIdV1 as TypeId;
use fe2o3_pliron::{
    ProductionSemanticSsaOccurrenceSiteV1 as Site, ProductionSemanticSsaOperandRoleV1 as Role,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Row {
    key: (usize, usize, u32),
    local: u32,
    ty: TypeId,
    definition: usize,
    operation: usize,
}

pub(super) struct ObjectReturnsV42<'slots, 'view, 'source> {
    slots: &'slots SourceSlots<'view, 'source>,
    rows: Vec<Row>,
    required: usize,
}

impl<'slots, 'view, 'source> ObjectReturnsV42<'slots, 'view, 'source> {
    pub(super) fn derive(
        slots: &'slots SourceSlots<'view, 'source>,
        width: FormalIndexWidth,
        out: &mut Writer<'_, '_>,
    ) -> Result<Self> {
        slots.with_source_query_v42(out, |out| {
            out.budget.reserve_storage(headers())?;
            let relation = slots.correspondence(out)?;
            let source = relation.source(out.budget)?;
            let semantic = source.source_semantic(out.budget)?;
            let inventory = relation.inventory(out.budget)?;
            let roots = source.root_count(out.budget)?;
            let mut count = 0usize;
            for root in 0..roots {
                for instance in 0..source.instance_count(root, out.budget)? {
                    out.budget.charge_work(1)?;
                    count = add(count, source.memory_anchor_count(root, instance, out.budget)?)?;
                }
            }
            let mut rows = vector(count, out)?;
            for root in 0..roots {
                for instance in 0..source.instance_count(root, out.budget)? {
                    let (function, _) = source.instance(root, instance, out.budget)?;
                    let function = semantic.functions().get(function.index() as usize)
                        .ok_or_else(mismatch)?;
                    for anchor in 0..source.memory_anchor_count(root, instance, out.budget)? {
                        out.budget.charge_work(2)?;
                        let Some(recipe) = relation.memory_object_recipe_v39(root, instance, anchor, out.budget)?
                        else { continue };
                        if recipe.endpoint_count(out.budget)? != 1 { continue }
                        let endpoint = recipe.endpoint(0, out.budget)?;
                        if endpoint.role(out.budget)? != EndpointRole::Write { continue }
                        let Some((Site::Terminator { block }, Role::CallDestinationAddress, local, prefix)) =
                            endpoint.original_place(out.budget)? else { continue };
                        let Some(Terminator::Call(call)) = function.blocks().get(block.get() as usize)
                            .map(|block| block.terminator().kind()) else { return Err(mismatch()) };
                        let destination = call.destination().ok_or_else(mismatch)?.place();
                        let root_type = function.locals().get(local.index() as usize)
                            .ok_or_else(mismatch)?.ty();
                        let ty = destination.ty();
                        let scalar = ScalarV30::from_source(semantic.types(), ty)?;
                        if scalar == ScalarV30::Unit
                            || destination.local() != local
                            || prefix as usize != destination.projections().len()
                            || endpoint.source_types(out.budget)? != (root_type, ty)
                            || endpoint.original_projection_count(out.budget)? != destination.projections().len()
                            || !matches!(endpoint.local_identity(out.budget)?,
                                Some((owner, source_local, _)) if owner == instance && source_local == local)
                            || !slots.has_original_object(root, instance, local.index(), out)?
                        { return Err(mismatch()) }
                        for (ordinal, projection) in destination.projections().iter().enumerate() {
                            out.budget.charge_work(1)?;
                            if endpoint.original_projection(ordinal, out.budget)?.0 != *projection {
                                return Err(mismatch());
                            }
                        }
                        let (operation, result) = recipe.physical_operation(out.budget)?;
                        let StorageOperationV1::WriteValue { .. } = operation else {
                            return Err(mismatch());
                        };
                        if result.is_some() { return Err(mismatch()) }
                        let coordinate = recipe.original_operation(out.budget)?;
                        out.budget.charge_work(inventory.operations().len().checked_ilog2().unwrap_or(0) as usize + 2)?;
                        let ordinal = inventory.operations().binary_search_by_key(&coordinate, |row| row.coordinate)
                            .map_err(|_| mismatch())?;
                        let actual = &inventory.operations()[ordinal];
                        if actual.operation.kind != OperationKind::Storage(operation) {
                            return Err(mismatch());
                        }
                        let [address, value] = inventory.uses().get(actual.operands.clone()).ok_or_else(mismatch)?
                        else { return Err(mismatch()) };
                        let PhysicalType::Pointer(pointer) = inventory.definitions().get(address.definition)
                            .ok_or_else(mismatch)?.ty else { return Err(mismatch()) };
                        let (_, layout) = endpoint.storage_layouts(out.budget)?;
                        if pointer.address_space != AddressSpace::Private
                            || pointer.pointee.as_ref() != &PhysicalType::StorageObject(layout)
                            || !aggregate_bindings::scalar_matches(scalar,
                                semantic.types().get(ty.index() as usize).ok_or_else(mismatch)?.rust_type_kind(),
                                inventory.definitions().get(value.definition).ok_or_else(mismatch)?.ty, width)
                        { return Err(mismatch()) }
                        rows.push(Row { key: (root, instance, block.get()), local: local.index(), ty,
                            definition: value.definition, operation: ordinal });
                    }
                }
            }
            out.budget.charge_work(rows.len().checked_mul(rows.len().checked_ilog2().unwrap_or(0) as usize + 2)
                .ok_or(Resource::Arithmetic)?)?;
            rows.sort_unstable_by_key(|row| row.key);
            let mut unique = 0;
            for read in 0..rows.len() {
                out.budget.charge_work(2)?;
                if unique > 0 && rows[unique - 1].key == rows[read].key {
                    if rows[unique - 1] != rows[read] { return Err(mismatch()) }
                } else {
                    rows[unique] = rows[read];
                    unique += 1;
                }
            }
            rows.truncate(unique);
            Ok(Self { slots, rows, required: out.budget.storage() })
        })
    }

    pub(super) fn definition(
        &self,
        root: usize,
        instance: usize,
        block: u32,
        local: u32,
        ty: TypeId,
        out: &mut Writer<'_, '_>,
    ) -> Result<usize> {
        self.slots.with_source_query_v42(out, |out| {
            if out.budget.storage() < self.required {
                return Err(Resource::Accounting.into());
            }
            out.budget
                .charge_work(self.rows.len().checked_ilog2().unwrap_or(0) as usize + 2)?;
            let at = self
                .rows
                .binary_search_by_key(&(root, instance, block), |row| row.key)
                .map_err(|_| mismatch())?;
            let row = self.rows[at];
            if row.local != local || row.ty != ty {
                return Err(mismatch());
            }
            Ok(row.definition)
        })
    }
}

pub(super) fn headers() -> usize {
    fn h<T>() -> usize {
        size_of::<T>() + 2 * size_of::<Result<T>>()
    }
    h::<ObjectReturnsV42<'_, '_, '_>>()
        + h::<Row>()
        + h::<Vec<Row>>()
        + h::<fe2o3_lower_mir_kernel::ProductionSourceObjectRecipeV39<'_, '_>>()
        + h::<fe2o3_lower_mir_kernel::ProductionSourceObjectEndpointV39<'_, '_>>()
        + 24 * size_of::<usize>()
        + 18 * size_of::<&()>()
}

#[cfg(test)]
#[path = "original_semantic_mir_object_returns_v42_tests.rs"]
mod tests;
