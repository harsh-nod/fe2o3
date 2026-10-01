//! Exact original field paths select carriers; they never supply source values.

use super::*;
use fe2o3_kernel_ir::{ScalarType as PhysicalScalar, Type as PhysicalType};
use fe2o3_lower_mir_kernel::{
    ProductionSourceSsaCarrierShapeV37 as Carrier, ProductionSourceSsaEndpointV36 as Endpoint,
};
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticFunctionIdV1 as FunctionId, SemanticRustTypeKindV1 as RustType,
    SemanticTypeIdV1 as TypeId, SemanticTypeShapeV1 as Shape,
};

#[derive(Clone, Copy)]
pub(super) struct ComponentBindingV42 {
    pub(super) leaf: usize,
    pub(super) definition: Option<usize>,
}

pub(super) struct AggregateBindingV42 {
    pub(super) local: usize,
    pub(super) source_type: TypeId,
    pub(super) components: Vec<ComponentBindingV42>,
}

fn refused() -> Error {
    Error::Statement("original aggregate component carrier or source type is not modeled")
}

pub(super) fn scalar_matches(
    source: ScalarV30,
    nominal: RustType,
    actual: &PhysicalType,
    width: FormalIndexWidth,
) -> bool {
    let PhysicalType::Scalar(actual) = actual else {
        return false;
    };
    match (source, *actual) {
        (ScalarV30::Bool, PhysicalScalar::Bool) => true,
        (ScalarV30::Float { width: 32 }, PhysicalScalar::F32)
        | (ScalarV30::Float { width: 64 }, PhysicalScalar::F64) => true,
        (
            ScalarV30::Integer {
                width: 8,
                signed: true,
            },
            PhysicalScalar::I8,
        )
        | (
            ScalarV30::Integer {
                width: 8,
                signed: false,
            },
            PhysicalScalar::U8,
        )
        | (
            ScalarV30::Integer {
                width: 16,
                signed: true,
            },
            PhysicalScalar::I16,
        )
        | (
            ScalarV30::Integer {
                width: 16,
                signed: false,
            },
            PhysicalScalar::U16,
        )
        | (
            ScalarV30::Integer {
                width: 32,
                signed: true,
            },
            PhysicalScalar::I32,
        )
        | (
            ScalarV30::Integer {
                width: 32,
                signed: false,
            },
            PhysicalScalar::U32,
        )
        | (
            ScalarV30::Integer {
                width: 64,
                signed: true,
            },
            PhysicalScalar::I64,
        )
        | (
            ScalarV30::Integer {
                width: 64,
                signed: false,
            },
            PhysicalScalar::U64,
        ) => true,
        (
            ScalarV30::Integer {
                width: bits,
                signed,
            },
            PhysicalScalar::Index,
        ) => {
            matches!(
                (nominal, signed),
                (RustType::Usize, false) | (RustType::Isize, true)
            ) && matches!(
                (width, bits),
                (FormalIndexWidth::Bits32, 32) | (FormalIndexWidth::Bits64, 64)
            )
        }
        _ => false,
    }
}

impl<'slots, 'view, 'source> PairedInvocations<'slots, 'view, 'source> {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn returned_binding(
        &mut self,
        plan: &InvocationPlan<'_, '_>,
        root: usize,
        instance: usize,
        local: usize,
        value: Value,
        frame: usize,
        physical: &Range<usize>,
        destination: &fe2o3_mir_model::semantic_mir_v1::SemanticPlaceV1,
        out: &mut Writer<'_, '_>,
    ) -> Result<Binding> {
        if destination.projections().is_empty() {
            return self.binding(
                plan, root, instance, local, value, frame, physical, None, out,
            );
        }
        let slots = self.slots;
        if slots.has_original_object(
            root,
            instance,
            u32::try_from(local).map_err(|_| Resource::Arithmetic)?,
            out,
        )? {
            return Err(refused());
        }
        let relation = slots.correspondence(out)?;
        let semantic = relation.source(out.budget)?.source_semantic(out.budget)?;
        let row = plan.instance(root, instance, out)?;
        let root_type = semantic
            .functions()
            .get(row.function.index() as usize)
            .and_then(|function| function.locals().get(local))
            .ok_or_else(mismatch)?
            .ty();
        let (range, result_type) = slots
            .aggregate_component_range(root_type, destination.projections(), out)?
            .ok_or_else(refused)?;
        if range.is_empty()
            || result_type != destination.ty()
            || destination.local().index() as usize != local
        {
            return Err(mismatch());
        }
        let endpoint = relation.ssa_typed_endpoint_v36(root, instance, value, out.budget)?;
        if endpoint.source_type(out.budget)? != root_type {
            return Err(mismatch());
        }
        let leaf = slots.aggregate_leaf(root_type, range.start, out)?;
        let path = leaf.path(out)?;
        if destination.projections().len() > path.len() {
            return Err(mismatch());
        }
        let mut selected = None;
        for &field in &path[..destination.projections().len()] {
            out.budget.charge_work(1)?;
            selected = Some(
                selected
                    .as_ref()
                    .unwrap_or(&endpoint)
                    .component(field as usize, out.budget)?,
            );
        }
        let endpoint = selected.as_ref().ok_or_else(mismatch)?;
        if endpoint.source_function(out.budget)? != row.function
            || endpoint.source_local(out.budget)?.index() as usize != local
            || endpoint.source_type(out.budget)? != result_type
        {
            return Err(mismatch());
        }
        if self.is_aggregate_binding(result_type, endpoint, out)? {
            return self.aggregate_binding(
                root,
                instance,
                local,
                row.function,
                result_type,
                endpoint,
                row.locals.start,
                frame,
                physical,
                None,
                out,
            );
        }
        let scalar = ScalarV30::from_source(semantic.types(), result_type)?;
        let definition = endpoint.original_definition(out.budget)?;
        match (
            definition,
            endpoint.physical_type(out.budget)?,
            endpoint.carrier_shape(out.budget)?,
        ) {
            (None, None, Carrier::Unit) if scalar == ScalarV30::Unit => (),
            (Some(definition), Some(ty), Carrier::Value) if physical.contains(&definition) => {
                let nominal = semantic
                    .types()
                    .get(result_type.index() as usize)
                    .ok_or_else(mismatch)?
                    .rust_type_kind();
                if relation
                    .inventory(out.budget)?
                    .definitions()
                    .get(definition)
                    .map(|row| row.ty)
                    != Some(ty)
                    || !scalar_matches(scalar, nominal, ty, self.width)
                {
                    return Err(refused());
                }
            }
            _ => return Err(refused()),
        }
        Ok(Binding {
            source: SourceValue::Local(add(row.locals.start, local)?),
            logical: LogicalBinding::Plain,
            definition,
            frame,
        })
    }

    pub(super) fn is_aggregate_binding(
        &self,
        ty: TypeId,
        endpoint: &Endpoint<'_, '_>,
        out: &mut Writer<'_, '_>,
    ) -> Result<bool> {
        let source = self.slots.correspondence(out)?.source(out.budget)?;
        let semantic = source.source_semantic(out.budget)?;
        out.budget.charge_work(2)?;
        let shape = semantic
            .types()
            .get(ty.index() as usize)
            .ok_or_else(mismatch)?
            .shape();
        if !matches!(
            shape,
            Shape::Tuple(_) | Shape::Aggregate(_) | Shape::Array { .. }
        ) {
            return Ok(false);
        }
        match endpoint.carrier_shape(out.budget)? {
            Carrier::Value => Ok(false),
            Carrier::Enum { .. } => Err(mismatch()),
            Carrier::Unit | Carrier::Aggregate { .. } => {
                self.slots
                    .aggregate_leaf_count(ty, out)?
                    .ok_or_else(refused)?;
                Ok(true)
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn aggregate_binding(
        &mut self,
        root: usize,
        instance: usize,
        local: usize,
        function: FunctionId,
        source_type: TypeId,
        endpoint: &Endpoint<'_, '_>,
        local_base: usize,
        frame: usize,
        physical: &Range<usize>,
        demanded_at: Option<ComponentCut>,
        out: &mut Writer<'_, '_>,
    ) -> Result<Binding> {
        out.budget.reserve_storage(headers())?;
        let slots = self.slots;
        let relation = slots.correspondence(out)?;
        let source = relation.source(out.budget)?;
        let semantic = source.source_semantic(out.budget)?;
        let inventory = relation.inventory(out.budget)?;
        if slots.has_original_object(
            root,
            instance,
            u32::try_from(local).map_err(|_| Resource::Arithmetic)?,
            out,
        )? {
            return Err(refused());
        }
        if endpoint.source_function(out.budget)? != function
            || endpoint.source_local(out.budget)?.index() as usize != local
            || endpoint.source_type(out.budget)? != source_type
        {
            return Err(Error::Statement(
                "original aggregate endpoint differs from its nominal source root",
            ));
        }
        let root_components = match semantic
            .types()
            .get(source_type.index() as usize)
            .ok_or_else(mismatch)?
            .shape()
        {
            Shape::Tuple(fields) | Shape::Aggregate(fields) => fields.fields().len(),
            Shape::Array { length, .. } => {
                usize::try_from(*length).map_err(|_| Resource::Arithmetic)?
            }
            _ => return Err(refused()),
        };
        match endpoint.carrier_shape(out.budget)? {
            Carrier::Aggregate { components } if components == root_components => (),
            Carrier::Unit if root_components == 0 => (),
            _ => {
                return Err(Error::Statement(
                    "original aggregate root carrier differs from its exact source arity",
                ));
            }
        }
        if slots
            .legacy_descriptor_by_source(
                root,
                instance,
                u32::try_from(local).map_err(|_| Resource::Arithmetic)?,
                out,
            )?
            .is_some()
        {
            return Err(refused());
        }
        let index = function.index() as usize;
        if demanded_at.is_some()
            && self
                .component_demands
                .get(index)
                .ok_or_else(mismatch)?
                .is_none()
        {
            self.component_demands[index] =
                Some(ComponentDemandsV42::derive(slots, function, out)?);
        }
        let demands = self.component_demands[index].as_ref();
        let count = slots
            .aggregate_leaf_count(source_type, out)?
            .ok_or_else(refused)?;
        if demanded_at.is_some_and(|cut| {
            cut.overwritten
                .is_some_and(|(start, end)| start >= end || end > count)
        }) {
            return Err(mismatch());
        }
        let mut components = vector(count, out)?;
        for leaf in 0..count {
            out.budget.charge_work(2)?;
            let required = match demanded_at {
                Some(cut) => {
                    !cut.overwritten
                        .is_some_and(|(start, end)| start <= leaf && leaf < end)
                        && demands
                            .ok_or_else(mismatch)?
                            .leaf_required(function, cut.block, local, leaf, out)?
                }
                None => true,
            };
            // A cut relates only independently live source components. Dead
            // children may be absent; whole-value snapshots take every path.
            if !required {
                continue;
            }
            let original = slots.aggregate_leaf(source_type, leaf, out)?;
            let path = original.path(out)?;
            let mut selected = None;
            for &field in path {
                out.budget.charge_work(4)?;
                let parent = selected.as_ref().unwrap_or(endpoint);
                let ty = parent.source_type(out.budget)?;
                let declaration = semantic
                    .types()
                    .get(ty.index() as usize)
                    .ok_or_else(mismatch)?;
                let expected = match declaration.shape() {
                    Shape::Tuple(fields) | Shape::Aggregate(fields) => fields.fields().len(),
                    Shape::Array { length, .. } => {
                        usize::try_from(*length).map_err(|_| Resource::Arithmetic)?
                    }
                    _ => return Err(refused()),
                };
                if parent.carrier_shape(out.budget)?
                    != (Carrier::Aggregate {
                        components: expected,
                    })
                {
                    return Err(Error::Statement(
                        "original aggregate child carrier differs from its exact source path",
                    ));
                }
                selected = Some(parent.component(field as usize, out.budget)?);
            }
            let selected = selected.as_ref().unwrap_or(endpoint);
            let ty = original.source_type(out)?;
            if selected.source_function(out.budget)? != function
                || selected.source_local(out.budget)?.index() as usize != local
                || selected.source_type(out.budget)? != ty
            {
                return Err(Error::Statement(
                    "original aggregate leaf differs from its nominal source path",
                ));
            }
            let scalar = original.scalar(out)?;
            let carrier = selected.carrier_shape(out.budget)?;
            // A source empty aggregate has one logical Unit leaf, but its
            // authenticated canonical tree can have zero physical children.
            let (definition, physical_type) =
                if carrier == (Carrier::Aggregate { components: 0 }) && scalar == ScalarV30::Unit {
                    (None, None)
                } else {
                    (
                        selected.original_definition(out.budget)?,
                        selected.physical_type(out.budget)?,
                    )
                };
            match (carrier, definition, physical_type) {
                (Carrier::Unit, None, None) if scalar == ScalarV30::Unit => (),
                (Carrier::Aggregate { components: 0 }, None, None) if scalar == ScalarV30::Unit => {
                    ()
                }
                (Carrier::Value, Some(definition), Some(ty)) if physical.contains(&definition) => {
                    let nominal = semantic
                        .types()
                        .get(original.source_type(out)?.index() as usize)
                        .ok_or_else(mismatch)?
                        .rust_type_kind();
                    if inventory.definitions().get(definition).map(|row| row.ty) != Some(ty)
                        || !scalar_matches(scalar, nominal, ty, self.width)
                    {
                        return Err(refused());
                    }
                }
                _ => return Err(refused()),
            }
            components.push(ComponentBindingV42 { leaf, definition });
        }
        let aggregate = self.aggregates.len();
        push_aggregate(
            &mut self.aggregates,
            AggregateBindingV42 {
                local: add(local_base, local)?,
                source_type,
                components,
            },
            out,
        )?;
        Ok(Binding {
            source: SourceValue::Aggregate(aggregate),
            logical: LogicalBinding::Plain,
            definition: None,
            frame,
        })
    }
}

fn push_aggregate(
    rows: &mut Vec<AggregateBindingV42>,
    row: AggregateBindingV42,
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    out.budget.charge_work(1)?;
    if rows.len() == rows.capacity() {
        let old = rows.capacity();
        let desired = old.checked_mul(2).ok_or(Resource::Arithmetic)?.max(4);
        // During reallocation both old and replacement storage may be live.
        out.budget.reserve_storage(
            desired
                .checked_mul(size_of::<AggregateBindingV42>())
                .ok_or(Resource::Arithmetic)?,
        )?;
        out.budget.charge_work(rows.len())?;
        rows.try_reserve_exact(desired - rows.len())
            .map_err(|_| Resource::Allocation)?;
        if rows.capacity() > desired {
            out.budget.reserve_storage(
                (rows.capacity() - desired)
                    .checked_mul(size_of::<AggregateBindingV42>())
                    .ok_or(Resource::Arithmetic)?,
            )?;
        }
        out.budget.release_storage(
            old.checked_mul(size_of::<AggregateBindingV42>())
                .ok_or(Resource::Arithmetic)?,
        )?;
    }
    rows.push(row);
    Ok(())
}

pub(super) fn headers() -> usize {
    fn h<T>() -> usize {
        size_of::<T>() + 2 * size_of::<Result<T>>()
    }
    h::<AggregateBindingV42>()
        + h::<ComponentBindingV42>()
        + h::<Vec<ComponentBindingV42>>()
        + h::<Option<Endpoint<'_, '_>>>()
        + 18 * size_of::<usize>()
        + 12 * size_of::<&()>()
}
