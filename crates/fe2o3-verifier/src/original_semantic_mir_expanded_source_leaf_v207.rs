//! Source field paths select actual expanded values. This emits a necessary
//! value/currentness conjunct, not control, effect or execution-history proof.
use super::super::{InvocationPlan, ScalarV30, Value, aggregate_bindings::scalar_matches};
use super::*;
use fe2o3_kernel_ir::{ExecutionRoleV15, FormalIndexWidth};
use fe2o3_lower_mir_kernel::{
    ProductionSourceSsaCarrierShapeV37 as Carrier, ProductionSourceSsaEndpointV36 as Endpoint,
};
use fe2o3_mir_model::semantic_mir_v1::{SemanticExecutionRoleV29, SemanticRustTypeKindV1};
use std::fmt::Write as _;

macro_rules! emit {
    ($out:expr, $($arg:tt)*) => { write!($out, $($arg)*).map_err(|_| $out.error())? };
}

pub(super) fn headers() -> usize {
    2 * size_of::<Endpoint<'_, '_>>()
        + 2 * size_of::<Result<Endpoint<'_, '_>>>()
        + size_of::<super::super::super::slots::SourceAggregateLeafV42<'_, '_, '_>>()
        + 2 * size_of::<Result<Option<usize>>>()
        + size_of::<Result<()>>()
        + 14 * size_of::<usize>()
}

impl ExpandedScalarBindingsV196<'_, '_, '_, '_> {
    pub(in super::super::super) fn emit_source_leaf_conjunct(
        &self,
        plan: &InvocationPlan<'_, '_>,
        root: usize,
        instance: usize,
        value: Value,
        ordinal: usize,
        width: FormalIndexWidth,
        out: &mut Writer<'_, '_>,
    ) -> Result<()> {
        self.slots.with_source_query_v42(out, |out| {
            self.check(out)?;
            let relation = self.slots.correspondence(out)?;
            let source = relation.source(out.budget)?;
            out.budget.charge_work(2)?;
            if !std::ptr::eq(source, plan.source(out)?) || width == FormalIndexWidth::Unknown {
                return Err(mismatch());
            }
            let row = plan.instance(root, instance, out)?;
            let endpoint = relation.ssa_typed_endpoint_v36(root, instance, value, out.budget)?;
            let source_type = endpoint.source_type(out.budget)?;
            let local = endpoint.source_local(out.budget)?.index() as usize;
            let semantic = source.source_semantic(out.budget)?;
            out.budget.charge_work(5)?;
            let declaration = semantic.functions().get(row.function.index() as usize)
                .and_then(|function| function.locals().get(local)).ok_or_else(mismatch)?;
            if !row.active || local >= row.locals.len() || declaration.ty() != source_type
                || endpoint.source_function(out.budget)? != row.function
                || self.slots.has_original_object(root, instance,
                    u32::try_from(local).map_err(|_| Resource::Arithmetic)?, out)?
            {
                return Err(mismatch());
            }
            let leaf = self.slots.aggregate_leaf(source_type, ordinal, out)?;
            let path = leaf.path(out)?;
            let scalar = leaf.scalar(out)?;
            let leaf_type = leaf.source_type(out)?;
            out.budget.charge_work(1)?;
            let nominal = semantic.types().get(leaf_type.index() as usize)
                .ok_or_else(mismatch)?.rust_type_kind();
            out.budget.charge_work(1)?;
            let execution = matches!(semantic.types().get(source_type.index() as usize)
                .ok_or_else(mismatch)?.rust_type_kind(), SemanticRustTypeKindV1::Execution(
                    SemanticExecutionRoleV29::MaskedTileU32 { .. }
                    | SemanticExecutionRoleV29::LaneFragmentU32 { .. }
                ));
            let actual = if execution {
                // Borrowed execution carriers require the separate loan relation.
                if endpoint.execution_borrow_v163(out.budget)?.is_some() {
                    return Err(mismatch());
                }
                if !matches!(endpoint.physical_type(out.budget)?, Some(Type::Execution(
                    ExecutionRoleV15::MaskedTileU32 { .. }
                    | ExecutionRoleV15::LaneFragmentU32 { .. }
                ))) {
                    return Err(mismatch());
                }
                let original = endpoint.original_definition(out.budget)?.ok_or_else(mismatch)?;
                self.tile_leaf(original, path, out)?
            } else {
                if !matches!(endpoint.carrier_shape(out.budget)?, Carrier::Aggregate { .. }) {
                    return Err(mismatch());
                }
                let mut component = endpoint;
                for field in path {
                    out.budget.charge_work(1)?;
                    component = component.component(*field as usize, out.budget)?;
                }
                out.budget.charge_work(1)?;
                if component.source_type(out.budget)? != leaf_type {
                    return Err(mismatch());
                }
                match component.carrier_shape(out.budget)? {
                    Carrier::Unit | Carrier::Aggregate { components: 0 }
                        if scalar == ScalarV30::Unit => None,
                    Carrier::Value => {
                        let original = component.original_definition(out.budget)?.ok_or_else(mismatch)?;
                        let ty = component.physical_type(out.budget)?.ok_or_else(mismatch)?;
                        let original_inventory = relation.inventory(out.budget)?;
                        out.budget.charge_work(1)?;
                        if original_inventory.definitions().get(original).map(|row| row.ty) != Some(ty)
                            || !scalar_matches(scalar, nominal, ty, width)
                        {
                            return Err(mismatch());
                        }
                        Some(self.definition(original, out)?)
                    }
                    _ => return Err(mismatch()),
                }
            };
            let inventory = self.target.inventory(out)?;
            let function = self.target.root_function(root, out)?;
            out.budget.charge_work(3)?;
            match actual {
                Some(actual) if inventory.functions().get(function.0 as usize)
                    .is_some_and(|row| row.definitions.contains(&actual))
                    && inventory.definitions().get(actual)
                        .is_some_and(|row| scalar_matches(scalar, nominal, row.ty, width)) => (),
                None if scalar == ScalarV30::Unit => (),
                _ => return Err(mismatch()),
            }
            let (mut ancestor, mut frame) = (instance, 0usize);
            loop {
                out.budget.charge_work(2)?;
                match plan.instance(root, ancestor, out)?.incoming {
                    Some((parent, _)) if parent < ancestor => {
                        ancestor = parent;
                        frame = frame.checked_add(1).ok_or(Resource::Arithmetic)?;
                    }
                    None if ancestor == 0 => break,
                    _ => return Err(mismatch()),
                }
            }
            let local = row.locals.start.checked_add(local).ok_or(Resource::Arithmetic)?;
            let owner = row.function.index();
            emit!(out, " && (source.machine.valid && target.valid && {frame} < source.machine.frames.active.len() && source.machine.frames.active[{frame}].owner == {owner}");
            if execution {
                emit!(out, " && invocation_source_execution_aggregate_current_v170(source, {local})");
            }
            emit!(out, " && (match invocation_source_aggregate_leaf_v42(source, {local}, {}, seq![", source_type.index());
            for field in path {
                out.budget.charge_work(1)?;
                emit!(out, "{field}int,");
            }
            emit!(out, "]) {{ Some(original) => {{ ");
            self.emit_actual_relation(actual, width, out)?;
            emit!(out, " }}, None => false }}))");
            Ok(())
        })
    }
}
