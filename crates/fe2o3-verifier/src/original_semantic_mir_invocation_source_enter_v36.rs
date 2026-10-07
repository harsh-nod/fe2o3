//! Source entry lifetimes come from the archive, never from target Alloca order.

use super::super::{Function, LocalRole, ScalarV30, Shape, Statement, invocations::InvocationPlan};
use super::source_bytes::{
    descriptor_helpers, descriptor_loans::Recipe, execution_loans, execution_transfer,
};
use super::{Error, Resource, Result, Writer, slots::SourceSlots, vector};
use fe2o3_mir_model::semantic_mir_v1::SemanticPointerMetadataV1 as Metadata;
use std::{fmt::Write as _, mem::size_of, ops::Range};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Class {
    Execution(execution_loans::Recipe),
    ExecutionTransfer(execution_transfer::Transfer),
    Scalar(u32),
    Pointer,
    Slice(u32),
    Aggregate(u32),
    Product(u32),
    Enum(u32),
    Descriptor(Recipe),
}

#[derive(Clone, Copy, Debug)]
struct Argument {
    local: usize,
    class: Class,
    descriptor: Option<usize>,
    bytes: u64,
    alignment: u32,
    object: bool,
}

#[derive(Clone, Copy, Debug)]
struct Slot {
    descriptor: usize,
    local: usize,
    implicit: bool,
    object: bool,
}

pub(super) struct SourceFrameEnter<'slots, 'view, 'source> {
    slots: &'slots SourceSlots<'view, 'source>,
    root: usize,
    instance: usize,
    owner: u32,
    owners: Vec<u32>,
    locals: Range<usize>,
    before: usize,
    entry: usize,
    arguments: Vec<Option<Argument>>,
    allocations: Vec<Slot>,
    required: usize,
}

fn mismatch() -> Error {
    Error::Statement("original MIR byte frame entry differs from its exact invocation")
}

type EntryCoordinates = (usize, usize, Option<u32>, Option<(usize, u32)>, Option<u32>);

fn entry_error(
    (root, instance, function, argument, local): EntryCoordinates,
    phase: &'static str,
    error: Error,
) -> Error {
    match error {
        Error::Statement("generated source limit") => error,
        Error::Statement(reason) => Error::SourceEntry {
            root,
            instance,
            function,
            argument,
            local,
            phase,
            reason,
        },
        other => other,
    }
}

fn entry_diagnostic_headers() -> usize {
    fn h<T>() -> usize {
        size_of::<T>() + 2 * size_of::<Result<T>>()
    }
    h::<EntryCoordinates>() + h::<&'static str>() + h::<Error>()
}

fn unsupported() -> Error {
    Error::Statement("original MIR byte argument lifetime or payload is not modeled")
}

type EntryObjectSite = (
    usize,
    usize,
    fe2o3_mir_model::semantic_mir_v1::SemanticFunctionIdV1,
    u32,
    fe2o3_mir_model::semantic_mir_v1::SemanticTypeIdV1,
);

fn entry_object_slot<'a>(
    slots: &'a SourceSlots<'_, '_>,
    (root, instance, function, local, ty): EntryObjectSite,
    class: Class,
    explicit: bool,
    out: &mut Writer<'_, '_>,
) -> Result<(
    usize,
    &'a fe2o3_lower_mir_kernel::ProductionSourceAllocationFrameV32,
)> {
    // Entry is an authenticated activation origin, not a choice of
    // one lifetime from a local's generation roster.
    let activation = slots
        .object_activation(root, instance, local, 0, out)?
        .ok_or_else(unsupported)?;
    out.budget.charge_work(14)?;
    if activation.origin != fe2o3_lower_mir_kernel::ProductionSourceObjectActivationV40::Entry
        || activation.ty != ty
        || explicit
        || !matches!(class, Class::Scalar(bits) if bits != 0)
    {
        return Err(unsupported());
    }
    let (descriptor, frame) = slots.descriptor_by_source(root, instance, local, Some(0), out)?;
    if !matches!(class, Class::Scalar(bits) if frame.bytes() == u64::from(bits.div_ceil(8))) {
        return Err(unsupported());
    }
    if descriptor != activation.descriptor
        || frame.root() != root
        || frame.instance() != instance
        || frame.function() != function
        || frame.local() != local
        || frame.semantic_type() != ty
        || frame.source_generation() != Some(0)
    {
        return Err(mismatch());
    }
    Ok((descriptor, frame))
}

fn storage_markers(function: &Function, out: &mut Writer<'_, '_>) -> Result<Vec<bool>> {
    let mut explicit = vector(function.locals().len(), out)?;
    out.budget.charge_work(function.locals().len())?;
    explicit.resize(function.locals().len(), false);
    for block in function.blocks() {
        out.budget.charge_work(1)?;
        for statement in block.statements() {
            out.budget.charge_work(1)?;
            if let Statement::StorageLive(local) | Statement::StorageDead(local) = statement.kind()
            {
                *explicit
                    .get_mut(local.index() as usize)
                    .ok_or_else(mismatch)? = true;
            }
        }
    }
    Ok(explicit)
}

impl<'slots, 'view, 'source> SourceFrameEnter<'slots, 'view, 'source> {
    pub(super) fn step_proof_hints(
        &self,
        out: &mut Writer<'_, '_>,
    ) -> Result<super::source_function::SourceEntryHintsV85> {
        self.slots
            .check_query_storage_floor(self.required, out.budget)?;
        out.budget.reserve_storage(step_hint_headers())?;
        if !self.heap_conservation_shape(out)? {
            return Err(mismatch());
        }
        let mut arguments = vector(self.arguments.len(), out)?;
        for argument in &self.arguments {
            out.budget.charge_work(1)?;
            arguments.push(argument.ok_or_else(mismatch)?.local);
        }
        Ok(super::source_function::SourceEntryHintsV85 {
            owner: self.owner,
            locals: self.locals.clone(),
            pc: self.entry,
            arguments,
        })
    }

    pub(super) fn heap_conservation_shape(&self, out: &mut Writer<'_, '_>) -> Result<bool> {
        let source = self.slots.correspondence(out)?.source(out.budget)?;
        if out.budget.storage() < self.required {
            return Err(source
                .retain_query_resource_error_v18(Resource::Accounting)
                .into());
        }
        out.budget.charge_work(
            self.arguments
                .len()
                .checked_add(1)
                .ok_or(Resource::Arithmetic)?,
        )?;
        Ok(self.allocations.is_empty()
            && self.arguments.iter().all(|argument| {
                matches!(
                    argument,
                    Some(Argument {
                        class: Class::Scalar(_),
                        descriptor: None,
                        ..
                    })
                )
            }))
    }

    pub(super) fn entry(&self) -> usize {
        self.entry
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
            return Err(entry_error(
                (root, instance, None, None, None),
                "source-entry-owner",
                mismatch(),
            ));
        }
        let row = plan.instance(root, instance, out)?;
        let semantic = source.source_semantic(out.budget)?;
        let function = semantic
            .functions()
            .get(row.function.index() as usize)
            .ok_or_else(|| {
                entry_error(
                    (root, instance, Some(row.function.index()), None, None),
                    "source-entry-function",
                    mismatch(),
                )
            })?;
        out.budget.charge_work(4)?;
        if !row.active
            || row.locals.len() != function.locals().len()
            || row.blocks.len() != function.blocks().len()
            || function.entry().index() as usize >= row.blocks.len()
        {
            return Err(entry_error(
                (root, instance, Some(row.function.index()), None, None),
                "source-entry-instance-ranges",
                mismatch(),
            ));
        }
        let entry = row
            .blocks
            .start
            .checked_add(function.entry().index() as usize)
            .ok_or(Resource::Arithmetic)?;
        let before = match row.incoming {
            Some((parent, block)) if parent < instance => {
                let parent = plan.instance(root, parent, out)?;
                if !parent.active || block.index() as usize >= parent.blocks.len() {
                    return Err(mismatch());
                }
                parent
                    .blocks
                    .start
                    .checked_add(block.index() as usize)
                    .ok_or(Resource::Arithmetic)?
            }
            None if instance == 0 => entry,
            _ => return Err(mismatch()),
        };
        let explicit = storage_markers(function, out)?;
        let range = slots.instance_descriptors(root, instance, out)?;
        let mut allocations = vector(range.len(), out)?;
        for position in range {
            let (descriptor, slot) = slots.descriptor_in_source_order(position, out)?;
            out.budget.charge_work(5)?;
            if slot.root() != root || slot.instance() != instance || slot.function() != row.function
            {
                return Err(unsupported());
            }
            let local = slot.local() as usize;
            let declaration = function.locals().get(local).ok_or_else(mismatch)?;
            if declaration.ty() != slot.semantic_type() {
                return Err(mismatch());
            }
            let object = slot.source_generation().is_some();
            // Entry recipes catalogue identities; explicit StorageLive markers
            // still determine when the original storage becomes active.
            let implicit = if object {
                !*explicit.get(local).ok_or_else(mismatch)?
                    && slots
                        .object_activation(root, instance, slot.local(), 0, out)?
                        .is_some_and(|activation| activation.descriptor == descriptor)
            } else {
                !*explicit.get(local).ok_or_else(mismatch)?
            };
            allocations.push(Slot {
                descriptor,
                local: row
                    .locals
                    .start
                    .checked_add(local)
                    .ok_or(Resource::Arithmetic)?,
                implicit,
                object,
            });
        }
        let inputs = function.abi().source_input_types();
        let mut arguments = vector(inputs.len(), out)?;
        out.budget.charge_work(inputs.len())?;
        arguments.resize(inputs.len(), None);
        for (local, declaration) in function.locals().iter().enumerate() {
            out.budget.charge_work(1)?;
            let LocalRole::Argument(argument) = declaration.role() else {
                continue;
            };
            out.budget.charge_work(5)?;
            if inputs.get(argument as usize) != Some(&declaration.ty()) {
                return Err(entry_error(
                    (
                        root,
                        instance,
                        Some(row.function.index()),
                        Some((argument as usize, declaration.ty().index())),
                        Some(u32::try_from(local).map_err(|_| Resource::Arithmetic)?),
                    ),
                    "source-entry-argument-type",
                    mismatch(),
                ));
            }
            let ty = semantic
                .types()
                .get(declaration.ty().index() as usize)
                .ok_or_else(mismatch)?;
            let class = if slots.is_product_v282(declaration.ty(), out)? {
                if instance == 0 || row.incoming.is_none() {
                    return Err(unsupported());
                }
                Class::Product(declaration.ty().index())
            } else if let Some(recipe) = execution_loans::entry_recipe_exact(
                slots,
                plan,
                root,
                instance,
                fe2o3_mir_model::semantic_mir_v1::SemanticLocalIdV1::from_index(
                    u32::try_from(local).map_err(|_| Resource::Arithmetic)?,
                ),
                out,
            )? {
                if recipe.mutable {
                    Class::ExecutionTransfer(execution_transfer::Transfer::for_entry(
                        slots,
                        plan,
                        root,
                        instance,
                        argument as usize,
                        out,
                    )?)
                } else {
                    Class::Execution(recipe)
                }
            } else if let Some(recipe) = descriptor_helpers::entry_recipe(
                slots,
                plan,
                root,
                instance,
                fe2o3_mir_model::semantic_mir_v1::SemanticLocalIdV1::from_index(
                    u32::try_from(local).map_err(|_| Resource::Arithmetic)?,
                ),
                out,
            )? {
                Class::Descriptor(recipe)
            } else {
                match ty.shape() {
                    Shape::Enum { .. } => {
                        out.budget.charge_work(2)?;
                        if instance == 0 || row.incoming.is_none() {
                            return Err(unsupported());
                        }
                        Class::Enum(declaration.ty().index())
                    }
                    Shape::Pointer(pointer) => {
                        if slots.witness_class(pointer.pointee(), out)?.is_some() {
                            return Err(unsupported());
                        }
                        match pointer.metadata() {
                            Metadata::None => Class::Pointer,
                            Metadata::SliceLength => {
                                Class::Slice(super::source_bytes::slice_metadata_bits_v36(ty, out)?)
                            }
                            _ => return Err(unsupported()),
                        }
                    }
                    _ => match slots.descriptor_slice_bits(declaration.ty(), out)? {
                        Some(bits) => Class::Slice(bits),
                        None => {
                            if matches!(
                                ty.shape(),
                                Shape::Tuple(_) | Shape::Aggregate(_) | Shape::Array { .. }
                            ) && slots.aggregate_leaf_count(declaration.ty(), out)?.is_some()
                            {
                                Class::Aggregate(declaration.ty().index())
                            } else {
                                Class::Scalar(
                                    ScalarV30::from_source(semantic.types(), declaration.ty())?
                                        .width(),
                                )
                            }
                        }
                    },
                }
            };
            let original_local = u32::try_from(local).map_err(|_| Resource::Arithmetic)?;
            let object = slots.has_original_object(root, instance, original_local, out)?;
            let slot = if object {
                Some(
                    entry_object_slot(
                        slots,
                        (
                            root,
                            instance,
                            row.function,
                            original_local,
                            declaration.ty(),
                        ),
                        class,
                        explicit[local],
                        out,
                    )
                    .map_err(|error| match error {
                        Error::Statement("generated source limit") => error,
                        Error::Statement(reason) => Error::SourceDescriptor {
                            root,
                            instance,
                            function: Some(row.function.index() as usize),
                            local: original_local,
                            phase: "source-enter-object-parameter",
                            reason,
                        },
                        other => other,
                    })?,
                )
            } else {
                slots.legacy_descriptor_by_source(
                    root,
                    instance,
                    original_local,
                    "source-enter-parameter",
                    out,
                )?
            };
            let (descriptor, bytes, alignment) = if let Some((descriptor, slot)) = slot {
                let Class::Scalar(bits) = class else {
                    return Err(unsupported());
                };
                if explicit[local] || bits == 0 || slot.bytes() != u64::from(bits.div_ceil(8)) {
                    return Err(unsupported());
                }
                (Some(descriptor), slot.bytes(), slot.alignment())
            } else {
                (None, 0, 0)
            };
            let argument_slot = arguments.get_mut(argument as usize).ok_or_else(mismatch)?;
            if argument_slot
                .replace(Argument {
                    local: row
                        .locals
                        .start
                        .checked_add(local)
                        .ok_or(Resource::Arithmetic)?,
                    class,
                    descriptor,
                    bytes,
                    alignment,
                    object,
                })
                .is_some()
            {
                return Err(entry_error(
                    (
                        root,
                        instance,
                        Some(row.function.index()),
                        Some((argument as usize, declaration.ty().index())),
                        Some(original_local),
                    ),
                    "source-entry-argument-duplicate",
                    mismatch(),
                ));
            }
        }
        out.budget.charge_work(arguments.len())?;
        if let Some(argument) = arguments.iter().position(Option::is_none) {
            return Err(entry_error(
                (
                    root,
                    instance,
                    Some(row.function.index()),
                    Some((argument, inputs[argument].index())),
                    None,
                ),
                "source-entry-argument-completeness",
                mismatch(),
            ));
        }
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
            if !ancestor.active || owners.len() == owners.capacity() {
                return Err(mismatch());
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
        let released = explicit
            .capacity()
            .checked_mul(size_of::<bool>())
            .ok_or(Resource::Arithmetic)?;
        drop(explicit);
        out.budget.release_storage(released)?;
        Ok(Self {
            slots,
            root,
            instance,
            owner: row.function.index(),
            owners,
            locals: row.locals.clone(),
            before,
            entry,
            arguments,
            allocations,
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
        write!(out, "spec fn invocation_source_active_{}_{}_v36(source: InvocationSourceByteStateV36) -> bool {{ source.machine.valid && invocation_source_byte_state_well_formed_v36(source) && source.machine.values.len() >= {} && source.machine.frames.active.len() == {} && source.machine.frames.active[0].invocation == 0", self.root, self.instance, self.locals.end, self.owners.len()).map_err(|_| out.error())?;
        for (i, owner) in self.owners.iter().enumerate() {
            out.budget.charge_work(1)?;
            write!(
                out,
                " && source.machine.frames.active[{i}].owner == {owner}"
            )
            .map_err(|_| out.error())?;
        }
        write!(out, " }}\n").map_err(|_| out.error())?;
        let before_depth = if self.instance == 0 {
            1
        } else {
            self.owners.len() - 1
        };
        out.budget.charge_work(3)?;
        write!(out, "spec fn invocation_source_entry_refuses_{}_{}_v167(source: InvocationSourceByteStateV36, arguments: Seq<InvocationSourceValueV42>, little_endian: bool) -> bool {{\n !source.machine.valid || !invocation_source_byte_state_well_formed_v36(source) || source.machine.pc != {} || source.machine.values.len() < {} || arguments.len() != {} || source.machine.frames.active.len() != {} || source.machine.frames.active[0].invocation != 0", self.root, self.instance, self.before, self.locals.end, self.arguments.len(), before_depth).map_err(|_| out.error())?;
        write!(
            out,
            " || (exists|local: int| {} <= local < {} && source.objects.contains_key(local))",
            self.locals.start, self.locals.end
        )
        .map_err(|_| out.error())?;
        for (i, owner) in self.owners[..before_depth].iter().enumerate() {
            out.budget.charge_work(1)?;
            write!(
                out,
                " || source.machine.frames.active[{i}].owner != {owner}"
            )
            .map_err(|_| out.error())?;
        }
        if self.instance == 0 {
            write!(
                out,
                " || source.machine.frames.next_invocation != 1 || source.slots.dom().len() != 0"
            )
            .map_err(|_| out.error())?;
        }
        for slot in &self.allocations {
            out.budget.charge_work(1)?;
            write!(out, " || source.slots.contains_key({})", slot.descriptor)
                .map_err(|_| out.error())?;
        }
        out.budget.charge_work(self.arguments.len())?;
        let transfers = self
            .arguments
            .iter()
            .filter(|argument| {
                matches!(
                    argument,
                    Some(Argument {
                        class: Class::ExecutionTransfer(_),
                        ..
                    })
                )
            })
            .count();
        write!(
            out,
            " || source.logical.execution_pending.dom().len() != {transfers}"
        )
        .map_err(|_| out.error())?;
        for (i, argument) in self.arguments.iter().enumerate() {
            out.budget.charge_work(1)?;
            write!(out, " || !(").map_err(|_| out.error())?;
            match argument.ok_or_else(mismatch)?.class {
                Class::ExecutionTransfer(transfer) => {
                    write!(out, "match arguments[{i}] {{ InvocationSourceValueV42::ExecutionTransfer(value) => invocation_source_execution_transfer_entry_v286(source, value, ").map_err(|_| out.error())?;
                    transfer.emit_site(out)?;
                    write!(out, ", ").map_err(|_| out.error())?;
                    transfer.operand.recipe.emit(out)?;
                    write!(out, "), _ => false }}")
                }
                Class::Execution(recipe) => {
                    write!(out, "match arguments[{i}] {{ InvocationSourceValueV42::Execution(value) => invocation_source_execution_snapshot_current_v170(source, value, ").map_err(|_| out.error())?;
                    recipe.emit(out)?;
                    write!(out, "), _ => false }}")
                }
                Class::Descriptor(recipe) => {
                    write!(out, "match arguments[{i}] {{ InvocationSourceValueV42::Descriptor(value) => invocation_source_descriptor_snapshot_current_v53(source, value, ").map_err(|_| out.error())?;
                    recipe.emit(out)?;
                    write!(out, "), _ => false }}")
                }
                Class::Scalar(bits) => write!(out, "match arguments[{i}] {{ InvocationSourceValueV42::Carrier(value) => invocation_source_byte_value_typed_v36(value, {bits}), _ => false }}"),
                Class::Pointer => write!(out, "match arguments[{i}] {{ InvocationSourceValueV42::Carrier(MemoryValueV30::Pointer(_)) => true, _ => false }}"),
                Class::Slice(bits) => write!(out, "match arguments[{i}] {{ InvocationSourceValueV42::Carrier(MemoryValueV30::Slice(slice)) => 0 <= slice.length < memory_value_modulus_v30({}), _ => false }}", bits / 8),
                Class::Aggregate(ty) => write!(out, "match arguments[{i}] {{ InvocationSourceValueV42::Aggregate(value) => value.source_type == {ty} && invocation_source_aggregate_complete_v42(value), _ => false }}"),
                Class::Product(ty) => write!(out, "match arguments[{i}] {{ InvocationSourceValueV42::Product(value) => value.source_type == {ty} && invocation_source_product_complete_v282(value) && invocation_source_product_current_v282(source, value, little_endian), _ => false }}"),
                Class::Enum(ty) => write!(out, "match arguments[{i}] {{ InvocationSourceValueV42::Enum(value) => value.source_type == {ty} && invocation_source_enum_snapshot_current_v50(source, value, little_endian), _ => false }}"),
            }.map_err(|_| out.error())?;
            write!(out, ")").map_err(|_| out.error())?;
        }
        write!(out, "\n}}\nspec fn invocation_source_entry_body_{}_{}_v167(source: InvocationSourceByteStateV36, arguments: Seq<InvocationSourceValueV42>, little_endian: bool) -> InvocationSourceByteStateV36 {{\n let entered = invocation_source_entry_initialize_v166(source, {}, {}, {}, ", self.root, self.instance, self.entry, self.locals.start, self.locals.end).map_err(|_| out.error())?;
        if self.instance == 0 {
            write!(out, "source.machine.frames").map_err(|_| out.error())?;
        } else {
            write!(
                out,
                "byte_enter_frame_v30(source.machine.frames, {})",
                self.owner
            )
            .map_err(|_| out.error())?;
        }
        write!(out, ");\n").map_err(|_| out.error())?;
        for slot in &self.allocations {
            out.budget.charge_work(1)?;
            if slot.implicit {
                if slot.object {
                    write!(out, " let entered = invocation_source_object_activate_v40(entered, {}, invocation_source_slot_{}_v36(), {}, 0int, {}, {});\n", slot.descriptor, slot.descriptor, slot.local, self.root, self.instance).map_err(|_| out.error())?;
                } else {
                    write!(out, " let entered = invocation_source_byte_activate_v36(entered, {}, invocation_source_slot_{}_v36(), {}, {}, {});\n", slot.descriptor, slot.descriptor, slot.local, self.root, self.instance).map_err(|_| out.error())?;
                }
            }
        }
        for (i, argument) in self.arguments.iter().enumerate() {
            out.budget.charge_work(1)?;
            let argument = argument.ok_or_else(mismatch)?;
            if let Class::ExecutionTransfer(transfer) = argument.class {
                write!(out, " let entered = match arguments[{i}] {{ InvocationSourceValueV42::ExecutionTransfer(value) => invocation_source_execution_transfer_install_v286(entered, value, ").map_err(|_| out.error())?;
                transfer.emit_site(out)?;
                write!(out, ", ").map_err(|_| out.error())?;
                transfer.operand.recipe.emit(out)?;
                write!(
                    out,
                    "), _ => invocation_source_byte_refused_v36(entered) }};\n"
                )
                .map_err(|_| out.error())?;
                continue;
            }
            if let Class::Execution(recipe) = argument.class {
                write!(out, " let entered = match arguments[{i}] {{ InvocationSourceValueV42::Execution(value) => invocation_source_execution_snapshot_install_v170(entered, {}, ", argument.local).map_err(|_| out.error())?;
                recipe.emit(out)?;
                write!(
                    out,
                    ", value), _ => invocation_source_byte_refused_v36(entered) }};\n"
                )
                .map_err(|_| out.error())?;
                continue;
            }
            if let Class::Descriptor(recipe) = argument.class {
                write!(out, " let entered = match arguments[{i}] {{ InvocationSourceValueV42::Descriptor(value) => invocation_source_descriptor_snapshot_install_v53(entered, {}, ", argument.local).map_err(|_| out.error())?;
                recipe.emit(out)?;
                write!(
                    out,
                    ", value), _ => invocation_source_byte_refused_v36(entered) }};\n"
                )
                .map_err(|_| out.error())?;
                continue;
            }
            if matches!(argument.class, Class::Enum(_)) {
                write!(out, " let entered = match arguments[{i}] {{ InvocationSourceValueV42::Enum(value) => if invocation_source_enum_snapshot_current_v50(entered, value, little_endian) {{ invocation_source_enum_install_v47(entered, {}, value) }} else {{ invocation_source_byte_refused_v36(entered) }}, _ => invocation_source_byte_refused_v36(entered) }};\n", argument.local).map_err(|_| out.error())?;
                continue;
            }
            if matches!(argument.class, Class::Aggregate(_)) {
                write!(out, " let entered = match arguments[{i}] {{ InvocationSourceValueV42::Aggregate(value) => invocation_source_aggregate_install_v42(entered, {}, value), _ => invocation_source_byte_refused_v36(entered) }};\n", argument.local).map_err(|_| out.error())?;
                continue;
            }
            if matches!(argument.class, Class::Product(_)) {
                write!(out, " let entered = match arguments[{i}] {{ InvocationSourceValueV42::Product(value) => invocation_source_product_install_v282(entered, {}, value, little_endian), _ => invocation_source_byte_refused_v36(entered) }};\n", argument.local).map_err(|_| out.error())?;
                continue;
            }
            write!(out, " let argument_{i} = match arguments[{i}] {{ InvocationSourceValueV42::Carrier(value) => value, _ => MemoryValueV30::Undefined }};\n").map_err(|_| out.error())?;
            if let Some(descriptor) = argument.descriptor {
                if argument.object {
                    write!(out, " let entered = match invocation_source_byte_address_v36(entered, InvocationSourceByteAccessV36 {{ base: InvocationSourceByteBaseV36::ObjectLocal({}), offset: 0, width: {}, alignment: {} }}, {}, {}) {{ Some(pointer) => InvocationSourceByteStateV36 {{ machine: invocation_source_store_v36(entered.machine, pointer, {}, {}, argument_{i}, little_endian), ..entered }}, None => invocation_source_byte_refused_v36(entered) }};\n", argument.local, argument.bytes, argument.alignment, self.root, self.instance, argument.bytes, argument.alignment).map_err(|_| out.error())?;
                } else {
                    write!(out, " let entered = match invocation_source_byte_slot_v36(entered, {descriptor}, invocation_source_slot_{descriptor}_v36(), {}, {}) {{ Some(pointer) => InvocationSourceByteStateV36 {{ machine: invocation_source_store_v36(entered.machine, pointer, {}, {}, argument_{i}, little_endian), ..entered }}, None => invocation_source_byte_refused_v36(entered) }};\n", self.root, self.instance, argument.bytes, argument.alignment).map_err(|_| out.error())?;
                }
            } else {
                write!(out, " let entered = invocation_source_byte_put_local_v36(entered, {}, argument_{i});\n", argument.local).map_err(|_| out.error())?;
            }
        }
        write!(out, " if entered.logical.execution_pending.dom().len() == 0 {{ entered }} else {{ invocation_source_byte_refused_v36(entered) }}\n}}\nspec fn invocation_source_enter_{0}_{1}_v36(source: InvocationSourceByteStateV36, arguments: Seq<InvocationSourceValueV42>, little_endian: bool) -> InvocationSourceByteStateV36 {{\n invocation_source_entry_select_v167(source, invocation_source_entry_refuses_{0}_{1}_v167(source, arguments, little_endian), invocation_source_entry_body_{0}_{1}_v167(source, arguments, little_endian))\n}}\n", self.root, self.instance).map_err(|_| out.error())
    }
}

fn step_hint_headers() -> usize {
    fn h<T>() -> usize {
        size_of::<T>() + 2 * size_of::<Result<T>>()
    }
    h::<super::source_function::SourceEntryHintsV85>()
        + h::<Vec<usize>>()
        + h::<Argument>()
        + h::<Range<usize>>()
        + size_of::<std::slice::Iter<'_, Option<Argument>>>()
        + 2 * size_of::<usize>()
        + 2 * size_of::<&()>()
}

fn headers() -> usize {
    fn h<T>() -> usize {
        size_of::<T>() + 2 * size_of::<Result<T>>()
    }
    h::<SourceFrameEnter<'_, '_, '_>>()
        + h::<Vec<bool>>()
        + h::<Vec<u32>>()
        + h::<Vec<Slot>>()
        + h::<Vec<Option<Argument>>>()
        + h::<Slot>()
        + h::<Option<Argument>>()
        + h::<Range<usize>>()
        + h::<Class>()
        + descriptor_helpers::headers()
        + execution_loans::headers()
        + execution_transfer::headers()
        + h::<super::slots::ObjectActivation>()
        + h::<Option<super::slots::ObjectActivation>>()
        + h::<(
            usize,
            &fe2o3_lower_mir_kernel::ProductionSourceAllocationFrameV32,
        )>()
        + h::<EntryObjectSite>()
        + entry_diagnostic_headers()
        + 32 * size_of::<usize>()
        + 24 * size_of::<&()>()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mixed_optimizer_refinement_v26::SOURCE_LIMIT;
    const LIMIT: usize = 100_000_000;

    #[test]
    fn original_mir_entry_refusal_keeps_argument_coordinates_and_first_typed_error() {
        use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
        macro_rules! h {
            ($ty:ty) => {
                size_of::<$ty>() + 2 * size_of::<Result<$ty>>()
            };
        }
        assert_eq!(
            entry_diagnostic_headers(),
            h!((usize, usize, Option<u32>, Option<(usize, u32)>, Option<u32>))
                + h!(&'static str)
                + h!(Error)
        );
        let coordinates = (2, 5, Some(7), Some((1, 13)), None);
        let phase = "source-entry-argument-completeness";
        assert!(matches!(
            entry_error(coordinates, phase, mismatch()),
            Error::SourceEntry {
                root: 2,
                instance: 5,
                function: Some(7),
                argument: Some((1, 13)),
                local: None,
                phase: "source-entry-argument-completeness",
                reason: "original MIR byte frame entry differs from its exact invocation",
            }
        ));
        for error in [
            Error::Statement("generated source limit"),
            Error::GeneratedSourceLimit {
                section: "earlier-source-section",
                emitted_bytes: 17,
                limit_bytes: SOURCE_LIMIT,
            },
            Error::Resource(Resource::Accounting),
            Error::Source(
                fe2o3_lower_mir_kernel::ProductionSourceOwnedViewErrorV18::Resource(
                    Resource::Accounting,
                ),
            ),
        ] {
            let before = format!("{error:?}");
            assert_eq!(
                format!("{:?}", entry_error(coordinates, phase, error)),
                before
            );
        }
        let mut work = Work::new(0);
        let error = work.charge_work(1).unwrap_err();
        assert!(matches!(
            entry_error(coordinates, phase, Error::Resource(Resource::Work(error))),
            Error::Resource(Resource::Work(found))
                if found.actual() == 1 && found.limit() == 0
        ));
    }

    fn run_object_arguments(
        work: usize,
        storage: usize,
        examine: impl FnOnce(
            &InvocationPlan<'_, '_>,
            &SourceSlots<'_, '_>,
            &mut Writer<'_, '_>,
        ) -> Result<()>,
    ) -> (Result<()>, usize, usize, usize) {
        use fe2o3_mir_model::semantic_mir_v1::*;
        super::super::super::invocations::tests::run_source_transform(
            work,
            storage,
            |types, functions| {
                let word = SemanticTypeIdV1::from_index(0);
                let raw = SemanticTypeIdV1::from_index(types.len() as u32);
                types.push(SemanticTypeDeclV1::new(
                    SemanticTypeIdentityV1::from_sha256([230; 32]),
                    SemanticLayoutIdentityV1::from_sha256([231; 32]),
                    SemanticTypeLayoutV1::new_with_backend_repr(
                        Some(8),
                        8,
                        SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                            SemanticBackendPrimitiveV1::pointer(0, 8, 8),
                            SemanticScalarValidityRangeV1::new(0, u64::MAX.into()),
                        )),
                        false,
                    )
                    .unwrap(),
                    SemanticTypeShapeV1::Pointer(
                        SemanticPointerTypeV1::new_with_kind(
                            word,
                            SemanticPointerKindV1::Raw,
                            SemanticMutabilityV1::Mutable,
                            0,
                            64,
                            SemanticPointerMetadataV1::None,
                        )
                        .unwrap(),
                    ),
                ));
                for (ordinal, function) in functions.iter_mut().enumerate() {
                    let prior = &*function;
                    let source = prior.source();
                    let mut locals = prior.locals().to_vec();
                    let pointer = SemanticLocalIdV1::from_index(locals.len() as u32);
                    assert_eq!(locals[1].role(), SemanticLocalRoleV1::Argument(0));
                    assert_eq!(locals[1].ty(), word);
                    locals.push(SemanticLocalDeclV1::new(
                        SemanticLocalIdentityV1::from_sha256([232 + ordinal as u8; 32]),
                        raw,
                        SemanticLocalRoleV1::Temporary,
                        source,
                    ));
                    let mut blocks = prior.blocks().to_vec();
                    let entry = prior.entry().index() as usize;
                    let mut statements = vec![SemanticStatementV1::new(
                        source,
                        SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                            SemanticPlaceV1::new(pointer, vec![], raw).unwrap(),
                            SemanticRvalueV1::new(
                                raw,
                                SemanticRvalueKindV1::AddressOf {
                                    place: SemanticPlaceV1::new(
                                        SemanticLocalIdV1::from_index(1),
                                        vec![],
                                        word,
                                    )
                                    .unwrap(),
                                    mutability: SemanticMutabilityV1::Mutable,
                                },
                            ),
                        )),
                    )];
                    statements.extend_from_slice(blocks[entry].statements());
                    blocks[entry] = SemanticBasicBlockV1::new(
                        blocks[entry].identity(),
                        blocks[entry].source(),
                        statements,
                        blocks[entry].terminator().clone(),
                    )
                    .unwrap();
                    let mut rebuilt = SemanticFunctionDeclV1::new(
                        prior.identity(),
                        prior.role(),
                        prior.item_definition_identity(),
                        prior.monomorphization_identity(),
                        prior.generic_type_arguments_identity(),
                        prior.const_generic_arguments_identity(),
                        source,
                        prior.abi().clone(),
                        locals,
                        prior.entry(),
                        blocks,
                    )
                    .unwrap();
                    if let Some(kernel) = prior.kernel_entry() {
                        rebuilt = rebuilt.with_kernel_entry(kernel.clone());
                    }
                    *function = rebuilt;
                }
            },
            |plan, out| {
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
            },
        )
    }

    #[test]
    fn original_mir_byte_entry_installs_exact_scalar_entry_objects() {
        let result = run_object_arguments(LIMIT, LIMIT, |plan, slots, out| {
            for root in 0..2 {
                for instance in 0..3 {
                    let row = plan.instance(root, instance, out)?;
                    let activation = slots.object_activation(root, instance, 1, 0, out)?.unwrap();
                    assert_eq!(
                        activation.origin,
                        fe2o3_lower_mir_kernel::ProductionSourceObjectActivationV40::Entry
                    );
                    let entry = SourceFrameEnter::derive(plan, slots, root, instance, out)?;
                    let argument = entry.arguments[0].unwrap();
                    assert!(argument.object);
                    assert_eq!(argument.class, Class::Scalar(32));
                    assert_eq!(argument.local, row.locals.start + 1);
                    assert_eq!(argument.descriptor, Some(activation.descriptor));
                    assert_eq!((argument.bytes, argument.alignment), (4, 4));
                    assert_eq!(
                        entry
                            .allocations
                            .iter()
                            .filter(|slot| slot.descriptor == activation.descriptor
                                && slot.object
                                && slot.implicit
                                && slot.local == argument.local)
                            .count(),
                        1
                    );
                    assert!(!entry.heap_conservation_shape(out)?);
                    let before = out.text.len();
                    entry.emit(out)?;
                    let emitted = &out.text[before..];
                    let activate = format!(
                        "let entered = invocation_source_object_activate_v40(entered, {},",
                        activation.descriptor
                    );
                    let install = format!(
                        "let entered = match invocation_source_byte_address_v36(entered, InvocationSourceByteAccessV36 {{ base: InvocationSourceByteBaseV36::ObjectLocal({}), offset: 0, width: 4, alignment: 4 }}",
                        argument.local
                    );
                    assert_eq!(emitted.matches(&activate).count(), 1);
                    assert_eq!(emitted.matches(&install).count(), 1);
                    assert!(emitted.find(&activate).unwrap() < emitted.find(&install).unwrap());
                    assert!(!emitted.contains(&format!(
                        "invocation_source_byte_put_local_v36(entered, {}, argument_0)",
                        argument.local
                    )));
                }
            }
            Ok(())
        });
        result.0.unwrap();
        assert_eq!(result.2, super::super::super::invocations::tests::FLOOR);
        assert!(result.3 > result.2);
    }

    #[test]
    fn original_mir_byte_entry_object_exact_and_one_short_resource_replay() {
        use fe2o3_lower_mir_kernel::ProductionSourceOwnedViewErrorV18 as SourceError;
        let emit = |plan: &InvocationPlan<'_, '_>,
                    slots: &SourceSlots<'_, '_>,
                    out: &mut Writer<'_, '_>| {
            for instance in 0..3 {
                SourceFrameEnter::derive(plan, slots, 0, instance, out)?.emit(out)?;
            }
            Ok(())
        };
        let measured = run_object_arguments(LIMIT, LIMIT, emit);
        measured.0.unwrap();
        let exact = run_object_arguments(measured.1, measured.3, emit);
        exact.0.unwrap();
        assert_eq!(
            (exact.1, exact.2, exact.3),
            (measured.1, measured.2, measured.3)
        );
        assert_eq!(exact.2, super::super::super::invocations::tests::FLOOR);
        assert!(
            matches!(run_object_arguments(measured.1 - 1, measured.3, emit).0,
            Err(Error::Resource(Resource::Work(error)))
            | Err(Error::Source(SourceError::Resource(Resource::Work(error))))
            if error.actual() == measured.1 && error.limit() == measured.1 - 1)
        );
        assert!(
            matches!(run_object_arguments(measured.1, measured.3 - 1, emit).0,
            Err(Error::Resource(Resource::Storage(error)))
            | Err(Error::Source(SourceError::Resource(Resource::Storage(error))))
            if error.actual() == measured.3 && error.limit() == measured.3 - 1)
        );
    }

    #[test]
    fn original_mir_byte_entry_object_refuses_foreign_endpoints_and_nonscalar_payloads() {
        use fe2o3_mir_model::semantic_mir_v1::{SemanticFunctionIdV1, SemanticTypeIdV1};
        run_object_arguments(LIMIT, LIMIT, |plan, slots, out| {
            out.budget.reserve_storage(headers())?;
            let row = plan.instance(0, 0, out)?;
            let activation = slots.object_activation(0, 0, 1, 0, out)?.unwrap();
            let original = (0, 0, row.function, 1, activation.ty);
            let admitted = entry_object_slot(slots, original, Class::Scalar(32), false, out)?;
            assert_eq!(admitted.0, activation.descriptor);
            assert_eq!(admitted.1.source_generation(), Some(0));
            for class in [
                Class::Scalar(0),
                Class::Scalar(16),
                Class::Pointer,
                Class::Slice(64),
                Class::Aggregate(activation.ty.index()),
                Class::Product(activation.ty.index()),
                Class::Enum(activation.ty.index()),
            ] {
                assert!(matches!(
                    entry_object_slot(slots, original, class, false, out),
                    Err(Error::Statement(
                        "original MIR byte argument lifetime or payload is not modeled"
                    ))
                ));
            }
            assert!(matches!(
                entry_object_slot(slots, original, Class::Scalar(32), true, out),
                Err(Error::Statement(
                    "original MIR byte argument lifetime or payload is not modeled"
                ))
            ));
            for foreign in [
                (
                    0,
                    0,
                    SemanticFunctionIdV1::from_index(u32::MAX),
                    1,
                    activation.ty,
                ),
                (
                    0,
                    0,
                    row.function,
                    1,
                    SemanticTypeIdV1::from_index(u32::MAX),
                ),
                (0, 0, row.function, 99, activation.ty),
            ] {
                assert!(matches!(
                    entry_object_slot(slots, foreign, Class::Scalar(32), false, out),
                    Err(Error::Statement(_))
                ));
            }
            assert!(out.text.is_empty());
            Ok(())
        })
        .0
        .unwrap();
    }

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
    fn original_mir_byte_entry_uses_exact_argument_roles_dynamic_frames_and_local_ranges() {
        run(LIMIT, LIMIT, |plan, slots, out| {
            for root in 0..2 {
                for instance in 0..3 {
                    let row = plan.instance(root, instance, out)?;
                    let entry = SourceFrameEnter::derive(plan, slots, root, instance, out)?;
                    assert_eq!(entry.locals, row.locals);
                    assert_eq!(entry.owner, row.function.index());
                    assert_eq!(entry.owners.len(), if instance == 0 { 1 } else { 2 });
                    assert_eq!(entry.arguments.len(), 2);
                    assert_eq!(entry.arguments[0].unwrap().local, row.locals.start + 1);
                    let before = out.text.len();
                    entry.emit(out)?;
                    let emitted = &out.text[before..];
                    let frames = if instance == 0 {
                        "source.machine.frames".to_owned()
                    } else {
                        format!("byte_enter_frame_v30(source.machine.frames, {})", row.function.index())
                    };
                    assert!(emitted.contains(&format!(
                        " let entered = invocation_source_entry_initialize_v166(source, {}, {}, {}, {frames});",
                        entry.entry, row.locals.start, row.locals.end
                    )));
                    assert_eq!(emitted.matches("invocation_source_entry_initialize_v166(").count(), 1);
                    assert_eq!(emitted.matches("byte_enter_frame_v30(").count(), usize::from(instance != 0));
                    assert!(!emitted.contains("Seq::new("));
                }
            }
            assert_eq!(
                out.text
                    .matches("byte_enter_frame_v30(source.machine.frames,")
                    .count(),
                4
            );
            assert_eq!(
                out.text
                    .matches("source.machine.frames.next_invocation != 1")
                    .count(),
                2
            );
            assert!(!out.text.contains("target"));
            Ok(())
        })
        .0
        .unwrap();
    }

    #[test]
    fn shared_entry_initializer_retains_the_complete_original_record_equation() {
        let text = include_str!("original_semantic_mir_source_entry_initialize_v166.vrs");
        let (_, body) = text
            .split_once(") -> InvocationSourceByteStateV36 {\n")
            .unwrap();
        let (body, proof) = body.split_once("\nproof fn ").unwrap();
        assert_eq!(
            body,
            concat!(
                "    InvocationSourceByteStateV36 {\n",
                "        machine: MemoryStateV30 {\n",
                "            pc,\n",
                "            values: Seq::new(source.machine.values.len(), |i: int|\n",
                "                if begin <= i < end { MemoryValueV30::Undefined }\n",
                "                else { source.machine.values[i] }),\n",
                "            memory: source.machine.memory,\n",
                "            generations: source.machine.generations,\n",
                "            frames,\n",
                "            valid: true,\n",
                "        },\n",
                "        logical: invocation_source_logical_clear_v38(source.logical, begin, end),\n",
                "        ..source\n",
                "    }\n",
                "}\n",
            )
        );
        assert!(!text.contains("requires"));
        assert!(!text.contains("assume("));
        assert!(!text.contains("external_body"));
        let (proof, installed_pc) = proof
            .split_once("\n#[verifier::spinoff_prover]\nproof fn ")
            .unwrap();
        assert_eq!(
            proof,
            concat!(
                "invocation_source_entry_initialize_pc_v166(\n",
                "    source: InvocationSourceByteStateV36, pc: int, begin: int, end: int,\n",
                "    frames: MemoryFrameRuntimeV30,\n",
                ")\n",
                "    ensures invocation_source_entry_initialize_v166(source, pc, begin, end, frames).machine.pc == pc,\n",
                "{\n",
                "}\n",
            )
        );
        assert_eq!(
            installed_pc,
            concat!(
                "invocation_source_entry_put_local_pc_v179(\n",
                "    source: InvocationSourceByteStateV36, local: int, value: MemoryValueV30,\n",
                ")\n",
                "    ensures invocation_source_byte_put_local_v36(source, local, value).machine.pc\n",
                "        == source.machine.pc,\n",
                "{\n",
                "    hide(invocation_source_byte_state_well_formed_v36);\n",
                "    hide(invocation_source_logical_write_v38);\n",
                "}\n",
            )
        );
    }

    #[test]
    fn original_mir_byte_entry_refuses_absent_invocation_before_emission() {
        run(LIMIT, LIMIT, |plan, slots, out| {
            assert!(SourceFrameEnter::derive(plan, slots, 2, 0, out).is_err());
            assert!(SourceFrameEnter::derive(plan, slots, 0, 99, out).is_err());
            assert!(out.text.is_empty());
            Ok(())
        })
        .0
        .unwrap();
    }

    #[test]
    fn original_mir_byte_entry_exact_and_one_short_resource_replay() {
        let emit = |plan: &InvocationPlan<'_, '_>,
                    slots: &SourceSlots<'_, '_>,
                    out: &mut Writer<'_, '_>| {
            for instance in 0..3 {
                SourceFrameEnter::derive(plan, slots, 0, instance, out)?.emit(out)?;
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
    fn original_mir_byte_entry_storage_marker_census_has_independent_thirteen_work_boundary() {
        use fe2o3_kernel_ir::{
            CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
            CanonicalKernelIrWorkBudgetV1 as Work,
        };
        super::super::super::invocations::tests::run_scalar_lifetime_variant(
            LIMIT,
            LIMIT,
            |plan, out| {
                let semantic = plan.source(out)?.source_semantic(out.budget)?;
                let function =
                    &semantic.functions()[plan.instance(0, 0, out)?.function.index() as usize];
                assert_eq!(function.locals().len(), 5);
                assert_eq!(function.blocks().len(), 4);
                assert_eq!(
                    function
                        .blocks()
                        .iter()
                        .map(|block| block.statements().len())
                        .sum::<usize>(),
                    4
                );
                // Five initialization writes, four block visits, four statement visits.
                for (work_limit, extra, succeeds) in [(13, 5, true), (12, 5, false), (13, 4, false)]
                {
                    let mut work = Work::new(work_limit);
                    let floor = SOURCE_LIMIT + headers();
                    let mut budget = Budget::new(&mut work, floor + extra);
                    budget.reserve_storage(floor).unwrap();
                    let mut writer = Writer::new(&mut budget).unwrap();
                    let result = storage_markers(function, &mut writer);
                    if succeeds {
                        assert_eq!(result.unwrap(), vec![false, false, false, false, true]);
                        assert_eq!(writer.budget.work(), 5 + 4 + 4);
                        assert_eq!(writer.budget.storage(), floor + 5 * size_of::<bool>());
                    } else {
                        assert!(matches!(result, Err(Error::Resource(_))));
                    }
                    assert!(writer.text.is_empty());
                }
                Ok(())
            },
        )
        .0
        .unwrap();
    }
}
