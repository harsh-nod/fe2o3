//! Constructor composition for descriptor-only cuts selected from authentic events.

use super::*;

pub(super) const SHARED: &str = include_str!("original_semantic_mir_source_wf_laws_v95.vrs");

pub(super) fn supports(
    function: &SourceByteFunction<'_, '_, '_>,
    block: usize,
    out: &mut Writer<'_, '_>,
) -> Result<bool> {
    out.budget.reserve_storage(headers())?;
    out.budget.charge_work(1)?;
    let row = function.control.get(block).ok_or_else(mismatch)?;
    if !matches!(row.end, End::Descriptor(_)) {
        return Ok(false);
    }
    for statement in 0..row.statements {
        out.budget.charge_work(1)?;
        if !function
            .body
            .event_at(block, statement, out)?
            .descriptor_wf_shape_v95()
        {
            return Ok(false);
        }
    }
    Ok(true)
}

pub(super) fn emit(hint: &SourceCutHintsV85, root: usize, out: &mut Writer<'_, '_>) -> Result<()> {
    out.budget.reserve_storage(headers())?;
    out.budget.charge_work(4)?;
    let (block, call) = hint.descriptor_wf.ok_or_else(mismatch)?;
    let instance = hint.instance;
    write!(out, " let little_endian = invocation_runtime_little_endian_v36();\n let c0 = invocation_source_micro_begin_{root}_{instance}_v36(source);\n assert(invocation_source_byte_state_well_formed_v36(c0.source));\n").map_err(|_| out.error())?;
    for statement in 0..hint.statements {
        out.budget.charge_work(1)?;
        let next = statement.checked_add(1).ok_or(Resource::Arithmetic)?;
        write!(out, " match invocation_source_byte_event_{root}_{instance}_v36({block}, {statement}) {{\n Some(InvocationSourceByteEventV36::Descriptor(InvocationSourceDescriptorEventV51::Borrow {{ destination, recipe, parent: None }})) => {{\n assert(!c{statement}.source.objects.contains_key(destination)) by {{ reveal(invocation_source_byte_state_well_formed_v36); }}\n assert(invocation_source_descriptor_step_v51(c{statement}.source, InvocationSourceDescriptorEventV51::Borrow {{ destination, recipe, parent: None }}).machine.valid) by {{ reveal(invocation_source_descriptor_length_v51); reveal(invocation_source_byte_put_local_v36); }}\n invocation_source_descriptor_borrow_wf_v95(c{statement}.source, destination, recipe);\n }},\n Some(InvocationSourceByteEventV36::Pointer(InvocationSourcePointerEventV36::Copy {{ destination, operand: InvocationSourceOperandV36::Slice {{ local, moved, metadata_bits: operand_bits }}, metadata_bits }})) => {{\n invocation_source_slice_copy_wf_v95(c{statement}.source, destination, local, moved, operand_bits, metadata_bits, {root}, {instance}, little_endian);\n }},\n _ => {{}},\n }}\n let c{next} = invocation_source_micro_step_{root}_{instance}_v36(c{statement}, little_endian);\n assert(invocation_source_byte_state_well_formed_v36(c{next}.source));\n").map_err(|_| out.error())?;
    }
    call.emit_wf_proof(hint.statements, out)
}

fn headers() -> usize {
    12 * size_of::<usize>()
        + 8 * size_of::<&()>()
        + size_of::<Event>()
        + 2 * size_of::<Result<Event>>()
        + size_of::<descriptor_calls::DescriptorCall>()
        + size_of::<Result<bool>>()
        + 3 * size_of::<Result<()>>()
}
