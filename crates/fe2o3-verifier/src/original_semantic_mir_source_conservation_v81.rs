//! Proof hints for a closed, independently classified memory-neutral subset.
//! A nonmatching event keeps the full paired-step obligation, without this hint.

use super::super::source_bytes::{Destination, Value};
use super::*;

pub(super) fn event_supported(event: Event) -> bool {
    matches!(
        event,
        Event::Transfer {
            destination: Destination::Local(_),
            value: Value::Constant(_),
            ..
        }
    )
}

fn function_supported(
    function: &SourceByteFunction<'_, '_, '_>,
    functions: &[Option<SourceByteFunction<'_, '_, '_>>],
    out: &mut Writer<'_, '_>,
) -> Result<bool> {
    if !function.enter.heap_conservation_shape(out)?
        || !function.returned.heap_conservation_shape(out)?
    {
        return Ok(false);
    }
    for (block, row) in function.control.iter().enumerate() {
        out.budget.charge_work(1)?;
        for statement in 0..row.statements {
            out.budget.charge_work(1)?;
            if !event_supported(function.body.event_at(block, statement, out)?) {
                return Ok(false);
            }
        }
        match &row.end {
            End::Unreachable | End::Goto(_) | End::Return => (),
            End::Call { child, arguments } => {
                out.budget.charge_work(1)?;
                if functions.get(*child).and_then(Option::as_ref).is_none() {
                    return Ok(false);
                }
                out.budget.charge_work(arguments.len())?;
                if !arguments
                    .iter()
                    .all(|operand| operand.scalar_local_for_conservation())
                {
                    return Ok(false);
                }
            }
            End::Switch { operand, .. } if operand.scalar_local_for_conservation() => (),
            _ => return Ok(false),
        }
    }
    Ok(true)
}

pub(super) fn derive(
    program: &SourceByteProgram<'_, '_, '_>,
    root: usize,
    out: &mut Writer<'_, '_>,
) -> Result<Option<Vec<usize>>> {
    out.budget.reserve_storage(headers())?;
    out.budget.charge_work(2)?;
    let range = &program.roots.get(root).ok_or_else(mismatch)?.0;
    let functions = program.functions.get(range.clone()).ok_or_else(mismatch)?;
    // Classify before retaining any fuel rows; no source text or target opcode
    // can substitute for the authenticated source event and frame owners.
    for (instance, function) in functions.iter().enumerate() {
        out.budget.charge_work(1)?;
        if let Some(function) = function {
            out.budget.charge_work(2)?;
            if function.root != root || function.instance != instance {
                return Err(mismatch());
            }
            if !function_supported(function, functions, out)? {
                return Ok(None);
            }
        }
    }
    structural_fuels(functions, out).map(Some)
}

pub(super) fn structural_fuels(
    functions: &[Option<SourceByteFunction<'_, '_, '_>>],
    out: &mut Writer<'_, '_>,
) -> Result<Vec<usize>> {
    out.budget.reserve_storage(
        size_of::<Vec<usize>>()
            + 2 * size_of::<Result<Vec<usize>>>()
            + size_of::<std::slice::Iter<'_, Option<SourceByteFunction<'_, '_, '_>>>>()
            + size_of::<std::slice::Iter<'_, BodyBlock>>()
            + 4 * size_of::<usize>(),
    )?;
    let mut fuels = vector(functions.len(), out)?;
    for function in functions {
        out.budget.charge_work(1)?;
        let mut fuel = 0;
        if let Some(function) = function {
            for row in &function.control {
                out.budget.charge_work(1)?;
                fuel = fuel.max(row.statements.checked_add(1).ok_or(Resource::Arithmetic)?);
            }
        }
        fuels.push(fuel);
    }
    Ok(fuels)
}

fn headers() -> usize {
    fn h<T>() -> usize {
        size_of::<T>() + 2 * size_of::<Result<T>>()
    }
    h::<Option<Vec<usize>>>()
        + h::<Vec<usize>>()
        + h::<bool>()
        + size_of::<std::slice::Iter<'_, Option<SourceByteFunction<'_, '_, '_>>>>()
        + size_of::<
            std::iter::Enumerate<std::slice::Iter<'_, Option<SourceByteFunction<'_, '_, '_>>>>,
        >()
        + size_of::<std::iter::Enumerate<std::slice::Iter<'_, BodyBlock>>>()
        + size_of::<std::slice::Iter<'_, TypedOperand>>()
        + size_of::<Range<usize>>()
        + 12 * size_of::<usize>()
        + 8 * size_of::<&()>()
}

#[cfg(test)]
#[path = "original_semantic_mir_source_conservation_v81_tests.rs"]
mod tests;
