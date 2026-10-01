//! Exact source invocation returns. Logical source lifetime never pops the
//! independent physical target allocation frame.

use super::super::{LocalRole, ScalarV30, Shape, Terminator, invocations::InvocationPlan};
use super::{Error, Resource, Result, Writer, slots::SourceSlots, vector};
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticCallableDeclV1 as Callable, SemanticEdgeRoleV1 as EdgeRole,
    SemanticPointerMetadataV1 as Metadata, SemanticUnwindActionV1 as Unwind,
};
use std::{fmt::Write as _, mem::size_of, ops::Range};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ReturnClass {
    Unit,
    Scalar(u32),
    Pointer,
    Slice(u32),
}

pub(super) struct SourceFrameReturn<'slots, 'view, 'source> {
    slots: &'slots SourceSlots<'view, 'source>,
    root: usize,
    instance: usize,
    owners: Vec<u32>,
    returns: Vec<usize>,
    locals: Range<usize>,
    returned: Option<usize>,
    destination: Option<usize>,
    continuation: Option<usize>,
    class: ReturnClass,
    required: usize,
}

fn mismatch() -> Error {
    Error::Statement("original MIR byte frame return differs from its exact invocation")
}

impl<'slots, 'view, 'source> SourceFrameReturn<'slots, 'view, 'source> {
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
        let class = match semantic
            .types()
            .get(ty.index() as usize)
            .map(|ty| ty.shape())
        {
            Some(Shape::Unit) => ReturnClass::Unit,
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
                None => ReturnClass::Scalar(ScalarV30::from_source(semantic.types(), ty)?.width()),
            },
        };
        let mut returned = None;
        for (local, declaration) in function.locals().iter().enumerate() {
            out.budget.charge_work(2)?;
            if declaration.role() == LocalRole::Return {
                if returned.is_some()
                    || declaration.ty() != ty
                    || slots
                        .legacy_descriptor_by_source(
                            root,
                            instance,
                            u32::try_from(local).map_err(|_| Resource::Arithmetic)?,
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
        for (block, declaration) in function.blocks().iter().enumerate() {
            out.budget.charge_work(1)?;
            if matches!(declaration.terminator().kind(), Terminator::Return) {
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
                || !destination.place().projections().is_empty()
                || destination.place().ty() != ty
                || caller
                    .locals()
                    .get(local as usize)
                    .is_none_or(|local| local.ty() != ty)
                || slots
                    .legacy_descriptor_by_source(root, parent, local, out)?
                    .is_some()
            {
                return Err(mismatch());
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
            locals: row.locals.clone(),
            returned,
            destination,
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
        write!(out, "open spec fn invocation_source_return_{}_{}_v36(source: InvocationSourceByteStateV36) -> InvocationSourceByteReturnV36 {{\n if !source.machine.valid || !byte_frame_runtime_well_formed_v30(source.machine.frames) || source.machine.frames.active.len() != {} || source.machine.frames.active[0].invocation != 0", self.root, self.instance, self.owners.len()).map_err(|_| out.error())?;
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
        if self.class != ReturnClass::Unit {
            let local = self.returned.ok_or_else(mismatch)?;
            write!(out, " || source.machine.values.len() <= {local} || !(")
                .map_err(|_| out.error())?;
            match self.class {
                ReturnClass::Scalar(bits) => write!(out, "invocation_source_byte_value_typed_v36(source.machine.values[{local}], {bits})"),
                ReturnClass::Pointer => write!(out, "match source.machine.values[{local}] {{ MemoryValueV30::Pointer(_) => true, _ => false }}"),
                ReturnClass::Slice(bits) => write!(out, "match source.machine.values[{local}] {{ MemoryValueV30::Slice(slice) => 0 <= slice.length < memory_value_modulus_v30({}), _ => false }}", bits / 8),
                ReturnClass::Unit => unreachable!(),
            }.map_err(|_| out.error())?;
            write!(out, ")").map_err(|_| out.error())?;
        }
        write!(
            out,
            " {{ invocation_source_return_refused_v36(source) }} else {{\n let returned = "
        )
        .map_err(|_| out.error())?;
        if self.class == ReturnClass::Unit {
            write!(out, "MemoryValueV30::Unit").map_err(|_| out.error())?;
        } else {
            write!(
                out,
                "source.machine.values[{}]",
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
            Some(local) => write!(out, "Some({local}int)"),
            None => write!(out, "None"),
        }
        .map_err(|_| out.error())?;
        match self.continuation {
            Some(block) => write!(out, ", {block}int)\n }}\n}}\n"),
            None => write!(out, ", -1int)\n }}\n}}\n"),
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
        + h::<Range<usize>>()
        + h::<ReturnClass>()
        + h::<Option<usize>>()
        + 24 * size_of::<usize>()
        + 24 * size_of::<&()>()
}

pub(super) const SOURCE_FRAMES_V36: &str = r#"
struct InvocationSourceByteReturnV36 {
    source: InvocationSourceByteStateV36,
    returned: MemoryValueV30,
}

open spec fn invocation_source_return_refused_v36(source: InvocationSourceByteStateV36)
    -> InvocationSourceByteReturnV36
{
    InvocationSourceByteReturnV36 { source: invocation_source_byte_refused_v36(source),
        returned: MemoryValueV30::Undefined }
}

open spec fn invocation_source_value_escapes_frame_v36(
    value: MemoryValueV30, frame: MemoryDynamicFrameV30,
) -> bool {
    match value {
        MemoryValueV30::Pointer(pointer) => byte_allocation_in_frame_v30(pointer.allocation, frame),
        MemoryValueV30::Slice(slice) => byte_allocation_in_frame_v30(slice.pointer.allocation, frame),
        _ => false,
    }
}

// Initialized pointer fragments retain nominal provenance even when they do
// not authorize a complete pointer load. Deinitialized historical tokens do not.
open spec fn invocation_source_memory_escapes_frame_v37(
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
open spec fn invocation_source_return_v36(
    source: InvocationSourceByteStateV36, begin: int, end: int,
    returned: MemoryValueV30, destination: Option<int>, continuation: int,
) -> InvocationSourceByteReturnV36 {
    if !source.machine.valid || !invocation_source_byte_state_well_formed_v36(source)
        || source.machine.frames.active.len() == 0
        || begin < 0 || end < begin || source.machine.values.len() < end
        || match returned { MemoryValueV30::Undefined => true, _ => false }
        || match destination {
            Some(local) => local < 0 || source.machine.values.len() <= local
                || (begin <= local < end) || source.machine.frames.active.len() < 2
                || continuation < 0,
            None => source.machine.frames.active.len() != 1 || continuation != -1,
        }
    {
        invocation_source_return_refused_v36(source)
    } else {
        let frame = source.machine.frames.active.last();
        // Capture first, install the returned value in the caller, then clear
        // only callee locals. One immutable source snapshot supplies all RHSs.
        let copied = match destination {
            Some(local) => source.machine.values.update(local, returned),
            None => source.machine.values,
        };
        let values = Seq::new(copied.len(), |i: int|
            if begin <= i < end { MemoryValueV30::Undefined } else { copied[i] });
        let logical = invocation_source_logical_clear_v38(match destination {
            Some(local) => invocation_source_logical_write_v38(source.logical, local),
            None => source.logical,
        }, begin, end);
        let escapes = invocation_source_value_escapes_frame_v36(returned, frame)
            || (exists|i: int| 0 <= i < values.len()
                && invocation_source_value_escapes_frame_v36(values[i], frame))
            || invocation_source_memory_escapes_frame_v37(source.machine.memory, frame)
            || (exists|i: int| logical.references.contains_key(i)
                && logical.references[i].frame == frame);
        if escapes {
            invocation_source_return_refused_v36(source)
        } else {
            InvocationSourceByteReturnV36 {
                source: InvocationSourceByteStateV36 {
                    machine: MemoryStateV30 { pc: continuation, values,
                        memory: byte_end_frame_v30(source.machine.memory, frame),
                        generations: source.machine.generations,
                        frames: byte_pop_frame_v30(source.machine.frames), valid: true },
                    slots: Map::new(
                        |descriptor: int| source.slots.contains_key(descriptor)
                            && !byte_allocation_in_frame_v30(source.slots[descriptor].allocation, frame),
                        |descriptor: int| source.slots[descriptor],
                    ),
                    logical,
                }, returned,
            }
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
        let copied = text.find("let copied = match destination").unwrap();
        let cleared = text
            .find("if begin <= i < end { MemoryValueV30::Undefined } else { copied[i] }")
            .unwrap();
        let checked = text.find("let escapes =").unwrap();
        assert!(copied < cleared && cleared < checked);
        assert!(text.contains("invocation_source_value_escapes_frame_v36(returned, frame)"));
        assert!(text.contains("exists|i: int| 0 <= i < values.len()"));
        assert!(
            text.contains(
                "invocation_source_memory_escapes_frame_v37(source.machine.memory, frame)"
            )
        );
        assert!(text.contains("memory.live[allocation].relocations[at].pointer.allocation, frame"));
        assert!(text.contains("!byte_allocation_in_frame_v30(allocation, frame)"));
        assert!(text.contains("source.slots.contains_key(descriptor)\n                            && !byte_allocation_in_frame_v30(source.slots[descriptor].allocation, frame)"));
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
            Range<usize>,
            Option<usize>,
            Option<usize>,
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
                + h::<Range<usize>>()
                + h::<ReturnClass>()
                + h::<Option<usize>>()
                + 24 * size_of::<usize>()
                + 24 * size_of::<&()>()
        );
    }
}
