//! Heap-only proof tactic for an authenticated empty Unit root return.

use super::*;
use fe2o3_kernel_ir::{OperationKind, Terminator};

#[derive(Clone, Copy)]
pub(super) struct Summary {
    begin: usize,
    end: usize,
    source_pc: usize,
    target_pc: usize,
}

pub(super) struct RootScan<'a> {
    row: &'a Root,
    allocation_free: bool,
    required: usize,
}

impl RootScan<'_> {
    pub(super) fn allocation_free(
        &self,
        model: &PairedInvocations<'_, '_, '_>,
        root: usize,
        out: &mut Writer<'_, '_>,
    ) -> Result<bool> {
        model
            .slots
            .check_query_storage_floor(self.required, out.budget)?;
        model.check(out)?;
        if !std::ptr::eq(self.row, model.roots.get(root).ok_or_else(mismatch)?) {
            return Err(mismatch());
        }
        out.budget.charge_work(1)?;
        Ok(self.allocation_free)
    }
}

pub(super) fn scan<'a>(
    model: &'a PairedInvocations<'_, '_, '_>,
    root: usize,
    out: &mut Writer<'_, '_>,
) -> Result<RootScan<'a>> {
    model.check(out)?;
    out.budget.reserve_storage(headers())?;
    out.budget.charge_work(1)?;
    let row = model.roots.get(root).ok_or_else(mismatch)?;
    let inventory = model.slots.correspondence(out)?.inventory(out.budget)?;
    let mut allocation_free = true;
    // One complete allocation census per root, reused by all its return cuts.
    for block in inventory
        .blocks()
        .get(row.blocks.clone())
        .ok_or_else(mismatch)?
    {
        out.budget.charge_work(1)?;
        for operation in inventory
            .operations()
            .get(block.operations.clone())
            .ok_or_else(mismatch)?
        {
            out.budget.charge_work(1)?;
            allocation_free &= !matches!(operation.operation.kind, OperationKind::Alloca { .. });
        }
    }
    Ok(RootScan {
        row,
        allocation_free,
        required: out.budget.storage(),
    })
}

pub(super) fn derive(
    model: &PairedInvocations<'_, '_, '_>,
    scan: &RootScan<'_>,
    root: usize,
    block: usize,
    hint: &SourceCutHintsV85,
    out: &mut Writer<'_, '_>,
) -> Result<Option<Summary>> {
    model
        .slots
        .check_query_storage_floor(scan.required, out.budget)?;
    model.check(out)?;
    let row = model.roots.get(root).ok_or_else(mismatch)?;
    if !std::ptr::eq(scan.row, row) {
        return Err(mismatch());
    }
    out.budget.reserve_storage(headers())?;
    out.budget.charge_work(10)?;
    let cut = row
        .cuts
        .get(block)
        .and_then(Option::as_ref)
        .ok_or_else(mismatch)?;
    if hint.pc != cut.source || add(row.instances.start, hint.instance)? != cut.instance {
        return Err(mismatch());
    }
    let Some((owner, locals)) = &hint.root_unit_return_v313 else {
        return Ok(None);
    };
    if !matches!(cut.end, End::Return)
        || hint.instance != 0
        || hint.statements != 0
        || hint.operands != 0
        || hint.call.is_some()
        || *owner != row.owner
        || locals.start > locals.end
        || locals.end > model.locals
        || !scan.allocation_free
    {
        return Ok(None);
    }
    let instance = model
        .instances
        .get(cut.instance)
        .and_then(Option::as_ref)
        .ok_or_else(mismatch)?;
    if instance.owners.as_slice() != [*owner]
        || !instance.suspended.is_empty()
        || instance.returned.is_some()
    {
        return Ok(None);
    }
    let inventory = model.slots.correspondence(out)?.inventory(out.budget)?;
    let target_pc = add(row.blocks.start, block)?;
    let target = inventory.blocks().get(target_pc).ok_or_else(mismatch)?;
    if !target_shape(
        target.operations.len(),
        target.edges.len(),
        target.terminator,
    ) {
        return Ok(None);
    }
    Ok(Some(Summary {
        begin: locals.start,
        end: locals.end,
        source_pc: cut.source,
        target_pc,
    }))
}

fn target_shape(operations: usize, edges: usize, terminator: &Terminator) -> bool {
    operations == 0
        && edges == 0
        && matches!(terminator, Terminator::Return { values } if values.is_empty())
}

pub(super) fn emit(root: usize, summary: &Summary, out: &mut Writer<'_, '_>) -> Result<()> {
    out.budget.reserve_storage(headers())?;
    out.budget.charge_work(30)?;
    let Summary {
        begin,
        end,
        source_pc,
        target_pc,
    } = *summary;
    emit!(
        out,
        r#" hide(invocation_paired_source_step_{root}_v36);
 hide(invocation_paired_actual_step_{root}_v36);
 hide(invocation_source_block_runtime_{root}_v36);
 hide(invocation_byte_boundary_{root}_v36);
 hide(invocation_source_return_v36);
 hide(invocation_source_logical_clear_v38);
 hide(invocation_source_snapshot_escapes_frame_v42);
 hide(invocation_source_value_escapes_frame_v36);
 hide(invocation_source_memory_escapes_frame_v37);
 hide(invocation_source_byte_state_well_formed_v36);
 hide(invocation_byte_heaps_related_v36);
 hide(byte_end_frame_v30);
 let map = invocation_source_byte_map_{root}_v36(source, target);
 assert(map.private == Map::<MemoryAllocationV30, InvocationByteBindingV36>::empty());
 assert(invocation_byte_heaps_related_v36(source.machine.memory, target.memory, map));
 invocation_empty_private_map_has_no_private_source_v78(source.machine.memory, target.memory, map);
 invocation_related_target_inputs_v96(source.machine, target, map);
 invocation_paired_source_defined_step_valid_{root}_v92(source);
 let little_endian = invocation_runtime_little_endian_v36();
 invocation_source_plain_return_preserves_heap_v78(source, {begin}, {end}, MemoryValueV30::Unit, None, -1int, little_endian);
 let returned = invocation_source_return_v36(source, {begin}, {end}, InvocationSourceValueV42::Carrier(MemoryValueV30::Unit), None, -1int, little_endian);
 let original = invocation_paired_source_step_{root}_v36(source).state;
 let actual = invocation_paired_actual_step_{root}_v36(target).state;
 assert(source.machine.pc == {source_pc} && target.pc == {target_pc});
 assert(original == returned.source) by {{
  reveal(invocation_paired_source_step_{root}_v36);
  reveal(invocation_source_block_runtime_{root}_v36);
  reveal_with_fuel(invocation_source_micro_run_{root}_0_v36, 1);
 }}
 assert(actual == MemoryStateV30 {{ pc: -1int, ..target }}) by {{
  reveal(invocation_paired_actual_step_{root}_v36);
  reveal(invocation_byte_boundary_{root}_v36);
  reveal_with_fuel(invocation_byte_follow_{root}_v36, 1);
 }}
 assert(original.machine.valid);
 assert(invocation_source_byte_state_well_formed_v36(original)
     && original.machine.frames.execution == source.machine.frames.execution) by {{
  reveal(invocation_source_return_v36);
 }}
 assert(original.machine.memory == source.machine.memory);
 assert(original.machine.generations == source.machine.generations);
 assert(invocation_source_byte_map_{root}_v36(original, actual) == map);
 assert(invocation_byte_heaps_related_v36(original.machine.memory, actual.memory, map));
 assert(invocation_byte_states_related_v36(original.machine, actual, map)) by {{
  reveal(invocation_source_byte_state_well_formed_v36);
  reveal(invocation_byte_states_related_v36);
 }}
"#
    );
    Ok(())
}

fn headers() -> usize {
    size_of::<Summary>()
        + size_of::<RootScan<'_>>()
        + size_of::<Result<RootScan<'_>>>()
        + size_of::<Option<Summary>>()
        + size_of::<Result<Option<Summary>>>()
        + size_of::<Result<()>>()
        + size_of::<std::slice::Iter<'_, fe2o3_kernel_analysis::CanonicalKirBlockRefV1<'_>>>()
        + size_of::<std::slice::Iter<'_, fe2o3_kernel_analysis::CanonicalKirOperationRefV1<'_>>>()
        + 12 * size_of::<usize>()
        + 12 * size_of::<&()>()
}

#[cfg(test)]
#[path = "original_semantic_mir_root_return_heap_v313_tests.rs"]
mod tests;
