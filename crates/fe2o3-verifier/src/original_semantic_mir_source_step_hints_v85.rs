//! Bounded proof coordinates copied from the same authenticated source owner.

use super::*;

pub(super) fn derive(
    program: &SourceByteProgram<'_, '_, '_>,
    root: usize,
    out: &mut Writer<'_, '_>,
) -> Result<Option<SourceStepHintsV85>> {
    out.budget.reserve_storage(headers())?;
    let conserved_fuels = conservation::derive(program, root, out)?;
    let conserves_heap = conserved_fuels.is_some();
    out.budget.charge_work(2)?;
    let range = &program.roots.get(root).ok_or_else(mismatch)?.0;
    let functions = program.functions.get(range.clone()).ok_or_else(mismatch)?;
    let mut count = 0usize;
    for function in functions.iter().flatten() {
        out.budget.charge_work(1)?;
        for row in &function.control {
            out.budget.charge_work(1)?;
            if matches!(row.end, End::Abort) {
                return Ok(None);
            }
        }
        count = count
            .checked_add(function.control.len())
            .ok_or(Resource::Arithmetic)?;
    }
    // Fuel describes the authenticated micro-program, not its memory effects.
    // Only the independently classified subset may use the no-write theorem.
    let fuels = match conserved_fuels {
        Some(fuels) => fuels,
        None => conservation::structural_fuels(functions, out)?,
    };
    let mut entries = vector(functions.len(), out)?;
    let mut cuts = vector(count, out)?;
    for (instance, function) in functions.iter().enumerate() {
        out.budget.charge_work(1)?;
        let Some(function) = function else {
            entries.push(None);
            continue;
        };
        out.budget.charge_work(3)?;
        if function.root != root
            || function.instance != instance
            || function.blocks.len() != function.control.len()
        {
            return Err(mismatch());
        }
        entries.push(if function.enter.heap_conservation_shape(out)? {
            Some(function.enter.step_proof_hints(out)?)
        } else {
            None
        });
        for (block, row) in function.control.iter().enumerate() {
            out.budget.charge_work(6)?;
            let (operands, call) = match &row.end {
                End::Call { child, arguments } => {
                    out.budget.charge_work(arguments.len())?;
                    if !arguments
                        .iter()
                        .all(|argument| argument.scalar_local_coordinates().is_some())
                    {
                        (arguments.len(), None)
                    } else {
                        let mut captured = vector(arguments.len(), out)?;
                        for argument in arguments {
                            out.budget.charge_work(1)?;
                            captured
                                .push(argument.scalar_local_coordinates().ok_or_else(mismatch)?);
                        }
                        (
                            arguments.len(),
                            Some(SourceCallHintsV85 {
                                child: *child,
                                arguments: captured,
                            }),
                        )
                    }
                }
                End::Switch { .. } => (1, None),
                _ => (0, None),
            };
            cuts.push(SourceCutHintsV85 {
                pc: function
                    .blocks
                    .start
                    .checked_add(block)
                    .ok_or(Resource::Arithmetic)?,
                instance,
                statements: row.statements,
                operands,
                call,
                frame_preserving: !conserves_heap && cut_frames::supports(function, block, out)?,
                normalization: match row.end {
                    End::ThreadWrite(call)
                        if row.statements == 0 && call.normalization_coordinates().is_some() =>
                    {
                        Some(call)
                    }
                    _ => None,
                },
            });
        }
    }
    Ok(Some(SourceStepHintsV85 {
        conserves_heap,
        fuels,
        entries,
        cuts,
    }))
}

fn headers() -> usize {
    fn h<T>() -> usize {
        size_of::<T>() + 2 * size_of::<Result<T>>()
    }
    h::<Option<SourceStepHintsV85>>()
        + h::<SourceStepHintsV85>()
        + h::<Vec<usize>>()
        + h::<Option<Vec<usize>>>()
        + h::<bool>()
        + h::<Vec<Option<SourceEntryHintsV85>>>()
        + h::<Vec<SourceCutHintsV85>>()
        + h::<Vec<(usize, bool, u32)>>()
        + h::<Option<SourceCallHintsV85>>()
        + h::<SourceCutHintsV85>()
        + h::<Option<thread_write::ThreadWriteCall>>()
        + h::<Option<(usize, u32)>>()
        + h::<SourceEntryHintsV85>()
        + h::<SourceCallHintsV85>()
        + h::<(usize, bool, u32)>()
        + size_of::<std::slice::Iter<'_, Option<SourceByteFunction<'_, '_, '_>>>>()
        + size_of::<std::iter::Flatten<std::slice::Iter<'_, Option<SourceByteFunction<'_, '_, '_>>>>>(
        )
        + size_of::<
            std::iter::Enumerate<std::slice::Iter<'_, Option<SourceByteFunction<'_, '_, '_>>>>,
        >()
        + size_of::<std::iter::Enumerate<std::slice::Iter<'_, BodyBlock>>>()
        + size_of::<std::slice::Iter<'_, TypedOperand>>()
        + 12 * size_of::<usize>()
        + 8 * size_of::<&()>()
}

#[cfg(test)]
#[path = "original_semantic_mir_source_step_hints_v85_tests.rs"]
mod tests;
