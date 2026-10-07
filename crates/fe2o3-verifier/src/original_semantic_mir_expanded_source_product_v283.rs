//! Necessary bindings for demanded, owner-bound Product carrier atoms.
//! Opaque execution/descriptor atoms need their own snapshot relation; they are
//! not reinterpreted as scalar leaves or assigned the Product holder's origin.
use super::super::super::slots::{ProductAtomV282 as Atom, SourceProductComponentV282};
use super::super::{InvocationPlan, ScalarV30, Value, aggregate_bindings::scalar_matches};
use super::*;
use fe2o3_kernel_ir::FormalIndexWidth;
use fe2o3_lower_mir_kernel::{
    ProductionSourceSsaCarrierShapeV37 as Carrier, ProductionSourceSsaEndpointV36 as Endpoint,
};
use fe2o3_mir_model::semantic_mir_v1::{SemanticRustTypeKindV1, SemanticTypeIdV1};
use std::fmt::Write as _;

macro_rules! emit {
    ($out:expr, $($arg:tt)*) => { write!($out, $($arg)*).map_err(|_| $out.error())? };
}

pub(super) fn headers() -> usize {
    fn h<T>() -> usize {
        size_of::<T>() + 2 * size_of::<Result<T>>()
    }
    h::<Endpoint<'_, '_>>() * 2
        + h::<SourceProductComponentV282<'_, '_, '_>>()
        + h::<Atom>()
        + h::<Carrier>()
        + h::<Option<usize>>()
        + h::<&[u32]>()
        + h::<std::slice::Iter<'_, u32>>()
        + h::<Value>()
        + h::<FormalIndexWidth>()
        + h::<SemanticRustTypeKindV1>()
        + h::<SemanticTypeIdV1>()
        + h::<Option<&Type>>()
        + h::<()>()
        + 18 * size_of::<usize>()
        // Owner/source/row borrows and the query closure's captured frame.
        + 20 * size_of::<&()>()
}

impl ExpandedScalarBindingsV196<'_, '_, '_, '_> {
    pub(in super::super::super) fn emit_source_product_conjunct_v283(
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
            let declaration = semantic.functions().get(row.function.index() as usize)
                .and_then(|function| function.locals().get(local)).ok_or_else(mismatch)?;
            out.budget.charge_work(5)?;
            if !row.active || local >= row.locals.len() || declaration.ty() != source_type
                || endpoint.source_function(out.budget)? != row.function
                || !self.slots.is_product_v282(source_type, out)?
                || self.slots.has_original_object(root, instance,
                    u32::try_from(local).map_err(|_| Resource::Arithmetic)?, out)?
            {
                return Err(mismatch());
            }
            let atom = self.slots.product_component_v282(source_type, ordinal, out)?;
            let path = atom.path(out)?;
            let atom_type = atom.source_type(out)?;
            let kind = atom.atom(out)?;
            if !matches!(kind, Atom::Scalar(_) | Atom::Pointer { .. } | Atom::Slice { .. }) {
                return Err(Error::Statement(
                    "expanded Product frame atom requires its opaque snapshot relation",
                ));
            }
            let mut component = endpoint;
            for &field in path {
                out.budget.charge_work(1)?;
                component = component.component(field as usize, out.budget)?;
            }
            out.budget.charge_work(2)?;
            if component.source_type(out.budget)? != atom_type {
                return Err(mismatch());
            }
            let nominal = semantic.types().get(atom_type.index() as usize)
                .ok_or_else(mismatch)?.rust_type_kind();
            let original = match (kind, component.carrier_shape(out.budget)?) {
                (Atom::Scalar(ScalarV30::Unit), Carrier::Unit | Carrier::Aggregate { components: 0 }) => None,
                (kind, Carrier::Value) => {
                    let original = component.original_definition(out.budget)?.ok_or_else(mismatch)?;
                    let physical = component.physical_type(out.budget)?.ok_or_else(mismatch)?;
                    let inventory = relation.inventory(out.budget)?;
                    out.budget.charge_work(2)?;
                    if inventory.definitions().get(original).map(|row| row.ty) != Some(physical)
                        || !match kind {
                            Atom::Scalar(scalar) => scalar_matches(scalar, nominal, physical, width),
                            Atom::Pointer { .. } => matches!(physical, Type::Pointer(_)),
                            Atom::Slice { .. } => matches!(physical, Type::Slice(_)),
                            _ => false,
                        }
                    {
                        return Err(mismatch());
                    }
                    Some(original)
                }
                _ => return Err(mismatch()),
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
            emit!(out, " && (source.machine.valid && target.valid && {frame} < source.machine.frames.active.len() && source.machine.frames.active[{frame}].owner == {owner} && source.logical.products.contains_key({local}) && source.logical.products[{local}].source_type == {} && ({{ let path = seq![", source_type.index());
            for field in path {
                out.budget.charge_work(1)?;
                emit!(out, "{field}int,");
            }
            emit!(out, "]; let product = source.logical.products[{local}]; product.components.contains_key(path) && invocation_source_product_atom_current_v282(source, {}, product.components[path], invocation_runtime_little_endian_v36()) && (match product.components[path] {{ InvocationSourceProductAtomV282::Carrier(original) => {{ ", atom_type.index());
            self.emit_source_original_relation(root, original, width, out)?;
            emit!(out, " }}, _ => false }}) }}))");
            Ok(())
        })
    }
}
