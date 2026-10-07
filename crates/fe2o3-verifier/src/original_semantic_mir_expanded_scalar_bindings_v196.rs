//! Scalar endpoints through the checked neutral prefix and actual scalar tail.
//! This is an owner-bound locator, not a value or execution refinement proof.
use super::super::tile_target::TileTargetV176;
use super::{Definition, Error, Resource, Result, SourceSlots, Writer};
use crate::mixed_optimizer_refinement_v26::semantics::{
    congruence_v27::compare_type, definition_index,
};
use fe2o3_kernel_ir::{
    CanonicalKirDefinitionDescendantKindV1 as Descendant,
    CanonicalKirOperationCoordinateV1 as Operation, Type,
};
use fe2o3_lower_mir_kernel::ProductionSourceTileLeafV162 as TileLeaf;
use std::{cmp::Ordering, mem::size_of};

#[path = "original_semantic_mir_expanded_value_relation_v200.rs"]
mod relation;

#[path = "original_semantic_mir_expanded_source_scalar_v203.rs"]
mod source_relation;

#[path = "original_semantic_mir_expanded_source_leaf_v207.rs"]
mod source_leaf;

#[path = "original_semantic_mir_expanded_source_product_v283.rs"]
mod source_product;

#[path = "original_semantic_mir_expanded_scalar_forwarding_v244.rs"]
mod forwarding;

#[path = "original_semantic_mir_scalar_reconstruction_v254.rs"]
mod reconstruction;

pub(in super::super) struct ExpandedScalarBindingsV196<'target, 'slots, 'view, 'source> {
    slots: &'slots SourceSlots<'view, 'source>,
    target: &'target TileTargetV176<'slots, 'view, 'source>,
    original_definitions: usize,
    actual_definitions: usize,
    required: usize,
}

fn mismatch() -> Error {
    Error::Statement("expanded scalar binding differs from its retained source endpoint")
}

impl<'target, 'slots, 'view, 'source> ExpandedScalarBindingsV196<'target, 'slots, 'view, 'source> {
    fn headers() -> usize {
        size_of::<Self>()
            + Error::frame_binding_headers_v284()
            + 2 * size_of::<Result<Self>>()
            + 3 * size_of::<Definition>()
            + size_of::<fe2o3_lower_mir_kernel::ProductionSourceTileOperationSpanV159>()
            + size_of::<TileLeaf>()
            + 2 * size_of::<Result<Option<usize>>>()
            + relation::headers()
            + source_relation::headers()
            + source_leaf::headers()
            + source_product::headers()
            + forwarding::headers()
            + reconstruction::headers()
            + size_of::<Definition>()
            + size_of::<fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1>()
            + size_of::<fe2o3_kernel_ir::ValueId>()
            + size_of::<Option<fe2o3_kernel_ir::ValueId>>()
            + 2 * size_of::<Result<usize>>()
            + size_of::<Result<Option<usize>>>()
            + 4 * size_of::<usize>()
            + 12 * size_of::<&()>()
            + 24 * size_of::<usize>()
    }

    pub(in super::super) fn derive(
        slots: &'slots SourceSlots<'view, 'source>,
        target: &'target TileTargetV176<'slots, 'view, 'source>,
        out: &mut Writer<'_, '_>,
    ) -> Result<Self> {
        slots.with_source_query_v42(out, |out| {
            out.budget.reserve_storage(Self::headers())?;
            out.budget.charge_work(3)?;
            if !std::ptr::eq(slots, target.source_slots(out)?) {
                return Err(mismatch());
            }
            let original = slots.correspondence(out)?.inventory(out.budget)?;
            let actual = target.inventory(out)?;
            let tile = slots.tile_owner_v176(out)?;
            if !std::ptr::eq(tile.output(out.budget)?, actual.owner()) {
                return Err(mismatch());
            }
            Ok(Self {
                slots,
                target,
                original_definitions: original.definitions().len(),
                actual_definitions: actual.definitions().len(),
                required: out.budget.storage(),
            })
        })
    }

    fn check(&self, out: &mut Writer<'_, '_>) -> Result<()> {
        self.slots
            .check_query_storage_floor(self.required, out.budget)?;
        out.budget.charge_work(1)?;
        if !std::ptr::eq(self.slots, self.target.source_slots(out)?) {
            return Err(mismatch());
        }
        Ok(())
    }

    pub(in super::super) fn check_owner(
        &self,
        slots: &SourceSlots<'_, '_>,
        target: &TileTargetV176<'_, '_, '_>,
        out: &mut Writer<'_, '_>,
    ) -> Result<()> {
        self.slots.with_source_query_v42(out, |out| {
            self.check(out)?;
            out.budget.charge_work(2)?;
            if !std::ptr::eq(self.slots, slots) || !std::ptr::eq(self.target, target) {
                return Err(mismatch());
            }
            Ok(())
        })
    }

    pub(in super::super) fn definition_counts(
        &self,
        out: &mut Writer<'_, '_>,
    ) -> Result<(usize, usize)> {
        self.slots.with_source_query_v42(out, |out| {
            self.check(out)?;
            Ok((self.original_definitions, self.actual_definitions))
        })
    }

    pub(in super::super) fn definition(
        &self,
        original: usize,
        out: &mut Writer<'_, '_>,
    ) -> Result<usize> {
        self.slots.with_source_query_v42(out, |out| {
            self.check(out)?;
            let input = self.slots.correspondence(out)?.inventory(out.budget)?;
            out.budget.charge_work(2)?;
            let source = input.definitions().get(original).ok_or_else(mismatch)?;
            if !matches!(
                source.ty,
                Type::Unit | Type::Scalar(_) | Type::Pointer(_) | Type::Slice(_)
            ) {
                return Err(Error::Statement(
                    "expanded scalar binding requires a scalar, pointer, slice or unit endpoint",
                ));
            }
            let tile = self.slots.tile_owner_v176(out)?;
            let neutral = tile.neutral_source_v162(out.budget)?;
            let descendants = neutral.definition_descendants(source.coordinate, out.budget)?;
            out.budget.charge_work(2)?;
            let [descendant] = descendants else {
                return Err(mismatch());
            };
            if descendant.kind != Descendant::Retained {
                return Err(mismatch());
            }
            let predecessor = neutral.output_inventory(out.budget)?;
            out.budget.charge_work(6)?;
            let predecessor_index = definition_index(predecessor, descendant.output)?;
            let predecessor = &predecessor.definitions()[predecessor_index];
            if compare_type(source.ty, predecessor.ty, out)? != Ordering::Equal {
                return Err(mismatch());
            }
            let coordinate = match (source.coordinate, descendant.output) {
                (
                    Definition::FunctionArgument { .. },
                    coordinate @ Definition::FunctionArgument { .. },
                )
                | (
                    Definition::BlockArgument { .. },
                    coordinate @ Definition::BlockArgument { .. },
                ) => coordinate,
                (
                    Definition::Result { operation, result },
                    Definition::Result {
                        operation: neutral_operation,
                        result: neutral_result,
                    },
                ) => {
                    let span = tile
                        .operation_span(operation, out.budget)?
                        .ok_or_else(mismatch)?;
                    out.budget.charge_work(4)?;
                    if span.expansion.input != neutral_operation
                        || span.expansion.end.checked_sub(span.expansion.first) != Some(1)
                        || result != neutral_result
                    {
                        return Err(mismatch());
                    }
                    Definition::Result {
                        operation: Operation {
                            block: span.expansion.input.block,
                            operation: span.expansion.first,
                        },
                        result: neutral_result,
                    }
                }
                _ => return Err(mismatch()),
            };
            // Dense definition indices change when earlier operations expand.
            // Only the checked coordinate locates a value in the actual census.
            let actual = self.target.inventory(out)?;
            out.budget.charge_work(7)?;
            let index = definition_index(actual, coordinate)?;
            let actual = &actual.definitions()[index];
            if actual.value != predecessor.value
                || compare_type(predecessor.ty, actual.ty, out)? != Ordering::Equal
            {
                return Err(mismatch());
            }
            Ok(index)
        })
    }

    /// A checked replacement is a value locator, not a discharged equality law.
    /// The emitted source relation still requires equality at the original cut.
    pub(in super::super) fn source_definition(
        &self,
        original: usize,
        out: &mut Writer<'_, '_>,
    ) -> Result<usize> {
        self.slots.with_source_query_v42(out, |out| {
            self.check(out)?;
            let input = self.slots.correspondence(out)?.inventory(out.budget)?;
            out.budget.charge_work(2)?;
            let source = input.definitions().get(original).ok_or_else(mismatch)?;
            if !matches!(
                source.ty,
                Type::Unit | Type::Scalar(_) | Type::Pointer(_) | Type::Slice(_)
            ) {
                return Err(Error::Statement(
                    "expanded scalar binding requires a scalar, pointer, slice or unit endpoint",
                ));
            }
            let tile = self.slots.tile_owner_v176(out)?;
            let neutral = tile.neutral_source_v162(out.budget)?;
            let descendants = neutral.definition_descendants(source.coordinate, out.budget)?;
            out.budget.charge_work(2)?;
            let [descendant] = descendants else {
                return Err(mismatch());
            };
            if descendant.kind == Descendant::Retained {
                return self.definition(original, out);
            }
            let predecessor = neutral.output_inventory(out.budget)?;
            out.budget.charge_work(3)?;
            let index = definition_index(predecessor, descendant.output)?;
            let predecessor = &predecessor.definitions()[index];
            if compare_type(source.ty, predecessor.ty, out)? != Ordering::Equal {
                return Err(mismatch());
            }
            let function = match predecessor.coordinate {
                Definition::FunctionArgument { function, .. } => function,
                Definition::BlockArgument { block, .. } => block.function,
                Definition::Result { operation, .. } => operation.block.function,
            };
            let value = predecessor.value.ok_or_else(mismatch)?;
            // The authenticated tile tail retains scalar value identities even
            // when expansion changes operation coordinates and dense indices.
            let actual = self.target.inventory(out)?;
            let index = actual
                .definition_index_for_value(function, value, out.budget)?
                .ok_or_else(mismatch)?;
            out.budget.charge_work(2)?;
            let actual = actual.definitions().get(index).ok_or_else(mismatch)?;
            if actual.value != Some(value)
                || compare_type(predecessor.ty, actual.ty, out)? != Ordering::Equal
            {
                return Err(mismatch());
            }
            Ok(index)
        })
    }

    pub(in super::super) fn tile_leaf(
        &self,
        original: usize,
        path: &[u32],
        out: &mut Writer<'_, '_>,
    ) -> Result<Option<usize>> {
        self.slots.with_source_query_v42(out, |out| {
            self.check(out)?;
            let original_inventory = self.slots.correspondence(out)?.inventory(out.budget)?;
            out.budget.charge_work(1)?;
            let original = original_inventory
                .definitions()
                .get(original)
                .ok_or_else(mismatch)?;
            match self.slots.tile_leaf_v162(original.coordinate, path, out)? {
                TileLeaf::Unit => Ok(None),
                TileLeaf::Scalar {
                    function,
                    value,
                    scalar,
                } => {
                    let actual = self.target.inventory(out)?;
                    let index = actual
                        .definition_index_for_value(function, value, out.budget)?
                        .ok_or_else(mismatch)?;
                    out.budget.charge_work(2)?;
                    let row = actual.definitions().get(index).ok_or_else(mismatch)?;
                    if row.value != Some(value) || row.ty != &Type::Scalar(scalar) {
                        return Err(mismatch());
                    }
                    Ok(Some(index))
                }
            }
        })
    }
}

#[cfg(test)]
#[path = "original_semantic_mir_expanded_scalar_bindings_v196_tests.rs"]
mod tests;
