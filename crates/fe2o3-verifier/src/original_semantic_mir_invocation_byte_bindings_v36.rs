//! Runtime private mappings are derived from the original Alloca result, not
//! supplied by a caller or guessed from a latest-generation counter.

use super::super::super::byte_function_v30::ByteAllocationResolverV30;
use super::{Error, Resource, Result, Writer, slots::SourceSlots, vector};
use fe2o3_kernel_ir::OperationKind;
use std::{fmt::Write as _, mem::size_of, ops::Range};

#[derive(Clone, Copy, Debug)]
struct Binding {
    descriptor: usize,
    definition: usize,
    physical_owner: usize,
}

pub(super) struct SourceByteBindings<'slots, 'view, 'source> {
    slots: &'slots SourceSlots<'view, 'source>,
    rows: Vec<Binding>,
    roots: Vec<Range<usize>>,
    definitions: usize,
    required: usize,
}

fn mismatch() -> Error {
    Error::Statement("original MIR byte map differs from its authenticated Alloca results")
}

impl<'slots, 'view, 'source> SourceByteBindings<'slots, 'view, 'source> {
    pub(super) fn derive(
        slots: &'slots SourceSlots<'view, 'source>,
        out: &mut Writer<'_, '_>,
    ) -> Result<Self> {
        out.budget.reserve_storage(headers())?;
        let relation = slots.correspondence(out)?;
        let inventory = relation.inventory(out.budget)?;
        let source = relation.source(out.budget)?;
        let roots = source.root_count(out.budget)?;
        let mut count = 0usize;
        for operation in inventory.operations() {
            out.budget.charge_work(1)?;
            if matches!(operation.operation.kind, OperationKind::Alloca { .. }) {
                count = count.checked_add(1).ok_or(Resource::Arithmetic)?;
            }
        }
        if roots != inventory.functions().len() {
            return Err(mismatch());
        }
        let mut rows = vector(count, out)?;
        let mut ranges = vector(roots, out)?;
        let mut seen = vector(roots, out)?;
        out.budget.charge_work(roots)?;
        seen.resize(roots, false);
        for root in 0..roots {
            let (_, physical) = source.root(root, out.budget)?;
            out.budget.charge_work(2)?;
            if seen.get(physical) != Some(&false) {
                return Err(mismatch());
            }
            seen[physical] = true;
            let function = inventory.functions().get(physical).ok_or_else(mismatch)?;
            let begin = rows.len();
            for operation in inventory
                .operations()
                .get(function.operations.clone())
                .ok_or_else(mismatch)?
            {
                out.budget.charge_work(1)?;
                if !matches!(operation.operation.kind, OperationKind::Alloca { .. }) {
                    continue;
                }
                out.budget.charge_work(5)?;
                if operation.results.len() != 1 {
                    return Err(mismatch());
                }
                let frame = slots.frame_by_allocation(operation.coordinate, out)?;
                let (descriptor, same) = slots.descriptor_by_source(
                    frame.root(),
                    frame.instance(),
                    frame.local(),
                    frame.source_generation(),
                    out,
                )?;
                let site = slots.site(operation.coordinate, out)?;
                if same.allocation() != operation.coordinate
                    || site.original != operation.coordinate
                    || frame.root() != root
                    || inventory
                        .definitions()
                        .get(operation.results.start)
                        .is_none()
                {
                    return Err(mismatch());
                }
                rows.push(Binding {
                    descriptor,
                    definition: operation.results.start,
                    physical_owner: site.physical_root_owner,
                });
            }
            ranges.push(begin..rows.len());
        }
        if rows.len() != count {
            return Err(mismatch());
        }
        let released = seen
            .capacity()
            .checked_mul(size_of::<bool>())
            .ok_or(Resource::Arithmetic)?;
        drop(seen);
        out.budget.release_storage(released)?;
        Ok(Self {
            slots,
            rows,
            roots: ranges,
            definitions: inventory.definitions().len(),
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
        for (root, range) in self.roots.iter().enumerate() {
            out.budget.charge_work(1)?;
            write!(out, "open spec fn invocation_source_byte_map_{root}_v36(source: InvocationSourceByteStateV36, target: MemoryStateV30) -> InvocationByteMapV36 {{\n let private = Map::<MemoryAllocationV30, InvocationByteBindingV36>::empty();\n").map_err(|_| out.error())?;
            for row in &self.rows[range.clone()] {
                out.budget.charge_work(1)?;
                let (descriptor, definition) = (row.descriptor, row.definition);
                write!(out, " let private = if source.slots.contains_key({descriptor}) && {definition} < target.values.len() {{ match target.values[{definition}] {{ MemoryValueV30::Pointer(pointer) => private.insert(source.slots[{descriptor}].allocation, InvocationByteBindingV36 {{ target: pointer, extent: invocation_source_slot_{descriptor}_v36().extent, alignment: invocation_source_slot_{descriptor}_v36().alignment }}), _ => private }} }} else {{ private }};\n").map_err(|_| out.error())?;
            }
            write!(out, " InvocationByteMapV36 {{ private }}\n}}\nopen spec fn invocation_source_byte_map_valid_{root}_v36(source: InvocationSourceByteStateV36, target: MemoryStateV30) -> bool {{\n invocation_source_byte_state_well_formed_v36(source) && target.values.len() == {} && (forall|descriptor: int| source.slots.contains_key(descriptor) ==> (false", self.definitions).map_err(|_| out.error())?;
            for row in &self.rows[range.clone()] {
                out.budget.charge_work(1)?;
                write!(out, " || descriptor == {}", row.descriptor).map_err(|_| out.error())?;
            }
            write!(out, "))").map_err(|_| out.error())?;
            for row in &self.rows[range.clone()] {
                out.budget.charge_work(1)?;
                let (descriptor, definition, owner) =
                    (row.descriptor, row.definition, row.physical_owner);
                write!(out, "\n && (source.slots.contains_key({descriptor}) ==> {{ let slot = invocation_source_slot_{descriptor}_v36(); let original = source.slots[{descriptor}]; original.byte_offset == 0 && match original.allocation {{ MemoryAllocationV30::Private {{ owner, site, generation, .. }} => owner == slot.owner && site == slot.site && generation >= 0, _ => false }} && source.machine.memory.live.contains_key(original.allocation) && source.machine.memory.live[original.allocation].bytes.len() == slot.extent && source.machine.memory.live[original.allocation].base_alignment == slot.alignment && match target.values[{definition}] {{ MemoryValueV30::Pointer(pointer) => pointer.byte_offset == 0 && match pointer.allocation {{ MemoryAllocationV30::Private {{ owner, invocation, site, generation }} => owner == {owner} && invocation == 0 && site == slot.site && generation >= 0, _ => false }} && target.memory.live.contains_key(pointer.allocation) && target.memory.live[pointer.allocation].bytes.len() == slot.extent && target.memory.live[pointer.allocation].base_alignment == slot.alignment, _ => false }} }})").map_err(|_| out.error())?;
            }
            write!(out, "\n}}\nopen spec fn invocation_source_byte_storage_related_{root}_v36(source: InvocationSourceByteStateV36, target: MemoryStateV30) -> bool {{ invocation_source_byte_map_valid_{root}_v36(source, target) && invocation_byte_states_related_v36(source.machine, target, invocation_source_byte_map_{root}_v36(source, target)) }}\n").map_err(|_| out.error())?;
        }
        Ok(())
    }
}

fn headers() -> usize {
    fn h<T>() -> usize {
        size_of::<T>() + 2 * size_of::<Result<T>>()
    }
    h::<SourceByteBindings<'_, '_, '_>>()
        + h::<Vec<Binding>>()
        + h::<Binding>()
        + h::<Vec<Range<usize>>>()
        + h::<Vec<bool>>()
        + h::<Range<usize>>()
        + 18 * size_of::<usize>()
        + 16 * size_of::<&()>()
}
