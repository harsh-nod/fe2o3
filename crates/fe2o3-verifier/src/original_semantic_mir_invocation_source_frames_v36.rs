//! Exact source invocation returns. Logical source lifetime never pops the
//! independent physical target allocation frame.

use super::super::{LocalRole, ScalarV30, Shape, Terminator, invocations::InvocationPlan};
use super::source_bytes::Access;
use super::source_bytes::{descriptor_helpers, descriptor_loans::Recipe};
use super::{Error, Resource, Result, Writer, slots::SourceSlots, vector};
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticCallableDeclV1 as Callable, SemanticEdgeRoleV1 as EdgeRole,
    SemanticPointerMetadataV1 as Metadata, SemanticTypeIdV1 as TypeId,
    SemanticUnwindActionV1 as Unwind,
};
use std::{fmt::Write as _, mem::size_of, ops::Range};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ReturnClass {
    Unit,
    Scalar(u32),
    Pointer,
    Slice(u32),
    Aggregate(u32),
    Product(u32),
    Enum(u32),
    Descriptor,
}

#[derive(Clone, Copy, Debug)]
struct DestinationComponent {
    root_type: TypeId,
    result_type: TypeId,
    first_leaf: usize,
    depth: usize,
    product: bool,
}

pub(super) struct SourceFrameReturn<'slots, 'view, 'source> {
    slots: &'slots SourceSlots<'view, 'source>,
    root: usize,
    instance: usize,
    owners: Vec<u32>,
    returns: Vec<usize>,
    descriptor_returns: Vec<Recipe>,
    descriptor_destination: Option<Recipe>,
    locals: Range<usize>,
    returned: Option<usize>,
    destination: Option<usize>,
    destination_component: Option<DestinationComponent>,
    destination_memory: Option<(Access, usize)>,
    continuation: Option<usize>,
    class: ReturnClass,
    required: usize,
}

fn mismatch() -> Error {
    Error::Statement("original MIR byte frame return differs from its exact invocation")
}

impl<'slots, 'view, 'source> SourceFrameReturn<'slots, 'view, 'source> {
    pub(super) fn heap_conservation_shape(&self, out: &mut Writer<'_, '_>) -> Result<bool> {
        let source = self.slots.correspondence(out)?.source(out.budget)?;
        if out.budget.storage() < self.required {
            return Err(source
                .retain_query_resource_error_v18(Resource::Accounting)
                .into());
        }
        out.budget.charge_work(4)?;
        Ok(self.class == ReturnClass::Unit
            && self.destination_component.is_none()
            && self.destination_memory.is_none()
            && self.descriptor_destination.is_none())
    }

    pub(super) fn derive(
        plan: &InvocationPlan<'_, '_>,
        slots: &'slots SourceSlots<'view, 'source>,
        root: usize,
        instance: usize,
        out: &mut Writer<'_, '_>,
    ) -> Result<Self> {
        out.budget.reserve_storage(headers())?;
        let source = slots.correspondence(out)?.source(out.budget)?;
        if !std::ptr::eq(source, plan.source(out)?) {
            return Err(mismatch());
        }
        let row = plan.instance(root, instance, out)?;
        let semantic = source.source_semantic(out.budget)?;
        let function = semantic
            .functions()
            .get(row.function.index() as usize)
            .ok_or_else(mismatch)?;
        out.budget.charge_work(4)?;
        if !row.active
            || row.locals.len() != function.locals().len()
            || row.blocks.len() != function.blocks().len()
        {
            return Err(mismatch());
        }
        let ty = function.abi().return_type();
        if super::source_bytes::execution_loans::nominal_reference(semantic.types(), ty)?.is_some()
        {
            return Err(Error::Statement(
                "original helper execution reference return requires loan snapshot transport",
            ));
        }
        let class = if slots.is_product_v282(ty, out)? {
            if instance == 0 || row.incoming.is_none() {
                return Err(mismatch());
            }
            ReturnClass::Product(ty.index())
        } else if descriptor_helpers::nominal_reference(slots, ty, out)? {
            if instance == 0 || row.incoming.is_none() {
                return Err(mismatch());
            }
            ReturnClass::Descriptor
        } else {
            match semantic
                .types()
                .get(ty.index() as usize)
                .map(|ty| ty.shape())
            {
                Some(Shape::Unit) => ReturnClass::Unit,
                Some(Shape::Enum { .. }) => {
                    out.budget.charge_work(2)?;
                    if instance == 0 || row.incoming.is_none() {
                        return Err(mismatch());
                    }
                    ReturnClass::Enum(ty.index())
                }
                Some(Shape::Pointer(pointer)) => match pointer.metadata() {
                    Metadata::None => ReturnClass::Pointer,
                    Metadata::SliceLength => {
                        ReturnClass::Slice(super::source_bytes::slice_metadata_bits_v36(
                            semantic
                                .types()
                                .get(ty.index() as usize)
                                .ok_or_else(mismatch)?,
                            out,
                        )?)
                    }
                    _ => return Err(mismatch()),
                },
                _ => match slots.descriptor_slice_bits(ty, out)? {
                    Some(bits) => ReturnClass::Slice(bits),
                    None => {
                        if matches!(
                            semantic
                                .types()
                                .get(ty.index() as usize)
                                .map(|ty| ty.shape()),
                            Some(Shape::Tuple(_) | Shape::Aggregate(_) | Shape::Array { .. })
                        ) && slots.aggregate_leaf_count(ty, out)?.is_some()
                        {
                            ReturnClass::Aggregate(ty.index())
                        } else {
                            ReturnClass::Scalar(
                                ScalarV30::from_source(semantic.types(), ty)?.width(),
                            )
                        }
                    }
                },
            }
        };
        let mut returned = None;
        for (local, declaration) in function.locals().iter().enumerate() {
            out.budget.charge_work(2)?;
            if declaration.role() == LocalRole::Return {
                if returned.is_some()
                    || declaration.ty() != ty
                    || slots.has_original_object(
                        root,
                        instance,
                        u32::try_from(local).map_err(|_| Resource::Arithmetic)?,
                        out,
                    )?
                    || slots
                        .legacy_descriptor_by_source(
                            root,
                            instance,
                            u32::try_from(local).map_err(|_| Resource::Arithmetic)?,
                            "source-frame-return-local",
                            out,
                        )?
                        .is_some()
                {
                    return Err(mismatch());
                }
                returned = Some(
                    row.locals
                        .start
                        .checked_add(local)
                        .ok_or(Resource::Arithmetic)?,
                );
            }
        }
        if returned.is_none() && class != ReturnClass::Unit {
            return Err(mismatch());
        }
        let mut count = 0usize;
        for block in function.blocks() {
            out.budget.charge_work(1)?;
            if matches!(block.terminator().kind(), Terminator::Return) {
                count = count.checked_add(1).ok_or(Resource::Arithmetic)?;
            }
        }
        let mut returns = vector(count, out)?;
        let mut descriptor_returns = vector(
            if class == ReturnClass::Descriptor {
                count
            } else {
                0
            },
            out,
        )?;
        for (block, declaration) in function.blocks().iter().enumerate() {
            out.budget.charge_work(1)?;
            if matches!(declaration.terminator().kind(), Terminator::Return) {
                if class == ReturnClass::Descriptor {
                    let local = returned
                        .ok_or_else(mismatch)?
                        .checked_sub(row.locals.start)
                        .ok_or_else(mismatch)?;
                    descriptor_returns.push(
                        descriptor_helpers::return_recipe(
                            slots,
                            plan,
                            root,
                            instance,
                            block,
                            fe2o3_mir_model::semantic_mir_v1::SemanticLocalIdV1::from_index(
                                u32::try_from(local).map_err(|_| Resource::Arithmetic)?,
                            ),
                            out,
                        )?
                        .ok_or_else(mismatch)?,
                    );
                }
                returns.push(
                    row.blocks
                        .start
                        .checked_add(block)
                        .ok_or(Resource::Arithmetic)?,
                );
            }
        }
        if returns.len() != count {
            return Err(mismatch());
        }
        let mut destination_component = None;
        let mut destination_memory = None;
        let mut descriptor_destination = None;
        let (destination, continuation) = if let Some((parent, block)) = row.incoming {
            if parent >= instance {
                return Err(mismatch());
            }
            let parent_row = plan.instance(root, parent, out)?;
            let caller = semantic
                .functions()
                .get(parent_row.function.index() as usize)
                .ok_or_else(mismatch)?;
            let call = match caller
                .blocks()
                .get(block.index() as usize)
                .map(|block| block.terminator().kind())
            {
                Some(Terminator::Call(call)) => call,
                _ => return Err(mismatch()),
            };
            let destination = call.destination().ok_or_else(mismatch)?;
            let local = destination.place().local().index();
            out.budget.charge_work(8)?;
            if !parent_row.active
                || !matches!(semantic.callables().get(call.callee().index() as usize), Some(Callable::Defined { function }) if *function == row.function)
                || call.unwind() != Unwind::Unreachable
                || destination.edge().role() != EdgeRole::CallReturn
                || destination.place().ty() != ty
            {
                return Err(mismatch());
            }
            let root_type = caller
                .locals()
                .get(local as usize)
                .ok_or_else(mismatch)?
                .ty();
            if class == ReturnClass::Descriptor {
                descriptor_destination = Some(descriptor_helpers::destination_recipe(
                    slots,
                    plan,
                    root,
                    parent,
                    block.index() as usize,
                    out,
                )?);
            }
            if let Some(access) = super::source_bytes::object_call_destination_v42(
                plan,
                slots,
                root,
                parent,
                block.index() as usize,
                ty,
                out,
            )? {
                if !matches!(class, ReturnClass::Scalar(_)) {
                    return Err(mismatch());
                }
                destination_memory = Some((access, parent));
            } else {
                // Original objects have exact statement generations and were
                // authenticated above; only legacy storage uses a local key.
                if slots
                    .legacy_descriptor_by_source(
                        root,
                        parent,
                        local,
                        "source-frame-caller-destination",
                        out,
                    )?
                    .is_some()
                {
                    return Err(mismatch());
                }
                if destination.place().projections().is_empty() {
                    if root_type != ty {
                        return Err(mismatch());
                    }
                } else {
                    if matches!(class, ReturnClass::Enum(_) | ReturnClass::Descriptor) {
                        return Err(mismatch());
                    }
                    let product = slots.is_product_v282(root_type, out)?;
                    let (range, result_type) = if product {
                        slots.product_component_range_v282(
                            root_type,
                            destination.place().projections(),
                            out,
                        )?
                    } else {
                        slots.aggregate_component_range(
                            root_type,
                            destination.place().projections(),
                            out,
                        )?
                    }
                    .ok_or_else(mismatch)?;
                    if result_type != ty || range.is_empty() {
                        return Err(mismatch());
                    }
                    destination_component = Some(DestinationComponent {
                        root_type,
                        result_type,
                        first_leaf: range.start,
                        depth: destination.place().projections().len(),
                        product,
                    });
                }
            }
            let target = parent_row
                .blocks
                .start
                .checked_add(destination.edge().target().index() as usize)
                .ok_or(Resource::Arithmetic)?;
            let destination = parent_row
                .locals
                .start
                .checked_add(local as usize)
                .ok_or(Resource::Arithmetic)?;
            if target >= parent_row.blocks.end
                || destination >= parent_row.locals.end
                || row.locals.contains(&destination)
            {
                return Err(mismatch());
            }
            (Some(destination), Some(target))
        } else {
            if instance != 0 {
                return Err(mismatch());
            }
            (None, None)
        };
        let mut depth = 0usize;
        let mut current = instance;
        loop {
            out.budget.charge_work(2)?;
            depth = depth.checked_add(1).ok_or(Resource::Arithmetic)?;
            let ancestor = plan.instance(root, current, out)?;
            if !ancestor.active {
                return Err(mismatch());
            }
            match ancestor.incoming {
                Some((parent, _)) if parent < current => current = parent,
                None if current == 0 => break,
                _ => return Err(mismatch()),
            }
        }
        let mut owners = vector(depth, out)?;
        current = instance;
        loop {
            out.budget.charge_work(2)?;
            let ancestor = plan.instance(root, current, out)?;
            if owners.len() == owners.capacity() {
                return Err(Resource::Accounting.into());
            }
            owners.push(ancestor.function.index());
            match ancestor.incoming {
                Some((parent, _)) if parent < current => current = parent,
                None if current == 0 => break,
                _ => return Err(mismatch()),
            }
        }
        out.budget.charge_work(depth)?;
        owners.reverse();
        if owners.len() != depth {
            return Err(mismatch());
        }
        Ok(Self {
            slots,
            root,
            instance,
            owners,
            returns,
            descriptor_returns,
            descriptor_destination,
            locals: row.locals.clone(),
            returned,
            destination,
            destination_component,
            destination_memory,
            continuation,
            class,
            required: out.budget.storage(),
        })
    }

    pub(super) fn emit(&self, out: &mut Writer<'_, '_>) -> Result<()> {
        let source = self.slots.correspondence(out)?.source(out.budget)?;
        if out.budget.storage() < self.required {
            return Err(source
                .retain_query_resource_error_v18(Resource::Accounting)
                .into());
        }
        write!(out, "spec fn invocation_source_return_{}_{}_v36(source: InvocationSourceByteStateV36, little_endian: bool) -> InvocationSourceByteReturnV36 {{\n if !source.machine.valid || !byte_frame_runtime_well_formed_v30(source.machine.frames) || source.machine.frames.active.len() != {} || source.machine.frames.active[0].invocation != 0", self.root, self.instance, self.owners.len()).map_err(|_| out.error())?;
        for (at, owner) in self.owners.iter().enumerate() {
            out.budget.charge_work(1)?;
            write!(
                out,
                " || source.machine.frames.active[{at}].owner != {owner}"
            )
            .map_err(|_| out.error())?;
        }
        write!(out, " || !(false").map_err(|_| out.error())?;
        for block in &self.returns {
            out.budget.charge_work(1)?;
            write!(out, " || source.machine.pc == {block}").map_err(|_| out.error())?;
        }
        write!(out, ")").map_err(|_| out.error())?;
        if self.class != ReturnClass::Unit
            && !matches!(
                self.class,
                ReturnClass::Aggregate(_)
                    | ReturnClass::Product(_)
                    | ReturnClass::Enum(_)
                    | ReturnClass::Descriptor
            )
        {
            let local = self.returned.ok_or_else(mismatch)?;
            write!(out, " || source.machine.values.len() <= {local} || !(")
                .map_err(|_| out.error())?;
            match self.class {
                ReturnClass::Scalar(bits) => write!(out, "invocation_source_byte_value_typed_v36(source.machine.values[{local}], {bits})"),
                ReturnClass::Pointer => write!(out, "match source.machine.values[{local}] {{ MemoryValueV30::Pointer(_) => true, _ => false }}"),
                ReturnClass::Slice(bits) => write!(out, "match source.machine.values[{local}] {{ MemoryValueV30::Slice(slice) => 0 <= slice.length < memory_value_modulus_v30({}), _ => false }}", bits / 8),
                ReturnClass::Unit => unreachable!(),
                ReturnClass::Aggregate(_) | ReturnClass::Product(_) | ReturnClass::Enum(_) | ReturnClass::Descriptor => unreachable!(),
            }.map_err(|_| out.error())?;
            write!(out, ")").map_err(|_| out.error())?;
        }
        write!(
            out,
            " {{ invocation_source_return_refused_v36(source) }} else {{\n let returned = "
        )
        .map_err(|_| out.error())?;
        if self.class == ReturnClass::Unit {
            write!(
                out,
                "InvocationSourceValueV42::Carrier(MemoryValueV30::Unit)"
            )
            .map_err(|_| out.error())?;
        } else if self.class == ReturnClass::Descriptor {
            if self.descriptor_returns.len() != self.returns.len() {
                return Err(mismatch());
            }
            for (block, recipe) in self.returns.iter().zip(&self.descriptor_returns) {
                out.budget.charge_work(1)?;
                write!(out, "if source.machine.pc == {block} {{ match invocation_source_descriptor_snapshot_v53(source, {}, ", self.returned.ok_or_else(mismatch)?).map_err(|_| out.error())?;
                recipe.emit(out)?;
                write!(out, ") {{ Some(value) => InvocationSourceValueV42::Descriptor(value), None => InvocationSourceValueV42::Carrier(MemoryValueV30::Undefined) }} }} else ").map_err(|_| out.error())?;
            }
            write!(
                out,
                "{{ InvocationSourceValueV42::Carrier(MemoryValueV30::Undefined) }}"
            )
            .map_err(|_| out.error())?;
        } else if let ReturnClass::Aggregate(ty) = self.class {
            write!(out, "match invocation_source_aggregate_snapshot_v42(source, {}, {ty}, seq![], {ty}) {{ Some(value) => InvocationSourceValueV42::Aggregate(value), None => InvocationSourceValueV42::Carrier(MemoryValueV30::Undefined) }}", self.returned.ok_or_else(mismatch)?).map_err(|_| out.error())?;
        } else if let ReturnClass::Product(ty) = self.class {
            write!(out, "match invocation_source_product_return_snapshot_v282(source, {}, {ty}, little_endian) {{ Some(value) => InvocationSourceValueV42::Product(value), None => InvocationSourceValueV42::Carrier(MemoryValueV30::Undefined) }}", self.returned.ok_or_else(mismatch)?).map_err(|_| out.error())?;
        } else if let ReturnClass::Enum(ty) = self.class {
            write!(out, "match invocation_source_enum_snapshot_v50(source, {}, {ty}, little_endian) {{ Some(value) => InvocationSourceValueV42::Enum(value), None => InvocationSourceValueV42::Carrier(MemoryValueV30::Undefined) }}", self.returned.ok_or_else(mismatch)?).map_err(|_| out.error())?;
        } else {
            write!(
                out,
                "InvocationSourceValueV42::Carrier(source.machine.values[{}])",
                self.returned.ok_or_else(mismatch)?
            )
            .map_err(|_| out.error())?;
        }
        write!(
            out,
            ";\n invocation_source_return_v36(source, {}, {}, returned, ",
            self.locals.start, self.locals.end
        )
        .map_err(|_| out.error())?;
        match self.destination {
            Some(local) => {
                write!(
                    out,
                    "Some(InvocationSourceReturnDestinationV42 {{ local: {local}, component: "
                )
                .map_err(|_| out.error())?;
                match self.destination_component {
                    Some(component) => {
                        write!(
                            out,
                            "Some(({}int, {}int, seq![",
                            component.root_type.index(),
                            component.result_type.index()
                        )
                        .map_err(|_| out.error())?;
                        if component.product {
                            let atom = self.slots.product_component_v282(
                                component.root_type,
                                component.first_leaf,
                                out,
                            )?;
                            let path = atom.path(out)?;
                            if component.depth > path.len() {
                                return Err(mismatch());
                            }
                            for field in &path[..component.depth] {
                                out.budget.charge_work(1)?;
                                write!(out, "{field}int,").map_err(|_| out.error())?;
                            }
                        } else {
                            let leaf = self.slots.aggregate_leaf(
                                component.root_type,
                                component.first_leaf,
                                out,
                            )?;
                            let path = leaf.path(out)?;
                            if component.depth > path.len() {
                                return Err(mismatch());
                            }
                            for field in &path[..component.depth] {
                                out.budget.charge_work(1)?;
                                write!(out, "{field}int,").map_err(|_| out.error())?;
                            }
                        }
                        write!(out, "]))").map_err(|_| out.error())?;
                    }
                    None => write!(out, "None").map_err(|_| out.error())?,
                }
                write!(out, ", memory: ").map_err(|_| out.error())?;
                if let Some((access, parent)) = self.destination_memory {
                    let ReturnClass::Scalar(bits) = self.class else {
                        return Err(mismatch());
                    };
                    write!(out, "Some((").map_err(|_| out.error())?;
                    access.emit(out)?;
                    write!(out, ", {}int, {parent}int, {bits}int))", self.root)
                        .map_err(|_| out.error())?;
                } else {
                    write!(out, "None").map_err(|_| out.error())?;
                }
                write!(out, ", descriptor: ").map_err(|_| out.error())?;
                if let Some(recipe) = self.descriptor_destination {
                    write!(out, "Some(").map_err(|_| out.error())?;
                    recipe.emit(out)?;
                    write!(out, ")").map_err(|_| out.error())?;
                } else {
                    write!(out, "None").map_err(|_| out.error())?;
                }
                write!(out, " }})").map_err(|_| out.error())?;
            }
            None => write!(out, "None").map_err(|_| out.error())?,
        }
        match self.continuation {
            Some(block) => write!(out, ", {block}int, little_endian)\n }}\n}}\n"),
            None => write!(out, ", -1int, little_endian)\n }}\n}}\n"),
        }
        .map_err(|_| out.error())
    }
}

fn headers() -> usize {
    fn h<T>() -> usize {
        size_of::<T>() + 2 * size_of::<Result<T>>()
    }
    h::<SourceFrameReturn<'_, '_, '_>>()
        + h::<Vec<u32>>()
        + h::<Vec<usize>>()
        + h::<Vec<Recipe>>()
        + h::<Option<Recipe>>()
        + descriptor_helpers::headers()
        + h::<Range<usize>>()
        + h::<ReturnClass>()
        + h::<Option<DestinationComponent>>()
        + h::<Option<(Access, usize)>>()
        + h::<Option<usize>>()
        + 24 * size_of::<usize>()
        + 24 * size_of::<&()>()
}

pub(super) const SOURCE_FRAMES_V36: &str = r#"
struct InvocationSourceByteReturnV36 {
    source: InvocationSourceByteStateV36,
    returned: InvocationSourceValueV42,
}

struct InvocationSourceReturnDestinationV42 {
    local: int,
    component: Option<(int, int, Seq<int>)>,
    memory: Option<(InvocationSourceByteAccessV36, int, int, int)>,
    descriptor: Option<InvocationSourceDescriptorRecipeV51>,
}

spec fn invocation_source_return_refused_v36(source: InvocationSourceByteStateV36)
    -> InvocationSourceByteReturnV36
{
    InvocationSourceByteReturnV36 { source: invocation_source_byte_refused_v36(source),
        returned: InvocationSourceValueV42::Carrier(MemoryValueV30::Undefined) }
}

spec fn invocation_source_return_value_defined_v42(value: InvocationSourceValueV42) -> bool {
    match value {
        InvocationSourceValueV42::Execution(_) | InvocationSourceValueV42::ExecutionTransfer(_) => false,
        InvocationSourceValueV42::Carrier(value) => match value {
            MemoryValueV30::Undefined => false, _ => true },
        InvocationSourceValueV42::Aggregate(value) => invocation_source_aggregate_complete_v42(value),
        InvocationSourceValueV42::Product(value) => invocation_source_product_complete_v282(value),
        InvocationSourceValueV42::Enum(value) => invocation_source_enum_complete_v47(value),
        InvocationSourceValueV42::Descriptor(value) => matches!(value.value, MemoryValueV30::Slice(_)),
    }
}

spec fn invocation_source_return_install_v42(
    source: InvocationSourceByteStateV36, destination: InvocationSourceReturnDestinationV42,
    value: InvocationSourceValueV42, little_endian: bool,
) -> InvocationSourceByteStateV36 {
    if let Some(recipe) = destination.descriptor {
        if destination.component.is_some() || destination.memory.is_some() {
            invocation_source_byte_refused_v36(source)
        } else { match value {
            InvocationSourceValueV42::Descriptor(value) =>
                invocation_source_descriptor_snapshot_install_v53(source, destination.local, recipe, value),
            _ => invocation_source_byte_refused_v36(source),
        } }
    } else if let Some((access, root, instance, bits)) = destination.memory {
        if destination.component.is_some()
            || access.base != InvocationSourceByteBaseV36::ObjectLocal(destination.local)
            || bits <= 0 || !(bits == 1 && access.width == 1 || bits == access.width * 8) {
            invocation_source_byte_refused_v36(source)
        } else { match (value, invocation_source_byte_address_v36(source, access, root, instance)) {
            (InvocationSourceValueV42::Carrier(value), Some(pointer)) => {
                if !invocation_source_byte_value_typed_v36(value, bits)
                    || !invocation_private_allocation_v36(pointer.allocation) {
                    invocation_source_byte_refused_v36(source)
                } else { InvocationSourceByteStateV36 {
                    machine: invocation_source_store_v36(source.machine, pointer,
                        access.width, access.alignment, value, little_endian), ..source } }
            },
            _ => invocation_source_byte_refused_v36(source),
        } }
    } else { match destination.component {
        Some((root_type, result_type, path)) => {
            if invocation_source_product_type_v282(root_type) {
                match invocation_source_product_pack_v282(result_type, value, None) {
                    Some(components) => invocation_source_product_replace_path_v282(source,
                        destination.local, root_type, path, result_type, components, little_endian),
                    None => invocation_source_byte_refused_v36(source),
                }
            } else {
            let aggregate = match value {
                InvocationSourceValueV42::Carrier(value) => Some(InvocationSourceAggregateV42 {
                    source_type: result_type, leaves: Map::empty().insert(seq![], value),
                    execution_lease: None }),
                InvocationSourceValueV42::Aggregate(value) => Some(value),
                InvocationSourceValueV42::Product(_) | InvocationSourceValueV42::Enum(_) | InvocationSourceValueV42::Descriptor(_)
                | InvocationSourceValueV42::Execution(_) | InvocationSourceValueV42::ExecutionTransfer(_) => None,
            };
            match aggregate {
                Some(aggregate) => if aggregate.source_type == result_type {
                    invocation_source_aggregate_replace_v42(source, destination.local,
                        root_type, path, aggregate)
                } else { invocation_source_byte_refused_v36(source) },
                None => invocation_source_byte_refused_v36(source),
            }
            }
        },
        None => match value {
            InvocationSourceValueV42::Carrier(value) =>
                invocation_source_byte_put_local_v36(source, destination.local, value),
            InvocationSourceValueV42::Aggregate(value) =>
                invocation_source_aggregate_install_v42(source, destination.local, value),
            InvocationSourceValueV42::Product(value) =>
                invocation_source_product_install_v282(source, destination.local, value, little_endian),
            InvocationSourceValueV42::Enum(value) =>
                if invocation_source_enum_snapshot_current_v50(source, value, little_endian) {
                    invocation_source_enum_install_v47(source, destination.local, value)
                } else { invocation_source_byte_refused_v36(source) },
            InvocationSourceValueV42::Descriptor(_) | InvocationSourceValueV42::Execution(_)
            | InvocationSourceValueV42::ExecutionTransfer(_) =>
                invocation_source_byte_refused_v36(source),
        },
    } }
}

spec fn invocation_source_snapshot_escapes_frame_v42(
    value: InvocationSourceValueV42, frame: MemoryDynamicFrameV30,
) -> bool {
    match value {
        InvocationSourceValueV42::Carrier(value) => invocation_source_value_escapes_frame_v36(value, frame),
        InvocationSourceValueV42::Aggregate(value) =>
            (match value.execution_lease { Some(lease) => lease.frame == frame, None => false })
            || (exists|path: Seq<int>| value.leaves.contains_key(path)
                && invocation_source_value_escapes_frame_v36(value.leaves[path], frame)),
        InvocationSourceValueV42::Product(value) => invocation_source_product_return_escapes_frame_v282(value, frame),
        InvocationSourceValueV42::Execution(_) | InvocationSourceValueV42::ExecutionTransfer(_) => true,
        InvocationSourceValueV42::Enum(value) => exists|field: int|
            value.fields.contains_key(field) && invocation_source_value_escapes_frame_v36(value.fields[field], frame),
        InvocationSourceValueV42::Descriptor(value) => invocation_source_descriptor_snapshot_escapes_frame_v53(value, frame),
    }
}

spec fn invocation_source_value_escapes_frame_v36(
    value: MemoryValueV30, frame: MemoryDynamicFrameV30,
) -> bool {
    match value {
        MemoryValueV30::Pointer(pointer) => byte_allocation_in_frame_v30(pointer.allocation, frame),
        MemoryValueV30::Slice(slice) => byte_allocation_in_frame_v30(slice.pointer.allocation, frame),
        MemoryValueV30::Execution(_) => true,
        _ => false,
    }
}

proof fn invocation_source_execution_capability_always_escapes_v178(
    capability: MemoryExecutionCapabilityV178, frame: MemoryDynamicFrameV30,
)
    ensures invocation_source_value_escapes_frame_v36(MemoryValueV30::Execution(capability), frame),
{}

proof fn invocation_source_pointer_value_escapes_frame_v77(
    pointer: MemoryPointerV30, frame: MemoryDynamicFrameV30,
)
    requires byte_allocation_in_frame_v30(pointer.allocation, frame),
    ensures invocation_source_value_escapes_frame_v36(MemoryValueV30::Pointer(pointer), frame),
{
    hide(byte_allocation_in_frame_v30);
    reveal(invocation_source_value_escapes_frame_v36);
}

#[verifier::spinoff_prover]
proof fn invocation_source_enum_return_cannot_hide_a_callee_pointer_v50(
    value: InvocationSourceEnumV47, field: int, pointer: MemoryPointerV30,
    frame: MemoryDynamicFrameV30,
)
    requires
        value.fields.contains_key(field),
        value.fields[field] == MemoryValueV30::Pointer(pointer),
        byte_allocation_in_frame_v30(pointer.allocation, frame),
    ensures
        invocation_source_snapshot_escapes_frame_v42(InvocationSourceValueV42::Enum(value), frame),
{
    hide(invocation_source_value_escapes_frame_v36);
    hide(invocation_source_descriptor_snapshot_escapes_frame_v53);
    hide(byte_allocation_in_frame_v30);
    reveal(invocation_source_snapshot_escapes_frame_v42);
    assert(invocation_source_snapshot_escapes_frame_v42(InvocationSourceValueV42::Enum(value), frame)
        == (exists|field: int| value.fields.contains_key(field)
            && invocation_source_value_escapes_frame_v36(value.fields[field], frame))) by (compute_only);
    invocation_source_pointer_value_escapes_frame_v77(pointer, frame);
    assert(invocation_source_value_escapes_frame_v36(value.fields[field], frame));
    assert(exists|index: int| value.fields.contains_key(index)
        && invocation_source_value_escapes_frame_v36(value.fields[index], frame));
}

// Initialized pointer fragments retain nominal provenance even when they do
// not authorize a complete pointer load. Deinitialized historical tokens do not.
spec fn invocation_source_memory_escapes_frame_v37(
    memory: ByteMemoryV30, frame: MemoryDynamicFrameV30,
) -> bool {
    exists|allocation: MemoryAllocationV30, at: int|
        memory.live.contains_key(allocation)
        && !byte_allocation_in_frame_v30(allocation, frame)
        && ((memory.live[allocation].relocations.contains_key(at)
            && byte_allocation_in_frame_v30(memory.live[allocation].relocations[at].pointer.allocation, frame))
            || (0 <= at < memory.live[allocation].bytes.len()
                && memory.live[allocation].initialized[at]
                && match memory.live[allocation].bytes[at] {
                    MemoryByteV37::PointerFragment { pointer, .. } =>
                        byte_allocation_in_frame_v30(pointer.allocation, frame),
                    _ => false,
                }))
}

// Called only by the exact source-instance wrapper. The independent statement
// dispatcher still admits each stored-pointer operation separately.
spec fn invocation_source_return_v36(
    source: InvocationSourceByteStateV36, begin: int, end: int,
    returned: InvocationSourceValueV42, destination: Option<InvocationSourceReturnDestinationV42>, continuation: int,
    little_endian: bool,
) -> InvocationSourceByteReturnV36 {
    if !source.machine.valid || !invocation_source_byte_state_well_formed_v36(source)
        || source.machine.frames.active.len() == 0
        || begin < 0 || end < begin || source.machine.values.len() < end
        || !invocation_source_return_value_defined_v42(returned)
        || match destination {
            Some(destination) => destination.local < 0 || source.machine.values.len() <= destination.local
                || (begin <= destination.local < end) || source.machine.frames.active.len() < 2
                || continuation < 0,
            None => source.machine.frames.active.len() != 1 || continuation != -1,
        }
    {
        invocation_source_return_refused_v36(source)
    } else {
        let frame = source.machine.frames.active.last();
        // The wrapper captured the complete value before teardown. Return
        // installation runs only after the caller becomes the active frame.
        let values = Seq::new(source.machine.values.len(), |i: int|
            if begin <= i < end { MemoryValueV30::Undefined } else { source.machine.values[i] });
        let logical = invocation_source_logical_clear_v38(source.logical, begin, end);
        let escapes = invocation_source_snapshot_escapes_frame_v42(returned, frame)
            || (exists|local: int| logical.execution_pending.contains_key(local)
                && (logical.execution_pending[local].caller_frame == frame
                    || logical.execution_pending[local].reference.frame == frame))
            || (exists|local: int| logical.products.contains_key(local)
                && invocation_source_product_escapes_frame_v282(logical.products[local], frame))
            || (exists|i: int| 0 <= i < values.len()
                && invocation_source_value_escapes_frame_v36(values[i], frame))
            || (exists|local: int, path: Seq<int>| logical.aggregates.contains_key(local)
                && logical.aggregates[local].leaves.contains_key(path)
                && invocation_source_value_escapes_frame_v36(logical.aggregates[local].leaves[path], frame))
            || (exists|local: int, field: int| logical.enums.contains_key(local)
                && logical.enums[local].fields.contains_key(field)
                && invocation_source_value_escapes_frame_v36(logical.enums[local].fields[field], frame))
            || invocation_source_memory_escapes_frame_v37(source.machine.memory, frame)
            || (exists|i: int| logical.descriptor_references.contains_key(i)
                && logical.descriptor_references[i].loan.frame == frame)
            || (exists|i: int| logical.references.contains_key(i)
                && logical.references[i].frame == frame);
        if escapes {
            invocation_source_return_refused_v36(source)
        } else {
            let cleaned = InvocationSourceByteStateV36 {
                    machine: MemoryStateV30 { pc: continuation, values,
                        memory: byte_end_frame_v30(source.machine.memory, frame),
                        generations: source.machine.generations,
                        frames: byte_pop_frame_v30(source.machine.frames), valid: true },
                    slots: Map::new(
                        source.slots.dom().filter(|descriptor: int|
                            !byte_allocation_in_frame_v30(source.slots[descriptor].allocation, frame)),
                        |descriptor: int| source.slots[descriptor],
                    ),
                    objects: Map::new(
                        source.objects.dom().filter(|local: int|
                            !byte_allocation_in_frame_v30(source.slots[source.objects[local].descriptor].allocation, frame)),
                        |local: int| source.objects[local],
                    ),
                    logical,
                };
            let installed = match destination {
                Some(destination) => invocation_source_return_install_v42(cleaned, destination, returned, little_endian),
                None => cleaned,
            };
            if !installed.machine.valid || !invocation_source_byte_state_well_formed_v36(installed)
                || installed.machine.frames != cleaned.machine.frames
                || installed.machine.generations != cleaned.machine.generations {
                invocation_source_return_refused_v36(installed)
            } else { InvocationSourceByteReturnV36 { source: installed, returned } }
        }
    }
}
"#;

#[cfg(test)]
mod tests {
    use super::*;

    const LIMIT: usize = 100_000_000;

    fn run(
        work: usize,
        storage: usize,
        examine: impl FnOnce(
            &InvocationPlan<'_, '_>,
            &SourceSlots<'_, '_>,
            &mut Writer<'_, '_>,
        ) -> Result<()>,
    ) -> (Result<()>, usize, usize, usize) {
        super::super::super::invocations::tests::run_variant(work, storage, false, |plan, out| {
            let source = plan.source(out)?;
            let owner = source.canonical(out.budget)?;
            let (inventory, receipt) =
                super::super::super::super::Inventory::derive_v18(owner, out.budget)?;
            out.budget.reserve_storage(receipt.retained_storage())?;
            let result = source.with_ranked_correspondence_v18(
                &inventory,
                out.budget,
                |relation, budget| {
                    let mut writer = Writer::new(budget)?;
                    let slots = SourceSlots::derive(plan, relation, &mut writer)?;
                    examine(plan, &slots, &mut writer)
                },
            );
            drop(inventory);
            if result.is_ok() {
                out.budget.release_storage(receipt.retained_storage())?;
            }
            result
        })
    }

    #[test]
    fn original_mir_byte_returns_use_exact_instance_locals_call_destinations_and_owners() {
        run(LIMIT, LIMIT, |plan, slots, out| {
            for root in 0..2 {
                for instance in 0..3 {
                    let row = plan.instance(root, instance, out)?;
                    let frame = SourceFrameReturn::derive(plan, slots, root, instance, out)?;
                    assert_eq!(frame.locals, row.locals);
                    assert_eq!(frame.owners.last().copied(), Some(row.function.index()));
                    if let Some((parent, site)) = row.incoming {
                        let source = plan.source(out)?.source_semantic(out.budget)?;
                        let parent = plan.instance(root, parent, out)?;
                        let call = match source.functions()[parent.function.index() as usize]
                            .blocks()[site.index() as usize]
                            .terminator()
                            .kind()
                        {
                            Terminator::Call(call) => call,
                            _ => panic!("direct fixture call"),
                        };
                        let destination = call.destination().unwrap();
                        assert_eq!(
                            frame.destination,
                            Some(
                                parent.locals.start + destination.place().local().index() as usize
                            )
                        );
                        assert_eq!(
                            frame.continuation,
                            Some(
                                parent.blocks.start + destination.edge().target().index() as usize
                            )
                        );
                        assert_eq!(
                            frame.owners,
                            vec![parent.function.index(), row.function.index()]
                        );
                    } else {
                        assert_eq!(frame.destination, None);
                        assert_eq!(frame.continuation, None);
                        assert_eq!(frame.owners.len(), 1);
                    }
                    frame.emit(out)?;
                }
            }
            assert_eq!(
                out.text.matches("-> InvocationSourceByteReturnV36").count(),
                6
            );
            assert!(
                out.text
                    .contains("source.machine.frames.active[0].invocation != 0")
            );
            assert!(!out.text.contains("target"));
            Ok(())
        })
        .0
        .unwrap();
    }

    #[test]
    fn original_mir_byte_returns_refuse_absent_root_or_instance_before_text() {
        run(LIMIT, LIMIT, |plan, slots, out| {
            assert!(SourceFrameReturn::derive(plan, slots, 2, 0, out).is_err());
            assert!(SourceFrameReturn::derive(plan, slots, 0, 99, out).is_err());
            assert!(out.text.is_empty());
            Ok(())
        })
        .0
        .unwrap();
    }

    #[test]
    fn original_mir_byte_return_prelude_keeps_complete_escape_census_and_source_only_lifetime() {
        let text = SOURCE_FRAMES_V36;
        let cleared = text
            .find("if begin <= i < end { MemoryValueV30::Undefined } else { source.machine.values[i] }")
            .unwrap();
        let checked = text.find("let escapes =").unwrap();
        let cleaned = text
            .find("let cleaned = InvocationSourceByteStateV36")
            .unwrap();
        let installed = text.find("let installed = match destination").unwrap();
        assert!(cleared < checked && checked < cleaned && cleaned < installed);
        assert!(text.contains(
            "invocation_source_return_install_v42(cleaned, destination, returned, little_endian)"
        ));
        assert!(text.contains("invocation_source_return_refused_v36(installed)"));
        assert!(text.contains("source: installed, returned"));
        assert!(text.contains("installed.machine.frames != cleaned.machine.frames"));
        assert!(text.contains("installed.machine.generations != cleaned.machine.generations"));
        assert!(text.contains("invocation_source_snapshot_escapes_frame_v42(returned, frame)"));
        assert!(text.contains("exists|i: int| 0 <= i < values.len()"));
        assert!(
            text.contains(
                "invocation_source_memory_escapes_frame_v37(source.machine.memory, frame)"
            )
        );
        assert!(text.contains("memory.live[allocation].relocations[at].pointer.allocation, frame"));
        assert!(text.contains("!byte_allocation_in_frame_v30(allocation, frame)"));
        assert!(text.contains("source.slots.dom().filter(|descriptor: int|\n                            !byte_allocation_in_frame_v30(source.slots[descriptor].allocation, frame))"));
        assert!(text.contains("|descriptor: int| source.slots[descriptor]"));
        assert!(text.contains("generations: source.machine.generations"));
        assert!(text.contains("byte_end_frame_v30(source.machine.memory, frame)"));
        assert!(text.contains("byte_pop_frame_v30(source.machine.frames)"));
        assert!(text.contains("!invocation_source_byte_state_well_formed_v36(source)"));
        assert!(!text.contains("target.memory"));
        assert!(!text.contains("target.frames"));
    }

    #[test]
    fn original_mir_byte_return_slice_guard_keeps_exact_metadata_width() {
        run(LIMIT, LIMIT, |plan, slots, out| {
            // Isolated emission control, not a fabricated admitted slice ABI.
            let mut frame = SourceFrameReturn::derive(plan, slots, 0, 1, out)?;
            for bits in [8, 16, 32, 64] {
                frame.class = ReturnClass::Slice(bits);
                frame.emit(out)?;
                assert!(out.text.contains(&format!(
                    "0 <= slice.length < memory_value_modulus_v30({})",
                    bits / 8
                )));
            }
            Ok(())
        })
        .0
        .unwrap();
    }

    #[test]
    fn original_mir_byte_returns_have_exact_and_one_short_resources() {
        let emit = |plan: &InvocationPlan<'_, '_>,
                    slots: &SourceSlots<'_, '_>,
                    out: &mut Writer<'_, '_>| {
            for instance in 0..3 {
                SourceFrameReturn::derive(plan, slots, 0, instance, out)?.emit(out)?;
            }
            Ok(())
        };
        let measured = run(LIMIT, LIMIT, emit);
        measured.0.unwrap();
        run(measured.1, measured.3, emit).0.unwrap();
        assert!(run(measured.1 - 1, measured.3, emit).0.is_err());
        assert!(run(measured.1, measured.3 - 1, emit).0.is_err());
    }

    #[test]
    fn original_mir_byte_return_headers_have_independent_field_envelope() {
        type Fields<'a, 'v, 's> = (
            &'a SourceSlots<'v, 's>,
            usize,
            usize,
            Vec<u32>,
            Vec<usize>,
            Vec<Recipe>,
            Option<Recipe>,
            Range<usize>,
            Option<usize>,
            Option<usize>,
            Option<DestinationComponent>,
            Option<(Access, usize)>,
            Option<usize>,
            ReturnClass,
            usize,
        );
        fn h<T>() -> usize {
            size_of::<T>() + 2 * size_of::<Result<T>>()
        }
        assert_eq!(
            size_of::<SourceFrameReturn<'_, '_, '_>>(),
            size_of::<Fields<'_, '_, '_>>()
        );
        assert_eq!(
            headers(),
            size_of::<Fields<'_, '_, '_>>()
                + 2 * size_of::<Result<SourceFrameReturn<'_, '_, '_>>>()
                + h::<Vec<u32>>()
                + h::<Vec<usize>>()
                + h::<Vec<Recipe>>()
                + h::<Option<Recipe>>()
                + descriptor_helpers::header_oracle()
                + h::<Range<usize>>()
                + h::<ReturnClass>()
                + h::<Option<DestinationComponent>>()
                + h::<Option<(Access, usize)>>()
                + h::<Option<usize>>()
                + 24 * size_of::<usize>()
                + 24 * size_of::<&()>()
        );
    }
}
