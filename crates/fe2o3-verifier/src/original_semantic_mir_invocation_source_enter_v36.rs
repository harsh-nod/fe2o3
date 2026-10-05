//! Source entry lifetimes come from the archive, never from target Alloca order.

use super::super::{Function, LocalRole, ScalarV30, Shape, Statement, invocations::InvocationPlan};
use super::source_bytes::{descriptor_helpers, descriptor_loans::Recipe, execution_loans};
use super::{Error, Resource, Result, Writer, slots::SourceSlots, vector};
use fe2o3_mir_model::semantic_mir_v1::SemanticPointerMetadataV1 as Metadata;
use std::{fmt::Write as _, mem::size_of, ops::Range};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Class {
    Execution(execution_loans::Recipe),
    Scalar(u32),
    Pointer,
    Slice(u32),
    Aggregate(u32),
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

fn unsupported() -> Error {
    Error::Statement("original MIR byte argument lifetime or payload is not modeled")
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
            || function.entry().index() as usize >= row.blocks.len()
        {
            return Err(mismatch());
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
                return Err(mismatch());
            }
            let ty = semantic
                .types()
                .get(declaration.ty().index() as usize)
                .ok_or_else(mismatch)?;
            let class = if let Some(recipe) = execution_loans::entry_recipe(
                slots,
                plan,
                root,
                instance,
                fe2o3_mir_model::semantic_mir_v1::SemanticLocalIdV1::from_index(
                    u32::try_from(local).map_err(|_| Resource::Arithmetic)?,
                ),
                out,
            )? {
                Class::Execution(recipe)
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
            let slot = slots.legacy_descriptor_by_source(
                root,
                instance,
                u32::try_from(local).map_err(|_| Resource::Arithmetic)?,
                out,
            )?;
            if slots.has_original_object(
                root,
                instance,
                u32::try_from(local).map_err(|_| Resource::Arithmetic)?,
                out,
            )? {
                return Err(unsupported());
            }
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
                })
                .is_some()
            {
                return Err(mismatch());
            }
        }
        out.budget.charge_work(arguments.len())?;
        if arguments.iter().any(Option::is_none) {
            return Err(mismatch());
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
        write!(out, "spec fn invocation_source_enter_{}_{}_v36(source: InvocationSourceByteStateV36, arguments: Seq<InvocationSourceValueV42>, little_endian: bool) -> InvocationSourceByteStateV36 {{\n if !source.machine.valid || !invocation_source_byte_state_well_formed_v36(source) || source.machine.pc != {} || source.machine.values.len() < {} || arguments.len() != {} || source.machine.frames.active.len() != {} || source.machine.frames.active[0].invocation != 0", self.root, self.instance, self.before, self.locals.end, self.arguments.len(), before_depth).map_err(|_| out.error())?;
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
        for (i, argument) in self.arguments.iter().enumerate() {
            out.budget.charge_work(1)?;
            write!(out, " || !(").map_err(|_| out.error())?;
            match argument.ok_or_else(mismatch)?.class {
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
                Class::Enum(ty) => write!(out, "match arguments[{i}] {{ InvocationSourceValueV42::Enum(value) => value.source_type == {ty} && invocation_source_enum_snapshot_current_v50(source, value, little_endian), _ => false }}"),
            }.map_err(|_| out.error())?;
            write!(out, ")").map_err(|_| out.error())?;
        }
        write!(out, " {{ invocation_source_byte_refused_v36(source) }} else {{\n let entered = invocation_source_entry_initialize_v166(source, {}, {}, {}, ", self.entry, self.locals.start, self.locals.end).map_err(|_| out.error())?;
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
            write!(out, " let argument_{i} = match arguments[{i}] {{ InvocationSourceValueV42::Carrier(value) => value, _ => MemoryValueV30::Undefined }};\n").map_err(|_| out.error())?;
            if let Some(descriptor) = argument.descriptor {
                write!(out, " let entered = match invocation_source_byte_slot_v36(entered, {descriptor}, invocation_source_slot_{descriptor}_v36(), {}, {}) {{ Some(pointer) => InvocationSourceByteStateV36 {{ machine: invocation_source_store_v36(entered.machine, pointer, {}, {}, argument_{i}, little_endian), ..entered }}, None => invocation_source_byte_refused_v36(entered) }};\n", self.root, self.instance, argument.bytes, argument.alignment).map_err(|_| out.error())?;
            } else {
                write!(out, " let entered = invocation_source_byte_put_local_v36(entered, {}, argument_{i});\n", argument.local).map_err(|_| out.error())?;
            }
        }
        write!(out, " entered\n }}\n}}\n").map_err(|_| out.error())
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
        + h::<super::slots::ObjectActivation>()
        + h::<Option<super::slots::ObjectActivation>>()
        + 32 * size_of::<usize>()
        + 24 * size_of::<&()>()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mixed_optimizer_refinement_v26::SOURCE_LIMIT;
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
