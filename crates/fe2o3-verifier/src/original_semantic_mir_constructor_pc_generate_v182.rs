//! Constructor control projection independent of runtime and definedness context.

use super::*;

pub(super) fn emit(
    root: usize,
    pc: usize,
    call: &SourceCallHintsV85,
    entry: &SourceEntryHintsV85,
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    out.budget.reserve_storage(16 * size_of::<usize>())?;
    out.budget.charge_work(9)?;
    if entry.arguments.len() != call.arguments.len() {
        return Err(mismatch());
    }
    emit!(
        out,
        "#[verifier::spinoff_prover]\nproof fn invocation_constructor_source_pc_{root}_{pc}_v182(source: InvocationSourceByteStateV36)\n ensures invocation_constructor_source_{root}_{pc}_v162(source).machine.pc == {},\n{{\n hide(invocation_source_entry_initialize_v166);\n hide(invocation_source_byte_put_local_v36);\n",
        entry.pc
    );
    emit!(
        out,
        " invocation_source_entry_initialize_pc_v166(source, {}, {}, {}, byte_enter_frame_v30(source.machine.frames, {}));\n let installed_0 = invocation_source_entry_initialize_v166(source, {}, {}, {}, byte_enter_frame_v30(source.machine.frames, {}));\n",
        entry.pc,
        entry.locals.start,
        entry.locals.end,
        entry.owner,
        entry.pc,
        entry.locals.start,
        entry.locals.end,
        entry.owner
    );
    for (ordinal, (destination, (local, _, _))) in
        entry.arguments.iter().zip(&call.arguments).enumerate()
    {
        out.budget.charge_work(4)?;
        let next = add(ordinal, 1)?;
        emit!(
            out,
            " invocation_source_entry_put_local_pc_v179(installed_{ordinal}, {destination}, source.machine.values[{local}]);\n let installed_{next} = invocation_source_byte_put_local_v36(installed_{ordinal}, {destination}, source.machine.values[{local}]);\n"
        );
    }
    emit!(
        out,
        " assert(invocation_constructor_source_{root}_{pc}_v162(source).machine.pc == {});\n}}\n",
        entry.pc
    );
    Ok(())
}
