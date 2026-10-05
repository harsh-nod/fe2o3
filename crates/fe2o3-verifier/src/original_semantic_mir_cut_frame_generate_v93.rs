//! Local frame summaries selected from the retained original micro-program.

use super::*;

const SHARED: &str = include_str!("original_semantic_mir_cut_frame_laws_v93.vrs");

pub(super) fn supports(
    function: &SourceByteFunction<'_, '_, '_>,
    block: usize,
    out: &mut Writer<'_, '_>,
) -> Result<bool> {
    out.budget.reserve_storage(
        4 * size_of::<usize>()
            + 3 * size_of::<&()>()
            + size_of::<Event>()
            + 2 * size_of::<Result<Event>>()
            + size_of::<Result<bool>>(),
    )?;
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
            .descriptor_frame_shape_v93()
        {
            return Ok(false);
        }
    }
    Ok(true)
}

pub(super) fn emit(
    program: &SourceByteProgram<'_, '_, '_>,
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    program.source_slots(out)?;
    out.budget.reserve_storage(headers())?;
    let mut shared = false;
    let mut wf_shared = false;
    for root in 0..program.roots.len() {
        out.budget.charge_work(1)?;
        let Some(hints) = step_hints::derive(program, root, out)? else {
            continue;
        };
        // The old no-write route already has its own shared preservation proof.
        if hints.conserves_heap {
            continue;
        }
        let functions = program
            .functions
            .get(program.roots[root].0.clone())
            .ok_or_else(mismatch)?;
        for hint in &hints.cuts {
            out.budget.charge_work(1)?;
            if !hint.frame_preserving {
                continue;
            }
            let function = functions
                .get(hint.instance)
                .and_then(Option::as_ref)
                .ok_or_else(mismatch)?;
            let block = hint
                .pc
                .checked_sub(function.blocks.start)
                .ok_or(Resource::Arithmetic)?;
            if !supports(function, block, out)? {
                return Err(mismatch());
            }
            if !shared {
                write!(out, "{SHARED}").map_err(|_| out.error())?;
                shared = true;
            }
            if hint.has_descriptor_wf() && !wf_shared {
                write!(out, "{}", source_wf::SHARED).map_err(|_| out.error())?;
                wf_shared = true;
            }
            emit_cut(function, block, out)?;
        }
    }
    Ok(())
}

fn emit_cut(
    function: &SourceByteFunction<'_, '_, '_>,
    block: usize,
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    out.budget.reserve_storage(headers())?;
    out.budget.charge_work(4)?;
    let (root, instance) = (function.root, function.instance);
    let row = function.control.get(block).ok_or_else(mismatch)?;
    let pc = function
        .blocks
        .start
        .checked_add(block)
        .ok_or(Resource::Arithmetic)?;
    let fuel = row.statements.checked_add(1).ok_or(Resource::Arithmetic)?;
    let End::Descriptor(call) = row.end else {
        return Err(mismatch());
    };
    write!(out, "proof fn invocation_cut_source_frame_{root}_{pc}_v93(source: InvocationSourceByteStateV36)\n requires source.machine.pc == {pc},\n ensures invocation_source_frame_equal_v93(source, invocation_paired_source_step_{root}_v36(source).state),\n{{\n hide(invocation_source_observations_v39);\n hide(invocation_source_byte_state_well_formed_v36);\n hide(invocation_source_descriptor_step_v51);\n hide(invocation_source_descriptor_length_v51);\n hide(invocation_source_pointer_step_v36);\n hide(invocation_source_byte_put_local_v36);\n let little_endian = invocation_runtime_little_endian_v36();\n let c0 = invocation_source_micro_begin_{root}_{instance}_v36(source);\n assert(invocation_source_frame_equal_v93(source, c0.source));\n").map_err(|_| out.error())?;
    for statement in 0..row.statements {
        out.budget.charge_work(1)?;
        if !function
            .body
            .event_at(block, statement, out)?
            .descriptor_frame_shape_v93()
        {
            return Err(mismatch());
        }
        let next = statement.checked_add(1).ok_or(Resource::Arithmetic)?;
        write!(out, " match invocation_source_byte_event_{root}_{instance}_v36({block}, {statement}) {{\n Some(InvocationSourceByteEventV36::Descriptor(event)) => {{ invocation_cut_frame_descriptor_step_v93(c{statement}.source, event); }},\n Some(InvocationSourceByteEventV36::Pointer(InvocationSourcePointerEventV36::Copy {{ destination, operand: InvocationSourceOperandV36::Slice {{ local, moved, metadata_bits: operand_bits }}, metadata_bits }})) => {{ invocation_cut_frame_slice_copy_v93(c{statement}.source, destination, local, moved, operand_bits, metadata_bits, {root}, {instance}, little_endian); }},\n _ => {{}},\n }}\n let c{next} = invocation_source_micro_step_{root}_{instance}_v36(c{statement}, little_endian);\n assert(invocation_source_frame_equal_v93(c{statement}.source, c{next}.source));\n").map_err(|_| out.error())?;
    }
    call.emit_frame_proof(row.statements, out)?;
    write!(
        out,
        " reveal_with_fuel(invocation_source_micro_run_{root}_{instance}_v36, {fuel});\n}}\n"
    )
    .map_err(|_| out.error())
}

fn headers() -> usize {
    16 * size_of::<usize>()
        + 12 * size_of::<&()>()
        + 3 * size_of::<bool>()
        + size_of::<Event>()
        + size_of::<descriptor_calls::DescriptorCall>()
        + 2 * size_of::<Option<SourceStepHintsV85>>()
        + 2 * size_of::<Result<Option<SourceStepHintsV85>>>()
        + 4 * size_of::<Result<()>>()
        + size_of::<
            std::iter::Flatten<
                std::slice::Iter<'static, Option<SourceByteFunction<'static, 'static, 'static>>>,
            >,
        >()
}
