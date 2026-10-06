//! Source-owned scalar bindings for the expanded relation. Source invocation
//! frames remain distinct from the target's flattened execution frames.
use super::super::{InvocationPlan, LogicalBinding, ScalarV30, Value, logical};
use super::*;
use fe2o3_kernel_ir::FormalIndexWidth;
use fe2o3_lower_mir_kernel::{
    ProductionSourceSsaCarrierShapeV37 as Carrier, ProductionSourceSsaEndpointV36 as Endpoint,
};
use std::fmt::Write as _;

macro_rules! emit {
    ($out:expr, $($arg:tt)*) => { write!($out, $($arg)*).map_err(|_| $out.error())? };
}

pub(super) fn headers() -> usize {
    size_of::<Endpoint<'_, '_>>()
        + 2 * size_of::<Result<Endpoint<'_, '_>>>()
        + size_of::<Option<(usize, u32)>>()
        + size_of::<Result<()>>()
        + 12 * size_of::<usize>()
        + logical::headers()
}

impl ExpandedScalarBindingsV196<'_, '_, '_, '_> {
    pub(in super::super::super) fn emit_source_conjunct(
        &self,
        plan: &InvocationPlan<'_, '_>,
        root: usize,
        instance: usize,
        value: Value,
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
            let local = endpoint.source_local(out.budget)?.index() as usize;
            let semantic = source.source_semantic(out.budget)?;
            let source_type = endpoint.source_type(out.budget)?;
            out.budget.charge_work(5)?;
            let declaration = semantic.functions().get(row.function.index() as usize)
                .and_then(|function| function.locals().get(local)).ok_or_else(mismatch)?;
            if !row.active || endpoint.source_function(out.budget)? != row.function
                || local >= row.locals.len() || declaration.ty() != source_type
            {
                return Err(mismatch());
            }
            let original = endpoint.original_definition(out.budget)?;
            let actual = match (original, endpoint.physical_type(out.budget)?) {
                (Some(definition), Some(ty)) => {
                    let original_inventory = relation.inventory(out.budget)?;
                    out.budget.charge_work(1)?;
                    if original_inventory.definitions().get(definition).map(|row| row.ty) != Some(ty) {
                        return Err(mismatch());
                    }
                    Some(self.source_definition(definition, out)?)
                }
                (None, None) if endpoint.carrier_shape(out.budget)? == Carrier::Unit
                    && ScalarV30::from_source(semantic.types(), source_type)? == ScalarV30::Unit => None,
                _ => return Err(mismatch()),
            };
            if let Some(actual) = actual {
                let function = self.target.root_function(root, out)?;
                let inventory = self.target.inventory(out)?;
                out.budget.charge_work(2)?;
                if !inventory.functions().get(function.0 as usize)
                    .is_some_and(|row| row.definitions.contains(&actual))
                {
                    return Err(mismatch());
                }
            }
            let logical = LogicalBinding::derive(self.slots, plan, root, &endpoint, out)?;
            let private = match self.slots.legacy_descriptor_by_source(root, instance,
                u32::try_from(local).map_err(|_| Resource::Arithmetic)?, out)? {
                Some((descriptor, _)) => {
                    let bits = ScalarV30::from_source(semantic.types(), source_type)?.width();
                    if bits == 0 || !matches!(logical, LogicalBinding::Plain) {
                        return Err(mismatch());
                    }
                    Some((descriptor, bits))
                }
                None => None,
            };
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
            emit!(out, " && (source.machine.valid && target.valid && {local} < source.machine.values.len() && {frame} < source.machine.frames.active.len() && source.machine.frames.active[{frame}].owner == {owner} && ({{ ");
            if let Some((descriptor, bits)) = private {
                let bytes = if bits == 1 { 1 } else { bits / 8 };
                emit!(out, "source.slots.contains_key({descriptor}) && ({{ let slot = invocation_source_slot_{descriptor}_v36(); let pointer = source.slots[{descriptor}]; (match pointer.allocation {{ MemoryAllocationV30::Private {{ owner, invocation, site, .. }} => owner == source.machine.frames.active[{frame}].owner && invocation == source.machine.frames.active[{frame}].invocation && owner == slot.owner && site == slot.site, _ => false }}) && invocation_source_read_enabled_v36(source.machine, pointer, {bytes}, slot.alignment) && ({{ let original = MemoryValueV30::Scalar(byte_load_v30(source.machine.memory, pointer, {bytes}, invocation_runtime_little_endian_v36())); invocation_source_byte_value_typed_v36(original, {bits}) && ");
                self.emit_actual_relation(actual, width, out)?;
                emit!(out, " }}) }})");
            } else {
                emit!(out, "let original = source.machine.values[{local}]; ");
                self.emit_actual_relation(actual, width, out)?;
                logical.emit_current(local, out)?;
            }
            emit!(out, " }}))");
            Ok(())
        })
    }
}
